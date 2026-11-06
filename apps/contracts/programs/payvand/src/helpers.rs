//! Small pieces both payment paths need: the calendar month a budget resets
//! on, and the first-touch/settle bookkeeping a token invoice shares between
//! the wallet and allowance paths.

use anchor_lang::prelude::*;

use crate::errors::PaymentError;
use crate::state::{PaymentState, TokenPayment};

/// First touch of a token invoice: fill it in. An invoice that already exists
/// at this address is bound to this seller, reference, mint and amount by its
/// seeds, so the only question left is whether someone already paid it.
pub fn open_token_invoice<'info>(
    payment: &mut Account<'info, TokenPayment>,
    seller: Pubkey,
    reference: Pubkey,
    mint: Pubkey,
    amount: u64,
    memo: String,
    bump: u8,
) -> Result<()> {
    if payment.seller == Pubkey::default() {
        payment.seller = seller;
        payment.reference = reference;
        payment.mint = mint;
        payment.amount = amount;
        payment.memo = memo;
        payment.state = PaymentState::Pending;
        payment.payer = Pubkey::default();
        payment.created_at = Clock::get()?.unix_timestamp;
        payment.settled_at = 0;
        payment.bump = bump;
        Ok(())
    } else {
        require!(
            payment.state == PaymentState::Pending,
            PaymentError::AlreadySettled
        );
        Ok(())
    }
}

/// Mark a token invoice paid. Callers have already moved the tokens.
pub fn settle_token_invoice<'info>(
    payment: &mut Account<'info, TokenPayment>,
    payer: Pubkey,
) -> Result<()> {
    payment.state = PaymentState::Settled;
    payment.payer = payer;
    payment.settled_at = Clock::get()?.unix_timestamp;
    Ok(())
}

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
