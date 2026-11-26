//! In-place upgrade of version 1–3 config accounts to the current layout.

use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::PaymentError;
use crate::helpers::{validate_fee_policy, validate_session_float};
use crate::state::{Config, FeeMode, FeePolicy};

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
