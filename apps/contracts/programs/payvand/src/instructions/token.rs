//! Token invoices and the per-mint monthly allowance.

use anchor_lang::prelude::*;
use anchor_lang::Lamports;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer as TokenTransfer};

use crate::constants::*;
use crate::errors::PaymentError;
use crate::helpers::{current_month_index, open_token_invoice, settle_token_invoice};
use crate::state::{TokenBudget, TokenPayment};

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct SettleTokenPayment<'info> {
    /// Pays the invoice rent, the seller's token account rent if it does not
    /// exist yet, and the fee. The tokens themselves come out of
    /// `payer_token_account`.
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init_if_needed,
        payer = payer,
        space = TOKEN_PAYMENT_SPACE,
        seeds = [
            TOKEN_PAYMENT_SEED,
            seller.key().as_ref(),
            reference.key().as_ref(),
            mint.key().as_ref(),
            amount.to_le_bytes().as_ref()
        ],
        bump
    )]
    pub payment: Account<'info, TokenPayment>,

    /// CHECK: The address the QR code names as the payee. It receives only
    /// through its own associated token account, checked below.
    pub seller: UncheckedAccount<'info>,

    /// Created on the fly when the seller has never held this token, so a
    /// buyer is never blocked by the recipient's account setup.
    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = seller
    )]
    pub seller_token_account: Account<'info, TokenAccount>,

    #[account(mut, token::mint = mint, token::authority = payer)]
    pub payer_token_account: Account<'info, TokenAccount>,

    pub mint: Account<'info, Mint>,

    /// CHECK: Arbitrary 32-byte value chosen when the QR was made; a PDA seed
    /// and an address watchers can look up.
    pub reference: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct CloseTokenPayment<'info> {
    #[account(
        mut,
        seeds = [
            TOKEN_PAYMENT_SEED,
            payment.seller.as_ref(),
            payment.reference.as_ref(),
            payment.mint.as_ref(),
            amount.to_le_bytes().as_ref()
        ],
        bump = payment.bump,
        close = seller
    )]
    pub payment: Account<'info, TokenPayment>,

    #[account(mut)]
    pub seller: Signer<'info>,
}

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

    pub mint: Account<'info, Mint>,

    /// Where the budget's tokens come from — the buyer's own account.
    #[account(mut, token::mint = mint, token::authority = buyer)]
    pub buyer_token_account: Account<'info, TokenAccount>,

    /// The pot itself: a token account owned by the budget PDA, so only this
    /// program can ever move what is inside it.
    #[account(
        init_if_needed,
        payer = buyer,
        associated_token::mint = mint,
        associated_token::authority = budget
    )]
    pub vault: Account<'info, TokenAccount>,

    /// CHECK: App-generated session key. Receives the fee float here, and is
    /// the only key allowed to spend from this budget afterwards.
    #[account(mut)]
    pub session_key: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct PayFromTokenBudget<'info> {
    /// Session key: fee payer and invoice-rent payer (refunded from the
    /// budget's float in this same transaction). Never the buyer's wallet.
    #[account(mut)]
    pub session_key: Signer<'info>,

    #[account(
        mut,
        seeds = [TOKEN_BUDGET_SEED, budget.buyer.as_ref(), budget.mint.as_ref()],
        bump = budget.bump
    )]
    pub budget: Account<'info, TokenBudget>,

    #[account(
        mut,
        token::mint = mint,
        token::authority = budget
    )]
    pub vault: Account<'info, TokenAccount>,

    pub mint: Account<'info, Mint>,

    /// CHECK: The address the QR code names as the payee.
    pub seller: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = session_key,
        associated_token::mint = mint,
        associated_token::authority = seller
    )]
    pub seller_token_account: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = session_key,
        space = TOKEN_PAYMENT_SPACE,
        seeds = [
            TOKEN_PAYMENT_SEED,
            seller.key().as_ref(),
            reference.key().as_ref(),
            mint.key().as_ref(),
            amount.to_le_bytes().as_ref()
        ],
        bump
    )]
    pub payment: Account<'info, TokenPayment>,

    /// CHECK: Arbitrary 32-byte value chosen when the QR was made.
    pub reference: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

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

    pub mint: Account<'info, Mint>,

    #[account(mut, token::mint = mint, token::authority = budget)]
    pub vault: Account<'info, TokenAccount>,

    #[account(mut, token::mint = mint, token::authority = buyer)]
    pub buyer_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn settle_token_payment_handler<'info, 'ctx>(
    ctx: Context<'ctx, SettleTokenPayment<'info>>,
    amount: u64,
    memo: String,
) -> Result<()> {
    require!(amount > 0, PaymentError::InvalidAmount);
    require!(
        memo.as_bytes().len() <= PAYMENT_MAX_MEMO,
        PaymentError::MemoTooLong
    );
    require_keys_neq!(
        ctx.accounts.seller.key(),
        Pubkey::default(),
        PaymentError::InvalidSeller
    );

    open_token_invoice(
        &mut ctx.accounts.payment,
        ctx.accounts.seller.key(),
        ctx.accounts.reference.key(),
        ctx.accounts.mint.key(),
        amount,
        memo,
        ctx.bumps.payment,
    )?;

    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            TokenTransfer {
                from: ctx.accounts.payer_token_account.to_account_info(),
                to: ctx.accounts.seller_token_account.to_account_info(),
                authority: ctx.accounts.payer.to_account_info(),
            },
        ),
        amount,
    )?;

    settle_token_invoice(&mut ctx.accounts.payment, ctx.accounts.payer.key())?;

    msg!(
        "token payment settled: {} of {} from {}",
        amount,
        ctx.accounts.mint.key(),
        ctx.accounts.payer.key()
    );
    Ok(())
}

