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
use crate::helpers::{
    ensure_anchor_band, flat_for_rent, pay_from_treasury, validate_fee_policy, validate_price,
    validate_session_float,
};
use crate::state::{Config, FeeAnchor, FeeMode, FeePolicy};

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

/// Re-pegs the flats to a live price a permissionless caller supplies, within a
/// band measured from the admin's anchor.
#[derive(Accounts)]
pub struct RefreshFeePrice<'info> {
    /// Anyone may call this and pays the network fee for it; the bounds inside
    /// are the protection, not the key. The app refreshes the flats from its
    /// rate feed on the same five-minute cache it prices invoices with.
    pub caller: Signer<'info>,

    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    /// The admin's price the supplied one is measured against. Required rather
    /// than optional: a deployment whose admin has not priced it yet has no
    /// band, so there is nothing to refresh within.
    #[account(seeds = [FEE_ANCHOR_SEED], bump = anchor.bump)]
    pub anchor: Account<'info, FeeAnchor>,
}

#[derive(Accounts)]
pub struct SetFeePriceAnchor<'info> {
    /// The recorded admin: the only key whose price the band is measured from.
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump, has_one = admin)]
    pub config: Account<'info, Config>,

    /// Created on the first call and rewritten on every later one.
    #[account(
        init_if_needed,
        payer = admin,
        space = FEE_ANCHOR_SPACE,
        seeds = [FEE_ANCHOR_SEED],
        bump
    )]
    pub anchor: Account<'info, FeeAnchor>,

    pub system_program: Program<'info, System>,
}

/// Upgrades the version 1 or 2 config account in place.
///
/// The account cannot be typed as `Account<'info, Config>`: it is still the old,
/// shorter layout, and deserializing it as `Config` would fail before the
/// handler could migrate it. It is a raw account here, pinned to the config seed
/// and checked by hand: the program must own it, `admin` must be the admin it
/// records, and it must be a version 1 or 2 account of exactly its layout's
/// length. Anything else is refused rather than overwritten.
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

/// The `FeePolicy` as version 2 and version 3 wrote it: the two rates, their two
/// boundaries, the two token flats and the two mints — no network fee. Kept
/// separate from `FeePolicy` because those accounts are still on chain in that
/// shape and `migrate_config` has to read them.
#[derive(AnchorSerialize, AnchorDeserialize)]
struct FeePolicyV3 {
    fee_bps_below: u16,
    fee_bps_above: u16,
    sol_tier_lamports: u64,
    token_tier_base_units: u64,
    flat_usdc_base_units: u64,
    flat_eurc_base_units: u64,
    usdc_mint: Pubkey,
    eurc_mint: Pubkey,
}

/// The version 3 `Config` body — everything version 4 has but the network fee.
/// Read only by `migrate_config`, which carries the fields over and zeroes the
/// price-derived ones so the next anchor reprices them.
#[derive(AnchorSerialize, AnchorDeserialize)]
struct ConfigV3 {
    admin: Pubkey,
    policy: FeePolicyV3,
    fee_mode: FeeMode,
    session_fee_float: u64,
    #[allow(dead_code)]
    flats_updated_at: i64,
    bump: u8,
    treasury_bump: u8,
    #[allow(dead_code)]
    version: u8,
}

/// The version 2 `Config` body — everything version 3 has but the refresh
/// cooldown. Read only by `migrate_config`, which carries the fields over and
/// leaves `flats_updated_at` at zero so the first refresh need not wait.
#[derive(AnchorSerialize, AnchorDeserialize)]
struct ConfigV2 {
    admin: Pubkey,
    policy: FeePolicyV3,
    fee_mode: FeeMode,
    session_fee_float: u64,
    bump: u8,
    treasury_bump: u8,
    #[allow(dead_code)]
    version: u8,
}

/// The version 1 `Config` body, read only by `migrate_config` so it can carry
/// the fields that outlive the layout change over to version 3.
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
    // No refresh has happened yet, and the first one should not have to wait for
    // a cooldown that guards against a caller the config has never had.
    config.flats_updated_at = 0;
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
    // A policy written by the admin counts as freshly priced, so
    // `refresh_fee_price` cannot immediately overwrite the flats the admin just
    // chose. Zero rather than the clock would let it, and the admin's own number
    // is the more deliberate one.
    config.flats_updated_at = Clock::get()?.unix_timestamp;

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

