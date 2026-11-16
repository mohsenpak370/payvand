//! Token invoices and the per-mint monthly allowance.
//!
//! Settlement mirrors the SOL paths: the invoice account is created, settled
//! and closed in the settle transaction, a receipt keyed by the invoice address
//! keeps the record, and the receipt's `settled_at` is the replay guard.

use anchor_lang::prelude::*;
use anchor_lang::{AccountsClose, Lamports};
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer as TokenTransfer};

use crate::constants::*;
use crate::errors::PaymentError;
use crate::helpers::{current_month_index, ensure_treasury_funded, fee_for, pay_from_treasury};
use crate::state::{Config, FeeMode, PaymentReceipt, PaymentState, TokenBudget, TokenPayment};

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct SettleTokenPayment<'info> {
    /// Advances the invoice and receipt rents and the network fee; the treasury
    /// refunds both rents in this same instruction, so the buyer's lamports
    /// loss is the network fee alone and the price is all they pay.
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init,
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
    #[account(mut)]
    pub seller: UncheckedAccount<'info>,

    /// The seller's own token account, which must already exist. Creating one
    /// here would mint rent to whoever named the seller, so opening it is the
    /// seller's setup step — like having a wallet at all — and never a line on
    /// the buyer's bill.
    #[account(mut, token::mint = mint, token::authority = seller)]
    pub seller_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut, token::mint = mint, token::authority = payer)]
    pub payer_token_account: Box<Account<'info, TokenAccount>>,

    pub mint: Box<Account<'info, Mint>>,

    #[account(
        init_if_needed,
        payer = payer,
        space = PAYMENT_RECEIPT_SPACE,
        seeds = [PAYMENT_RECEIPT_SEED, payment.key().as_ref()],
        bump
    )]
    pub receipt: Account<'info, PaymentReceipt>,

    /// CHECK: Arbitrary 32-byte value chosen when the QR was made; a PDA seed
    /// and an address watchers can look up.
    pub reference: UncheckedAccount<'info>,

    /// The fee policy, read-only here. Same rules as the SOL path.
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    /// The SOL pool: it refunds the rents the payer advanced and receives the
    /// invoice rent back when the invoice closes.
    #[account(mut, seeds = [TREASURY_SEED], bump = config.treasury_bump)]
    pub treasury: SystemAccount<'info>,

    /// The fee's destination in this mint, opened once per mint by whoever runs
    /// the treasury; the contract cannot create it without paying rent to a
    /// seller's account, which is the faucet this avoids.
    #[account(mut, token::mint = mint, token::authority = treasury)]
    pub treasury_token_account: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

