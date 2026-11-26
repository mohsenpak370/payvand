//! Closing the SOL allowance: the budget account closes to the buyer, so
//! whatever is left in the pot, and its rent, go back to the wallet.

use anchor_lang::prelude::*;

use crate::constants::BUDGET_SEED;
use crate::state::Budget;

#[derive(Accounts)]
pub struct CloseBudget<'info> {
    #[account(
        mut,
        close = buyer,
        seeds = [BUDGET_SEED, buyer.key().as_ref()],
        bump = budget.bump
    )]
    pub budget: Account<'info, Budget>,

    #[account(mut)]
    pub buyer: Signer<'info>,
}

pub fn close_budget_handler<'info, 'ctx>(_ctx: Context<'ctx, CloseBudget<'info>>) -> Result<()> {
    msg!("budget closed; remaining lamports returned to the buyer");
    Ok(())
}
