//! Closing a token allowance: whatever is left in the vault, and the vault's
//! rent, go back to the buyer, and the budget account closes with them.

use anchor_lang::prelude::*;
// Interface types and CPIs, not `Account<TokenAccount>`: see the module note
// in `token/mod.rs` on why that makes `Token2022Unsupported` the error a
// Token-2022 caller sees.
use anchor_spl::token_interface::{
    self, CloseAccount as TokenCloseAccount, TokenAccount, TokenInterface,
    TransferChecked as TokenTransferChecked,
};

use crate::constants::*;
use crate::errors::PaymentError;
use crate::helpers::mint_decimals;
use crate::state::TokenBudget;

#[derive(Accounts)]
pub struct CloseTokenBudget<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,

    #[account(
        mut,
        close = buyer,
        seeds = [TOKEN_BUDGET_SEED, buyer.key().as_ref(), budget.mint.as_ref()],
        bump = budget.bump
    )]
    pub budget: Account<'info, TokenBudget>,

    /// CHECK: The budget's mint, compared against the budget below. Unchecked
    /// only to refuse a Token-2022 mint by name, as on the other token contexts.
    #[account(
        constraint = mint.owner == &anchor_spl::token::ID
            && mint.data_len() == SPL_MINT_LEN
            @ PaymentError::Token2022Unsupported
    )]
    pub mint: UncheckedAccount<'info>,

    /// The canonical vault, as in `pay_from_token_budget`, so a decoy account
    /// that shares the mint and the budget's authority cannot be drained in its
    /// place.
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = budget
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    #[account(mut, token::mint = mint, token::authority = buyer)]
    pub buyer_token_account: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn close_token_budget_handler<'info, 'ctx>(
    ctx: Context<'ctx, CloseTokenBudget<'info>>,
) -> Result<()> {
    // The mint argument decides which account the vault claim is checked
    // against, so it has to be the budget's own: otherwise a caller names some
    // other mint and closes against a vault this budget never funded.
    require_keys_eq!(
        ctx.accounts.mint.key(),
        ctx.accounts.budget.mint,
        PaymentError::MintMismatch
    );

    let buyer = ctx.accounts.budget.buyer;
    let mint = ctx.accounts.budget.mint;
    let bump = ctx.accounts.budget.bump;
    let seeds: &[&[u8]] = &[TOKEN_BUDGET_SEED, buyer.as_ref(), mint.as_ref(), &[bump]];

    // Whatever is left in the vault goes back to the buyer's own account.
    let remaining = ctx.accounts.vault.amount;
    if remaining > 0 {
        let decimals = mint_decimals(&ctx.accounts.mint)?;
        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.key(),
                TokenTransferChecked {
                    from: ctx.accounts.vault.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.buyer_token_account.to_account_info(),
                    authority: ctx.accounts.budget.to_account_info(),
                },
                &[seeds],
            ),
            remaining,
            decimals,
        )?;
    }

    // Close the vault itself and send its rent to the buyer.
    token_interface::close_account(CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        TokenCloseAccount {
            account: ctx.accounts.vault.to_account_info(),
            destination: ctx.accounts.buyer.to_account_info(),
            authority: ctx.accounts.budget.to_account_info(),
        },
        &[seeds],
    ))?;

    msg!("token budget closed; vault returned to the buyer");
    Ok(())
}