/// Opens the treasury's token account for a mint, paid by the admin.
///
/// `settle_token_payment` sends the fee into `treasury_token_account`, so a mint
/// the treasury has no account for makes every invoice in it unpurchasable — a
/// settle that fails on the buyer's signature with `AccountNotInitialized`.
/// The treasury is a PDA: nobody can create its associated account as a wallet
/// owner would, and only this program can sign for it. This is that one call.
///
/// Idempotent on purpose, so provisioning a mint twice is a no-op and the step
/// can be repeated safely by a deploy script.
#[derive(Accounts)]
pub struct OpenTreasuryTokenAccount<'info> {
    /// Pays the account's rent. The treasury's own lamports are the pool the
    /// fee is measured against, so they are not spent on accounts.
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump, has_one = admin)]
    pub config: Account<'info, Config>,

    #[account(seeds = [TREASURY_SEED], bump = config.treasury_bump)]
    pub treasury: SystemAccount<'info>,

    pub mint: Account<'info, Mint>,

    /// The fee's destination in this mint: the treasury's own account, created
    /// here when it is missing and left alone when it is not.
    #[account(
        init_if_needed,
        payer = admin,
        associated_token::mint = mint,
        associated_token::authority = treasury
    )]
    pub treasury_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
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
    /// Session key: fee payer and rent payer (refunded from the budget's float
    /// in this same transaction). Never the buyer's wallet.
    #[account(mut)]
    pub session_key: Signer<'info>,

    #[account(
        mut,
        seeds = [TOKEN_BUDGET_SEED, budget.buyer.as_ref(), budget.mint.as_ref()],
        bump = budget.bump
    )]
    pub budget: Account<'info, TokenBudget>,

    /// The canonical vault and the only accepted one: the budget's associated
    /// token account for this mint. A token account that merely shares the mint
    /// and the budget's authority is a place to strand a budget's tokens, so it
    /// is not a vault.
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = budget
    )]
    pub vault: Account<'info, TokenAccount>,

    pub mint: Account<'info, Mint>,

    /// CHECK: The address the QR code names as the payee; also the destination
    /// of the invoice account's close.
    #[account(mut)]
    pub seller: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = session_key,
        associated_token::mint = mint,
        associated_token::authority = seller
    )]
    pub seller_token_account: Account<'info, TokenAccount>,

    #[account(
        init,
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

    #[account(
        init_if_needed,
        payer = session_key,
        space = PAYMENT_RECEIPT_SPACE,
        seeds = [PAYMENT_RECEIPT_SEED, payment.key().as_ref()],
        bump
    )]
    pub receipt: Account<'info, PaymentReceipt>,

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

    /// The canonical vault, as in `pay_from_token_budget`, so a decoy account
    /// that shares the mint and the budget's authority cannot be drained in its
    /// place.
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = budget
    )]
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
    require!(memo.len() <= PAYMENT_MAX_MEMO, PaymentError::MemoTooLong);
    require_keys_neq!(
        ctx.accounts.seller.key(),
        Pubkey::default(),
        PaymentError::InvalidSeller
    );
    require!(
        ctx.accounts.receipt.settled_at == 0,
        PaymentError::AlreadySettled
    );

    // Same policy as the SOL path: the buyer pays the price, the seller carries
    // the fee, and the protocol's own records come out of the pool.
    let config = &ctx.accounts.config;
    require!(
        config.fee_mode == FeeMode::SellerPays,
        PaymentError::FeeModeUnsupported
    );

    let fee = fee_for(config, amount)?;
    // The seller must still receive something, as in `settle_payment`.
    require!(amount > fee, PaymentError::InvoiceTooSmall);

    let payment_rent = Rent::get()?.minimum_balance(TOKEN_PAYMENT_SPACE);
    let receipt_rent = Rent::get()?.minimum_balance(PAYMENT_RECEIPT_SPACE);
    // The payer advanced both rents above; the refund is checked before any
    // transfer so a settlement cannot start what the treasury cannot finish.
    let refund = payment_rent.saturating_add(receipt_rent);
    ensure_treasury_funded(&ctx.accounts.treasury.to_account_info(), refund)?;

    // Exactly `amount` leaves the buyer: the fee goes to the treasury's account
    // for this mint and the rest to the seller, who is the one carrying it.
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            TokenTransfer {
                from: ctx.accounts.payer_token_account.to_account_info(),
                to: ctx.accounts.seller_token_account.to_account_info(),
                authority: ctx.accounts.payer.to_account_info(),
            },
        ),
        amount - fee,
    )?;
    if fee > 0 {
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                TokenTransfer {
                    from: ctx.accounts.payer_token_account.to_account_info(),
                    to: ctx.accounts.treasury_token_account.to_account_info(),
                    authority: ctx.accounts.payer.to_account_info(),
                },
            ),
            fee,
        )?;
    }

    // Hand back the two rents the payer advanced. A CPI, so it runs while no
    // lamport delta is outstanding; the invoice's close below does the direct
    // move.
    pay_from_treasury(
        &ctx.accounts.treasury.to_account_info(),
        &ctx.accounts.payer.to_account_info(),
        refund,
        ctx.accounts.config.treasury_bump,
    )?;

    let now = Clock::get()?.unix_timestamp;
    let payer = ctx.accounts.payer.key();
    let seller = ctx.accounts.seller.key();
    let reference = ctx.accounts.reference.key();
    let mint = ctx.accounts.mint.key();
    let invoice = ctx.accounts.payment.key();

    {
        let payment = &mut ctx.accounts.payment;
        payment.seller = seller;
        payment.reference = reference;
        payment.mint = mint;
        payment.amount = amount;
        payment.memo = memo.clone();
        payment.state = PaymentState::Settled;
        payment.payer = payer;
        payment.created_at = now;
        payment.settled_at = now;
        payment.bump = ctx.bumps.payment;
    }

    {
        let receipt = &mut ctx.accounts.receipt;
        receipt.invoice = invoice;
        receipt.seller = seller;
        receipt.reference = reference;
        receipt.mint = mint;
        receipt.amount = amount;
        receipt.memo = memo;
        receipt.payer = payer;
        receipt.created_at = now;
        receipt.settled_at = now;
        receipt.closed_at = now;
        receipt.bump = ctx.bumps.receipt;
    }

    // The invoice's rent returns to the pool that advanced it, leaving the
    // permanent receipt as the only lasting cost — as in `settle_payment`.
    ctx.accounts
        .payment
        .close(ctx.accounts.treasury.to_account_info())?;

    msg!(
        "token payment settled: {} of {}, fee {} to the treasury",
        amount,
        mint,
        fee
    );
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
    Ok(())
}

