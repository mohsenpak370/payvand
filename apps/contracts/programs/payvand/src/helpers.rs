//! Small pieces both payment paths need: the calendar month a budget resets
//! on, the fee a settlement owes, and the treasury moves that pay for it.

use anchor_lang::prelude::*;

use crate::constants::{MIN_SESSION_FEE_FLOAT, TREASURY_SEED};
use crate::errors::PaymentError;
use crate::state::{Config, FeePolicy};

/// `year * 12 + month` (month 1-based) for the current clock, so a monthly
/// allowance resets on a real calendar-month boundary rather than every 30
/// days. Howard Hinnant's civil-from-days, without pulling in a date crate.
pub fn current_month_index() -> Result<u64> {
    let ts = Clock::get()?.unix_timestamp.max(0) as u64;
    let z = (ts / 86_400) as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    Ok(year as u64 * 12 + month as u64)
}

/// The fee a settlement of `amount` owes under `config`.
///
/// `mint` is `None` on the SOL paths and the invoice's mint on the token ones.
/// The rate is marginal: the part of the amount at or below the currency's
/// boundary pays `fee_bps_below`, the part above it pays `fee_bps_above`, so the
/// boundary is a change of slope and not a cliff — an invoice crossing it costs
/// one lamport's worth of rate more, never twice the fee.
///
/// A token invoice then adds its currency's flat, which is what pays the
/// permanent marker's rent that the treasury advances on that path. A SOL
/// invoice adds nothing: its seller already carries that rent directly.
///
/// Capped at `amount - 1` so the guard on the price (`amount > rent + fee`) can
/// never be satisfied by taking the whole invoice; the cap is a safety net, not
/// a policy.
pub fn fee_for(config: &Config, amount: u64, mint: Option<&Pubkey>) -> Result<u64> {
    let policy = &config.policy;
    let (boundary, flat) = match mint {
        None => (policy.sol_tier_lamports as u128, 0u128),
        Some(mint) if *mint == policy.usdc_mint => (
            policy.token_tier_base_units as u128,
            policy.flat_usdc_base_units as u128,
        ),
        Some(mint) if *mint == policy.eurc_mint => (
            policy.token_tier_base_units as u128,
            policy.flat_eurc_base_units as u128,
        ),
        Some(_) => (policy.token_tier_base_units as u128, 0u128),
    };

    let amount = amount as u128;
    let below = amount.min(boundary);
    let above = amount.saturating_sub(boundary);
    let proportional = below
        .checked_mul(policy.fee_bps_below as u128)
        .and_then(|first| {
            above
                .checked_mul(policy.fee_bps_above as u128)
                .and_then(|second| first.checked_add(second))
        })
        .ok_or(PaymentError::FeeOverflow)?
        / 10_000;

    let fee = proportional.saturating_add(flat);
    Ok(fee.min(amount.saturating_sub(1)) as u64)
}

/// Policy validation shared by `initialize_config`, `update_config` and
/// `migrate_config`.
pub fn validate_fee_policy(policy: &FeePolicy) -> Result<()> {
    require!(
        policy.fee_bps_below <= 10_000 && policy.fee_bps_above <= 10_000,
        PaymentError::InvalidFeeBps
    );
    Ok(())
}

/// The session float's floor, shared by the same three instructions.
pub fn validate_session_float(session_fee_float: u64) -> Result<()> {
    require!(
        session_fee_float >= MIN_SESSION_FEE_FLOAT,
        PaymentError::InvalidSessionFloat
    );
    Ok(())
}

/// Refuses to settle unless the treasury can fund the records it is about to
/// refund and keep its own rent-exempt minimum, so a settle never leaves the
/// treasury short.
///
/// Only the token paths call this since revision 23: a token settlement cannot
/// charge its non-signing seller lamports, so the treasury still funds the
/// marker's rent there. The SOL paths pay it out of the seller's proceeds and
/// ask the treasury for nothing.
pub fn ensure_treasury_funded(treasury: &AccountInfo, receipt_rent: u64) -> Result<()> {
    let floor = Rent::get()?.minimum_balance(0);
    require!(
        treasury.lamports() >= receipt_rent.saturating_add(floor),
        PaymentError::TreasuryUnderfunded
    );
    Ok(())
}

/// Moves lamports out of the treasury. The treasury is a system account, so the
/// program cannot subtract its lamports directly; it signs a system transfer
/// with the seed it was derived from instead.
pub fn pay_from_treasury<'info>(
    treasury: &AccountInfo<'info>,
    destination: &AccountInfo<'info>,
    amount: u64,
    bump: u8,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    anchor_lang::system_program::transfer(
        CpiContext::new_with_signer(
            anchor_lang::system_program::ID,
            anchor_lang::system_program::Transfer {
                from: treasury.clone(),
                to: destination.clone(),
            },
            &[&[TREASURY_SEED, &[bump][..]]],
        ),
        amount,
    )
}
