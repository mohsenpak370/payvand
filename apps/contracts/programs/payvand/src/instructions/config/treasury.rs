//! Withdrawing the SOL treasury's surplus without starving token settlements.

use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::PaymentError;
use crate::helpers::pay_from_treasury;
use crate::state::Config;

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