/// Re-pegs the two token flats to a live SOL price.
///
/// A token settle cannot charge its seller lamports, so the marker's rent is
/// collected in the currency as `flat_usdc_base_units` / `flat_eurc_base_units`.
/// Those numbers are only true at the SOL price they were written at: SOL moves,
/// the rent does not, and a flat left alone drifts — charging a token seller for
/// a record that has become cheaper in their currency, or leaving the treasury
/// to fund the difference once SOL has risen. This recomputes both from the
/// cluster's own rent and a price the caller supplies, so the only price in the
/// fee is the one passed here.
///
/// Permissionless on purpose: the app is already the party that fetches rates,
/// and requiring the admin key would put a second copy of it in a server's
/// environment. What keeps it safe is inside — a price must be plausible, it must
/// sit within `PRICE_ANCHOR_BAND_BPS` of the price the admin anchored, and a
/// second call within `FLATS_REFRESH_SECS` is refused. The band is measured from
/// the admin's anchor, not from the flat this call would rewrite, so a caller
/// cannot compound one accepted move into the next: however many refreshes run,
/// the flats stay within that band of the admin's price.
pub fn refresh_fee_price_handler(
    ctx: Context<RefreshFeePrice>,
    sol_usd_micros: u64,
    sol_eur_micros: u64,
) -> Result<()> {
    validate_price(sol_usd_micros)?;
    validate_price(sol_eur_micros)?;

    let receipt_rent = Rent::get()?.minimum_balance(PAYMENT_RECEIPT_SPACE);
    let now = Clock::get()?.unix_timestamp;

    let anchor = &ctx.accounts.anchor;
    ensure_anchor_band(anchor.sol_usd_micros, sol_usd_micros)?;
    ensure_anchor_band(anchor.sol_eurc_micros, sol_eur_micros)?;

    let config = &mut ctx.accounts.config;

    if config.flats_updated_at != 0 {
        require!(
            now >= config.flats_updated_at.saturating_add(FLATS_REFRESH_SECS),
            PaymentError::FeePriceTooSoon
        );
    }

    config.policy.flat_usdc_base_units = flat_for_rent(receipt_rent, sol_usd_micros)?;
    config.policy.flat_eurc_base_units = flat_for_rent(receipt_rent, sol_eur_micros)?;
    // The network fee is lamports, and a token seller is charged its equivalent
    // at this same price — the two costs a token payment carries move together,
    // so they are priced from one number.
    let network_fee = config.policy.network_fee_lamports;
    config.policy.network_fee_usdc_base_units = flat_for_rent(network_fee, sol_usd_micros)?;
    config.policy.network_fee_eurc_base_units = flat_for_rent(network_fee, sol_eur_micros)?;
    config.flats_updated_at = now;

    msg!(
        "flats refreshed at {} / {} micros: {} USDC and {} EURC base units, network fee {} / {}",
        sol_usd_micros,
        sol_eur_micros,
        config.policy.flat_usdc_base_units,
        config.policy.flat_eurc_base_units,
        config.policy.network_fee_usdc_base_units,
        config.policy.network_fee_eurc_base_units
    );
    emit!(ConfigUpdated {
        admin: config.admin,
        policy: config.policy,
        fee_mode: config.fee_mode,
        session_fee_float: config.session_fee_float,
    });
    Ok(())
}

/// Writes the price the permissionless `refresh_fee_price` is measured against,
/// and prices the flats at it in the same call.
///
/// Admin only, like every other policy write: the band exists so a caller cannot
/// choose the price, which means the price has to come from somewhere only the
/// admin can write. Setting it here also writes both flats from the cluster's
/// current rent at that price, so a deliberate admin re-peg takes effect without
/// waiting for the cooldown or for a second transaction, and the anchor is never
/// out of step with what the policy charges.
pub fn set_fee_price_anchor_handler(
    ctx: Context<SetFeePriceAnchor>,
    sol_usd_micros: u64,
    sol_eur_micros: u64,
) -> Result<()> {
    validate_price(sol_usd_micros)?;
    validate_price(sol_eur_micros)?;

    let receipt_rent = Rent::get()?.minimum_balance(PAYMENT_RECEIPT_SPACE);
    let config = &mut ctx.accounts.config;
    config.policy.flat_usdc_base_units = flat_for_rent(receipt_rent, sol_usd_micros)?;
    config.policy.flat_eurc_base_units = flat_for_rent(receipt_rent, sol_eur_micros)?;
    let network_fee = config.policy.network_fee_lamports;
    config.policy.network_fee_usdc_base_units = flat_for_rent(network_fee, sol_usd_micros)?;
    config.policy.network_fee_eurc_base_units = flat_for_rent(network_fee, sol_eur_micros)?;
    config.flats_updated_at = Clock::get()?.unix_timestamp;

    let anchor = &mut ctx.accounts.anchor;
    anchor.sol_usd_micros = sol_usd_micros;
    anchor.sol_eurc_micros = sol_eur_micros;
    anchor.bump = ctx.bumps.anchor;

    msg!(
        "fee price anchored at {} / {} micros: {} USDC and {} EURC base units",
        sol_usd_micros,
        sol_eur_micros,
        config.policy.flat_usdc_base_units,
        config.policy.flat_eurc_base_units
    );
    emit!(ConfigUpdated {
        admin: config.admin,
        policy: config.policy,
        fee_mode: config.fee_mode,
        session_fee_float: config.session_fee_float,
    });
    Ok(())
}

