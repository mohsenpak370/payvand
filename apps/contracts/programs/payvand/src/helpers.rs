//! Small pieces both payment paths need: the calendar month a budget resets
//! on, the fee a settlement owes, and the treasury moves that pay for it.

use anchor_lang::prelude::*;

use crate::constants::{MIN_SESSION_FEE_FLOAT, TREASURY_SEED};
use crate::errors::PaymentError;
use crate::state::Config;

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
/// Capped at `amount - 1` so the underflow guard (`amount > rent + fee`) can
/// never be satisfied by taking the whole invoice; the cap is a safety net, not
/// a policy — a sane `fee_bps` stays far below it.
pub fn fee_for(config: &Config, amount: u64) -> Result<u64> {
    let proportional = (amount as u128)
        .checked_mul(config.fee_bps as u128)
        .ok_or(PaymentError::FeeOverflow)?
        / 10_000;
    let fee = (proportional as u64).saturating_add(config.fixed_fee_lamports);
    Ok(fee.min(amount.saturating_sub(1)))
}

/// Policy validation shared by `initialize_config` and `update_config`.
pub fn validate_fee_policy(
    fee_bps: u16,
    _fixed_fee_lamports: u64,
    session_fee_float: u64,
) -> Result<()> {
    require!(fee_bps <= 10_000, PaymentError::InvalidFeeBps);
    require!(
        session_fee_float >= MIN_SESSION_FEE_FLOAT,
        PaymentError::InvalidSessionFloat
    );
    Ok(())
}

/// Refuses to settle unless the treasury can refund a receipt rent and keep
/// its own rent-exempt minimum, so a settle never leaves the treasury short.
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
