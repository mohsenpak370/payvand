//! The fee policy account and the treasury the fee lands in.
//!
//! `Config` is a singleton created once by the program's upgrade authority, so
//! the admin it records is never a caller's choice. Settlements only read it,
//! and the treasury is addressed by seed, so no settlement can name its own fee
//! recipient.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::bpf_loader_upgradeable::{self, UpgradeableLoaderState};

use crate::constants::*;
use crate::errors::PaymentError;
use crate::events::ConfigUpdated;
use crate::helpers::{pay_from_treasury, validate_fee_policy, validate_session_float};
use crate::state::{Config, FeeMode, FeePolicy};

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    /// The upgrade authority: pays for `Config` and becomes its admin.
    #[account(mut)]
    pub admin: Signer<'info>,

    /// CHECK: This program's `ProgramData` account. The seeds pin it to the
    /// upgradeable loader's PDA for this program id and the owner check pins it
    /// to that loader, so a decoy account cannot stand in for it. The handler
    /// reads the upgrade authority out of it.
    #[account(
        seeds = [crate::ID.as_ref()],
        bump,
        seeds::program = bpf_loader_upgradeable::id(),
        owner = bpf_loader_upgradeable::id()
    )]
    pub program_data: UncheckedAccount<'info>,

    #[account(
        init,
        payer = admin,
        space = CONFIG_SPACE,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, Config>,

    /// The fee destination. Created outside this instruction (it needs no data
    /// and no signature), and pinned here by its seed.
    #[account(mut, seeds = [TREASURY_SEED], bump)]
    pub treasury: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    pub admin: Signer<'info>,

    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump, has_one = admin)]
    pub config: Account<'info, Config>,
}

/// Upgrades the version 1 config account in place.
///
/// The account cannot be typed as `Account<'info, Config>`: it is still the old,
/// shorter layout, and deserializing it as `Config` would fail before the
/// handler could migrate it. It is a raw account here, pinned to the config seed
/// and checked by hand: the program must own it, `admin` must be the admin it
/// records, and it must be a version 1 account of exactly
/// `CONFIG_SPACE_V1` bytes. Anything else is refused rather than overwritten.
#[derive(Accounts)]
pub struct MigrateConfig<'info> {
    /// The recorded admin. Pays the rent the taller account now needs.
    #[account(mut)]
    pub admin: Signer<'info>,

    /// CHECK: The program's own `config` account, in its version 1 shape. The
    /// seeds derive it, and the handler checks its owner, its length and that
    /// the admin it records is the signer before touching it.
    #[account(mut, seeds = [CONFIG_SEED], bump)]
    pub config: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

/// Bincode tag of `UpgradeableLoaderState::ProgramData`, as a `u32` like every
/// bincode enum variant index. The enum order is `Uninitialized = 0`,
/// `Buffer = 1`, `Program = 2`, `ProgramData = 3` — pinned by the harness test
/// that serializes the real enum.
const PROGRAM_DATA_TAG: u32 = 3;
/// Offset of the optional authority inside the `ProgramData` metadata:
/// `4` (tag) + `8` (slot). Bincode writes `Option<Pubkey>` as a one-byte tag
/// followed by the key, which is where `size_of_programdata_metadata()`'s
/// `4 + 8 + 1 + 32` comes from.
const PROGRAM_DATA_AUTHORITY_OFFSET: usize = 12;

/// Reads the upgrade authority out of this program's `ProgramData` account.
///
/// The account is pinned to this program's PDA by the context's seeds and
/// owner constraints, so this only has to read its documented metadata: a
/// `u32` variant tag, the deploy slot, then the optional authority. Parsing it
/// by hand avoids pulling a serializer into the on-chain binary.
///
/// A program deployed without an upgrade authority (`None`) can never
/// bootstrap a config: there is no key whose signature could prove the right to
/// set the fee policy, and a random signer must never become admin.
fn upgrade_authority(program_data: &AccountInfo) -> Result<Option<Pubkey>> {
    let data = program_data.try_borrow_data()?;
    let metadata = UpgradeableLoaderState::size_of_programdata_metadata();
    require!(data.len() >= metadata, PaymentError::ProgramNotUpgradeable);
    let tag = u32::from_le_bytes(
        data[0..4]
            .try_into()
            .map_err(|_| error!(PaymentError::ProgramNotUpgradeable))?,
    );
    require!(tag == PROGRAM_DATA_TAG, PaymentError::ProgramNotUpgradeable);

    match data[PROGRAM_DATA_AUTHORITY_OFFSET] {
        0 => Ok(None),
        1 => {
            let start = PROGRAM_DATA_AUTHORITY_OFFSET + 1;
            let key: [u8; 32] = data[start..start + 32]
                .try_into()
                .map_err(|_| error!(PaymentError::ProgramNotUpgradeable))?;
            Ok(Some(Pubkey::new_from_array(key)))
        }
        _ => Err(PaymentError::ProgramNotUpgradeable.into()),
    }
}

/// The version 1 `Config` body, read only by `migrate_config` so it can carry
/// the fields that outlive the layout change over to version 2.
#[derive(AnchorSerialize, AnchorDeserialize)]
struct ConfigV1 {
    admin: Pubkey,
    #[allow(dead_code)]
    fee_bps: u16,
    #[allow(dead_code)]
    fixed_fee_lamports: u64,
    fee_mode: FeeMode,
    session_fee_float: u64,
    bump: u8,
    treasury_bump: u8,
    version: u8,
}