/// Upgrades the live config account — version 1, 2 or 3 — to version 4 in
/// place.
///
/// Only the layout changes: the admin, the fee mode and the session float the
/// account already records are carried over, and the new `FeePolicy` is the one
/// the admin passes — bar its four price-derived fields (the two token flats and
/// the network fee's two token equivalents), which are zeroed so the next
/// `set_fee_price_anchor` prices them from the cluster's rent and the admin's
/// own rate.
///
/// The flats are zeroed on purpose. Whatever numbers the old account (or the
/// admin's arguments) carried were priced for a rent and a SOL rate from before
/// this upgrade, and a stale floor would overcharge or undercharge every token
/// seller until the admin re-pegs it. Zero is the program's "not priced yet",
/// and the admin's next `set_fee_price_anchor` writes both flats from the
/// current rent and its own price; `flats_updated_at` also starts at zero, so
/// nothing waits for a cooldown either.
///
/// A deployment migrated by this instruction has no price anchor yet — that
/// account postdates the layouts this migration upgrades — so the admin's
/// `set_fee_price_anchor` is required before any permissionless
/// `refresh_fee_price` can run. Between the migration and that call a token
/// settlement is charged the percentage alone, with no record floor, so anchor
/// the price immediately afterwards, before taking a token payment. The account
/// grows to `CONFIG_SPACE`, so the admin tops its rent up first — the runtime
/// refuses a realloc that leaves an account below its rent-exempt minimum.
pub fn migrate_config_handler(ctx: Context<MigrateConfig>, policy: FeePolicy) -> Result<()> {
    validate_fee_policy(&policy)?;
    // The four price-derived fields are zeroed: whatever the old account or the
    // admin's arguments carried was priced for a rent and a SOL rate from before
    // this upgrade. `network_fee_lamports` is not price-derived — it is what one
    // signature costs — so it is carried over from the policy as passed.
    let policy = FeePolicy {
        flat_usdc_base_units: 0,
        flat_eurc_base_units: 0,
        network_fee_usdc_base_units: 0,
        network_fee_eurc_base_units: 0,
        ..policy
    };

    let info = ctx.accounts.config.to_account_info();
    require_keys_eq!(*info.owner, crate::ID, PaymentError::InvalidConfigAdmin);

    let (admin, fee_mode, session_fee_float, bump, treasury_bump) = {
        let data = info.try_borrow_data()?;
        match data.len() {
            CONFIG_SPACE_V3 => {
                let old = ConfigV3::try_from_slice(&data[8..])
                    .map_err(|_| error!(PaymentError::InvalidConfigVersion))?;
                require!(old.version == 3, PaymentError::InvalidConfigVersion);
                (
                    old.admin,
                    old.fee_mode,
                    old.session_fee_float,
                    old.bump,
                    old.treasury_bump,
                )
            }
            CONFIG_SPACE_V1 => {
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
            }
            CONFIG_SPACE_V2 => {
                let old = ConfigV2::try_from_slice(&data[8..])
                    .map_err(|_| error!(PaymentError::InvalidConfigVersion))?;
                require!(old.version == 2, PaymentError::InvalidConfigVersion);
                (
                    old.admin,
                    old.fee_mode,
                    old.session_fee_float,
                    old.bump,
                    old.treasury_bump,
                )
            }
            _ => return Err(PaymentError::InvalidConfigVersion.into()),
        }
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
        flats_updated_at: 0,
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

/// Moves treasury surplus to the admin, keeping enough behind for the refunds a
/// token settlement draws from the pool.
///
/// The treasury already holds the rent-exempt minimums of the records it has
/// funded; what it must not lose is the next one. A withdrawal that stopped at
/// the empty account's own rent would leave the pool short of what
/// `ensure_treasury_funded` refunds — the token invoice's rent and the marker's —
/// and every token payment would be refused until someone topped it up, so the
/// reserve is that refund plus the account's rent. It is a floor on the
/// withdrawal, not on the treasury: a settlement may still spend it, and then
/// the guard refuses the next one until the pool is refunded.
pub fn withdraw_treasury_handler(ctx: Context<WithdrawTreasury>, amount: u64) -> Result<()> {
    let treasury = ctx.accounts.treasury.to_account_info();
    let rent = Rent::get()?;
    // The refund a token settlement draws is both rents plus the network fee it
    // reimburses the sponsor, so the reserve has to carry all three.
    let settlement_refund = rent
        .minimum_balance(TOKEN_PAYMENT_SPACE)
        .saturating_add(rent.minimum_balance(PAYMENT_RECEIPT_SPACE))
        .saturating_add(ctx.accounts.config.policy.network_fee_lamports);
    let reserve = settlement_refund
        .saturating_mul(TREASURY_RESERVE_SETTLEMENTS)
        .saturating_add(rent.minimum_balance(0));
    let available = treasury.lamports().saturating_sub(reserve);
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
