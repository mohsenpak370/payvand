//! Opening (or topping up) a token allowance: the one wallet signature a buyer
//! makes for it.
//!
//! No invoice is touched here. The buyer registers the session key, gives it
//! its fee float, and tops the vault up to the monthly limit.

use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
// Interface types and CPIs, not `Account<TokenAccount>`: see the module note
// in `token/mod.rs` on why that makes `Token2022Unsupported` the error a
// Token-2022 caller sees.
use anchor_spl::token_interface::{
    self, TokenAccount, TokenInterface, TransferChecked as TokenTransferChecked,
};

use crate::constants::*;
use crate::errors::PaymentError;
use crate::events::BudgetOpened;
use crate::helpers::{current_month_index, mint_decimals};
use crate::state::TokenBudget;

#[derive(Accounts)]
pub struct OpenTokenBudget<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,

    #[account(
        init_if_needed,
        payer = buyer,
        space = TOKEN_BUDGET_SPACE,
        seeds = [TOKEN_BUDGET_SEED, buyer.key().as_ref(), mint.key().as_ref()],
        bump
    )]
    pub budget: Account<'info, TokenBudget>,

    /// CHECK: The mint this budget is denominated in. Unchecked only to refuse
    /// a Token-2022 mint by name, as on the other token contexts.
    #[account(
        constraint = mint.owner == &anchor_spl::token::ID
            && mint.data_len() == SPL_MINT_LEN
            @ PaymentError::Token2022Unsupported
    )]
    pub mint: UncheckedAccount<'info>,

    /// Where the budget's tokens come from — the buyer's own account.
    #[account(mut, token::mint = mint, token::authority = buyer)]
    pub buyer_token_account: InterfaceAccount<'info, TokenAccount>,

    /// The pot itself: a token account owned by the budget PDA, so only this
    /// program can ever move what is inside it.
    #[account(
        init_if_needed,
        payer = buyer,
        associated_token::mint = mint,
        associated_token::authority = budget
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: App-generated session key. Receives the fee float here, and is
    /// the only key allowed to spend from this budget afterwards.
    #[account(mut)]
    pub session_key: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn open_token_budget_handler<'info, 'ctx>(
    ctx: Context<'ctx, OpenTokenBudget<'info>>,
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

    // The session key pays the network fee of every silent payment, so it
    // keeps a float just like it does for SOL.
    let float_gap = SESSION_FEE_FLOAT.saturating_sub(ctx.accounts.session_key.lamports());
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

    // The budget holds its own rent and nothing more: the pool refunds the
    // records its session key advances.
    let budget_rent = Rent::get()?.minimum_balance(TOKEN_BUDGET_SPACE);
    let wanted = budget_rent;
    let held = ctx.accounts.budget.to_account_info().lamports();
    if wanted > held {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                anchor_lang::system_program::ID,
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.buyer.to_account_info(),
                    to: ctx.accounts.budget.to_account_info(),
                },
            ),
            wanted - held,
        )?;
    }

    // Top the vault up to the limit: a returning buyer moves only the
    // difference between the limit and what the vault still holds.
    let held_tokens = ctx.accounts.vault.amount;
    let top_up = monthly_limit.saturating_sub(held_tokens);
    if top_up > 0 {
        let decimals = mint_decimals(&ctx.accounts.mint)?;
        token_interface::transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                TokenTransferChecked {
                    from: ctx.accounts.buyer_token_account.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.vault.to_account_info(),
                    authority: ctx.accounts.buyer.to_account_info(),
                },
            ),
            top_up,
            decimals,
        )?;
    }

    // Same rule as `open_budget`: a refill keeps the month's counter, so a
    // mid-month top-up raises the limit without granting the limit again.
    let now = current_month_index()?;
    let budget = &mut ctx.accounts.budget;
    budget.buyer = ctx.accounts.buyer.key();
    budget.session_key = session;
    budget.mint = ctx.accounts.mint.key();
    budget.monthly_limit = monthly_limit;
    if budget.month_index != now {
        budget.spent = 0;
        budget.month_index = now;
    }
    budget.bump = ctx.bumps.budget;

    msg!(
        "token budget opened: {} of {} for session {}",
        monthly_limit,
        ctx.accounts.mint.key(),
        session
    );
    emit!(BudgetOpened {
        buyer: ctx.accounts.buyer.key(),
        session_key: session,
        mint: ctx.accounts.mint.key(),
        monthly_limit,
    });
    Ok(())
}
