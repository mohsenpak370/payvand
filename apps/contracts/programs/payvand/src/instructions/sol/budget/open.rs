//! Opening (or topping up) the SOL allowance: the one wallet signature a buyer
//! makes for it.
//!
//! No invoice is touched here. The buyer registers the session key, gives it
//! its fee float, and tops the pot up to the monthly limit.

use anchor_lang::prelude::*;

use crate::constants::{BUDGET_SEED, BUDGET_SPACE, CONFIG_SEED};
use crate::errors::PaymentError;
use crate::events::BudgetOpened;
use crate::helpers::current_month_index;
use crate::state::{Budget, Config};

#[derive(Accounts)]
pub struct OpenBudget<'info> {
    /// Main wallet: the one signature the buyer ever makes.
    #[account(mut)]
    pub buyer: Signer<'info>,

    #[account(
        init_if_needed,
        payer = buyer,
        space = BUDGET_SPACE,
        seeds = [BUDGET_SEED, buyer.key().as_ref()],
        bump
    )]
    pub budget: Account<'info, Budget>,

    /// CHECK: App-generated session key. Receives the fee float here, and is
    /// the only key allowed to spend from the budget afterwards.
    #[account(mut)]
    pub session_key: UncheckedAccount<'info>,

    /// Supplies the session float, so a policy change reaches every new budget.
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    pub system_program: Program<'info, System>,
}

pub fn open_budget_handler<'info, 'ctx>(
    ctx: Context<'ctx, OpenBudget<'info>>,
    monthly_limit: u64,
) -> Result<()> {
    require!(monthly_limit > 0, PaymentError::InvalidAmount);
    let session = ctx.accounts.session_key.key();
    require_keys_neq!(
        session,
        ctx.accounts.buyer.key(),
        PaymentError::InvalidSessionKey
    );
    require_keys_neq!(session, Pubkey::default(), PaymentError::InvalidSessionKey);

    let float_gap = ctx
        .accounts
        .config
        .session_fee_float
        .saturating_sub(ctx.accounts.session_key.lamports());
    if float_gap > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                anchor_lang::system_program::ID,
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.buyer.to_account_info(),
                    to: ctx.accounts.session_key.to_account_info(),
                },
            ),
            float_gap,
        )?;
    }

    // Top the pot up to the limit: a returning buyer pays only the
    // difference between the limit and what is left unspent.
    let rent_min = Rent::get()?.minimum_balance(BUDGET_SPACE);
    let pot = ctx
        .accounts
        .budget
        .to_account_info()
        .lamports()
        .saturating_sub(rent_min);
    let top_up = monthly_limit.saturating_sub(pot);
    if top_up > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                anchor_lang::system_program::ID,
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.buyer.to_account_info(),
                    to: ctx.accounts.budget.to_account_info(),
                },
            ),
            top_up,
        )?;
    }

    // A refill is this same instruction on the same account
    // (`init_if_needed`), so the counter only starts over when the calendar
    // month has turned: raising the limit mid-month must not hand back what was
    // already spent, or a buyer could refill their way past their own limit.
    let now = current_month_index()?;
    let budget = &mut ctx.accounts.budget;
    budget.buyer = ctx.accounts.buyer.key();
    budget.session_key = session;
    budget.monthly_limit = monthly_limit;
    if budget.month_index != now {
        budget.spent = 0;
        budget.month_index = now;
    }
    // close_budget validates `bump = budget.bump`; without this the
    // stored bump stays 0 and closing can never pass its seeds check.
    budget.bump = ctx.bumps.budget;

    msg!(
        "budget opened: {} lamports a month for session {}",
        monthly_limit,
        session
    );
    emit!(BudgetOpened {
        buyer: ctx.accounts.buyer.key(),
        session_key: session,
        mint: Pubkey::default(),
        monthly_limit,
    });
    Ok(())
}