pub fn pay_from_token_budget_handler<'info, 'ctx>(
    ctx: Context<'ctx, PayFromTokenBudget<'info>>,
    amount: u64,
    memo: String,
) -> Result<()> {
    require!(amount > 0, PaymentError::InvalidAmount);
    require!(memo.len() <= PAYMENT_MAX_MEMO, PaymentError::MemoTooLong);
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
    require!(
        ctx.accounts.receipt.settled_at == 0,
        PaymentError::AlreadySettled
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

    let now_ts = Clock::get()?.unix_timestamp;
    let payer = ctx.accounts.session_key.key();
    let seller = ctx.accounts.seller.key();
    let reference = ctx.accounts.reference.key();
    let invoice = ctx.accounts.payment.key();

    {
        let payment = &mut ctx.accounts.payment;
        payment.seller = seller;
        payment.reference = reference;
        payment.mint = mint;
        payment.amount = amount;
        payment.memo = memo.clone();
        payment.state = PaymentState::Settled;
        payment.payer = payer;
        payment.created_at = now_ts;
        payment.settled_at = now_ts;
        payment.bump = ctx.bumps.payment;
    }

    {
        let receipt = &mut ctx.accounts.receipt;
        receipt.invoice = invoice;
        receipt.seller = seller;
        receipt.reference = reference;
        receipt.mint = mint;
        receipt.amount = amount;
        receipt.memo = memo;
        receipt.payer = payer;
        receipt.created_at = now_ts;
        receipt.settled_at = now_ts;
        receipt.closed_at = now_ts;
        receipt.bump = ctx.bumps.receipt;
    }

    // Refund the invoice and receipt rent the session key just advanced, out
    // of the float the budget keeps for exactly this.
    let invoice_rent = Rent::get()?.minimum_balance(TOKEN_PAYMENT_SPACE);
    let receipt_rent = Rent::get()?.minimum_balance(PAYMENT_RECEIPT_SPACE);
    let budget_rent = Rent::get()?.minimum_balance(TOKEN_BUDGET_SPACE);
    let spare = ctx
        .accounts
        .budget
        .to_account_info()
        .lamports()
        .saturating_sub(budget_rent);
    let refund = invoice_rent.saturating_add(receipt_rent).min(spare);
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

    // Auto-close: the invoice rent is already reimbursed above, and the
    // account's own rent now follows it to the seller.
    ctx.accounts
        .payment
        .close(ctx.accounts.seller.to_account_info())?;

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

/// The account checks are the instruction: `init_if_needed` opens the
/// treasury's own account for the mint, paid by the admin, and Anchor proves it
/// is exactly that account (the ATA for this mint and the treasury PDA) before
/// the handler runs. Nothing is left to do, and a second call changes nothing.
pub fn open_treasury_token_account_handler(_ctx: Context<OpenTreasuryTokenAccount>) -> Result<()> {
    Ok(())
}