pub fn close_token_payment_handler<'info, 'ctx>(
    _ctx: Context<'ctx, CloseTokenPayment<'info>>,
    amount: u64,
) -> Result<()> {
    msg!("token payment closed: {} base units invoice", amount);
    Ok(())
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

    // The budget account itself refunds the invoice rent its session key
    // advances, so it holds a float on top of its own rent.
    let budget_rent = Rent::get()?.minimum_balance(TOKEN_BUDGET_SPACE);
    let wanted = budget_rent.saturating_add(TOKEN_BUDGET_FEE_FLOAT);
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
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                TokenTransfer {
                    from: ctx.accounts.buyer_token_account.to_account_info(),
                    to: ctx.accounts.vault.to_account_info(),
                    authority: ctx.accounts.buyer.to_account_info(),
                },
            ),
            top_up,
        )?;
    }

    let budget = &mut ctx.accounts.budget;
    budget.buyer = ctx.accounts.buyer.key();
    budget.session_key = session;
    budget.mint = ctx.accounts.mint.key();
    budget.monthly_limit = monthly_limit;
    budget.spent = 0;
    budget.month_index = current_month_index()?;
    budget.bump = ctx.bumps.budget;
    Ok(())
}

pub fn pay_from_token_budget_handler<'info, 'ctx>(
    ctx: Context<'ctx, PayFromTokenBudget<'info>>,
    amount: u64,
    memo: String,
) -> Result<()> {
    require!(amount > 0, PaymentError::InvalidAmount);
    require!(
        memo.as_bytes().len() <= PAYMENT_MAX_MEMO,
        PaymentError::MemoTooLong
    );
    require_keys_neq!(
        ctx.accounts.seller.key(),
        Pubkey::default(),
        PaymentError::InvalidSeller
    );
    require_keys_eq!(
        ctx.accounts.session_key.key(),
        ctx.accounts.budget.session_key,
        PaymentError::InvalidSessionKey
    );
    require_keys_eq!(
        ctx.accounts.mint.key(),
        ctx.accounts.budget.mint,
        PaymentError::MintMismatch
    );

    let now = current_month_index()?;
    let (monthly_limit, mut spent, month_index) = {
        let b = &ctx.accounts.budget;
        (b.monthly_limit, b.spent, b.month_index)
    };
    if month_index != now {
        spent = 0;
    }
    require!(
        spent.saturating_add(amount) <= monthly_limit,
        PaymentError::BudgetExhausted
    );
    require!(
        ctx.accounts.vault.amount >= amount,
        PaymentError::InsufficientBudget
    );

    open_token_invoice(
        &mut ctx.accounts.payment,
        ctx.accounts.seller.key(),
        ctx.accounts.reference.key(),
        ctx.accounts.mint.key(),
        amount,
        memo,
        ctx.bumps.payment,
    )?;

    // Vault → seller, signed by the budget PDA.
    let buyer = ctx.accounts.budget.buyer;
    let mint = ctx.accounts.budget.mint;
    let bump = ctx.accounts.budget.bump;
    let seeds: &[&[u8]] = &[TOKEN_BUDGET_SEED, buyer.as_ref(), mint.as_ref(), &[bump]];
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            TokenTransfer {
                from: ctx.accounts.vault.to_account_info(),
                to: ctx.accounts.seller_token_account.to_account_info(),
                authority: ctx.accounts.budget.to_account_info(),
            },
            &[seeds],
        ),
        amount,
    )?;

    settle_token_invoice(&mut ctx.accounts.payment, ctx.accounts.session_key.key())?;

    // Refund the invoice rent the session key just advanced, out of the
    // float the budget keeps for exactly this.
    let invoice_rent = Rent::get()?.minimum_balance(TOKEN_PAYMENT_SPACE);
    let budget_rent = Rent::get()?.minimum_balance(TOKEN_BUDGET_SPACE);
    let spare = ctx
        .accounts
        .budget
        .to_account_info()
        .lamports()
        .saturating_sub(budget_rent);
    let refund = invoice_rent.min(spare);
    if refund > 0 {
        ctx.accounts.budget.to_account_info().sub_lamports(refund)?;
        ctx.accounts
            .session_key
            .to_account_info()
            .add_lamports(refund)?;
    }

    let budget = &mut ctx.accounts.budget;
    budget.spent = spent.saturating_add(amount);
    budget.month_index = now;

    msg!(
        "token payment settled from budget: {} of {}, session {}",
        amount,
        mint,
        ctx.accounts.session_key.key()
    );
    Ok(())
}

pub fn close_token_budget_handler<'info, 'ctx>(
    ctx: Context<'ctx, CloseTokenBudget<'info>>,
) -> Result<()> {
    let buyer = ctx.accounts.budget.buyer;
    let mint = ctx.accounts.budget.mint;
    let bump = ctx.accounts.budget.bump;
    let seeds: &[&[u8]] = &[TOKEN_BUDGET_SEED, buyer.as_ref(), mint.as_ref(), &[bump]];

    // Whatever is left in the vault goes back to the buyer's own account.
    let remaining = ctx.accounts.vault.amount;
    if remaining > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.key(),
                TokenTransfer {
                    from: ctx.accounts.vault.to_account_info(),
                    to: ctx.accounts.buyer_token_account.to_account_info(),
                    authority: ctx.accounts.budget.to_account_info(),
                },
                &[seeds],
            ),
            remaining,
        )?;
    }

    // Close the vault itself and send its rent to the buyer.
    token::close_account(CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        token::CloseAccount {
            account: ctx.accounts.vault.to_account_info(),
            destination: ctx.accounts.buyer.to_account_info(),
            authority: ctx.accounts.budget.to_account_info(),
        },
        &[seeds],
    ))?;

    msg!("token budget closed; vault returned to the buyer");
    Ok(())
}
