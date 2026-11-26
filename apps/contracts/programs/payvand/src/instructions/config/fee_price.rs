//! The admin's price anchor and the bounded, permissionless flat refresh.

use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::PaymentError;
use crate::events::ConfigUpdated;
use crate::helpers::{ensure_anchor_band, flat_for_rent, validate_price};
use crate::state::{Config, FeeAnchor};

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