pub fn initialize_config_handler(
    ctx: Context<InitializeConfig>,
    policy: FeePolicy,
    fee_mode: FeeMode,
    session_fee_float: u64,
) -> Result<()> {
    // The signer must be the upgrade authority: `Signer` already proved the key
    // signed, so comparing addresses is the whole check.
    match upgrade_authority(&ctx.accounts.program_data.to_account_info())? {
        Some(authority) if authority == ctx.accounts.admin.key() => {}
        Some(_) => return Err(PaymentError::InvalidConfigAdmin.into()),
        None => return Err(PaymentError::ProgramNotUpgradeable.into()),
    }

    validate_fee_policy(&policy)?;
    validate_session_float(session_fee_float)?;

    let config = &mut ctx.accounts.config;
    config.admin = ctx.accounts.admin.key();
    config.policy = policy;
    config.fee_mode = fee_mode;
    config.session_fee_float = session_fee_float;
    config.bump = ctx.bumps.config;
    config.treasury_bump = ctx.bumps.treasury;
    config.version = CONFIG_VERSION;

    msg!(
        "config initialized: {} / {} bps at {} lamports and {} base units, mode {:?}, float {}",
        policy.fee_bps_below,
        policy.fee_bps_above,
        policy.sol_tier_lamports,
        policy.token_tier_base_units,
        fee_mode,
        session_fee_float
    );
    Ok(())
}

pub fn update_config_handler(
    ctx: Context<UpdateConfig>,
    policy: FeePolicy,
    fee_mode: FeeMode,
    session_fee_float: u64,
) -> Result<()> {
    validate_fee_policy(&policy)?;
    validate_session_float(session_fee_float)?;

    let config = &mut ctx.accounts.config;
    config.policy = policy;
    config.fee_mode = fee_mode;
    config.session_fee_float = session_fee_float;

    msg!(
        "config updated: {} / {} bps at {} lamports and {} base units, mode {:?}, float {}",
        policy.fee_bps_below,
        policy.fee_bps_above,
        policy.sol_tier_lamports,
        policy.token_tier_base_units,
        fee_mode,
        session_fee_float
    );
    emit!(ConfigUpdated {
        admin: config.admin,
        policy,
        fee_mode,
        session_fee_float,
    });
    Ok(())
}

/// Upgrades the live version 1 config account to version 2 in place.
///
/// Only the layout changes: the admin, the fee mode and the session float the
/// account already records are carried over, and the new `FeePolicy` is the one
/// the admin passes. The account grows, so the admin tops its rent up first —
/// the runtime refuses a realloc that leaves an account below its rent-exempt
/// minimum.
pub fn migrate_config_handler(ctx: Context<MigrateConfig>, policy: FeePolicy) -> Result<()> {
    validate_fee_policy(&policy)?;

    let info = ctx.accounts.config.to_account_info();
    require_keys_eq!(*info.owner, crate::ID, PaymentError::InvalidConfigAdmin);

    let (admin, fee_mode, session_fee_float, bump, treasury_bump) = {
        let data = info.try_borrow_data()?;
        require!(
            data.len() == CONFIG_SPACE_V1,
            PaymentError::InvalidConfigVersion
        );
        let old = ConfigV1::try_from_slice(&data[8..])
            .map_err(|_| error!(PaymentError::InvalidConfigVersion))?;
        require!(old.version == 1, PaymentError::InvalidConfigVersion);
        (
            old.admin,
            old.fee_mode,
            old.session_fee_float,
            old.bump,
            old.treasury_bump,
        )
    };

    require!(
        admin == ctx.accounts.admin.key(),
        PaymentError::InvalidConfigAdmin
    );
    validate_session_float(session_fee_float)?;

    // Grow first, then pay the rent the taller account needs. `realloc` keeps
    // the lamports the account already had, so only the difference is owed.
    info.resize(CONFIG_SPACE)?;
    let required = Rent::get()?.minimum_balance(CONFIG_SPACE);
    let held = info.lamports();
    if held < required {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                anchor_lang::system_program::ID,
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.admin.to_account_info(),
                    to: info.clone(),
                },
            ),
            required - held,
        )?;
    }

    let config = Config {
        admin,
        policy,
        fee_mode,
        session_fee_float,
        bump,
        treasury_bump,
        version: CONFIG_VERSION,
    };
    let mut body = Vec::with_capacity(CONFIG_SPACE - 8);
    config.serialize(&mut body)?;
    {
        let mut data = info.try_borrow_mut_data()?;
        require!(
            data.len() == CONFIG_SPACE,
            PaymentError::InvalidConfigVersion
        );
        data[8..].copy_from_slice(&body);
    }

    msg!(
        "config migrated to v{}: {} / {} bps at {} lamports and {} base units",
        CONFIG_VERSION,
        policy.fee_bps_below,
        policy.fee_bps_above,
        policy.sol_tier_lamports,
        policy.token_tier_base_units
    );
    Ok(())
}

pub fn withdraw_treasury_handler(ctx: Context<WithdrawTreasury>, amount: u64) -> Result<()> {
    let treasury = ctx.accounts.treasury.to_account_info();
    let floor = Rent::get()?.minimum_balance(0);
    let available = treasury.lamports().saturating_sub(floor);
    require!(amount <= available, PaymentError::TreasuryUnderfunded);

    pay_from_treasury(
        &treasury,
        &ctx.accounts.admin.to_account_info(),
        amount,
        ctx.accounts.config.treasury_bump,
    )?;

    msg!("treasury withdrew {} lamports", amount);
    Ok(())
}

#[derive(Accounts)]
pub struct WithdrawTreasury<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump, has_one = admin)]
    pub config: Account<'info, Config>,

    #[account(mut, seeds = [TREASURY_SEED], bump = config.treasury_bump)]
    pub treasury: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}
