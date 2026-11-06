//! SOL invoices and the SOL monthly allowance.

use anchor_lang::prelude::*;
use anchor_lang::Lamports;

use crate::constants::*;
use crate::errors::PaymentError;
use crate::helpers::current_month_index;
use crate::state::{Budget, Payment, PaymentState};

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct SettlePayment<'info> {
    /// Pays for the invoice account (rent) and the transfer itself.
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init_if_needed,
        payer = payer,
        space = PAYMENT_SPACE,
        seeds = [
            PAYMENT_SEED,
            seller.key().as_ref(),
            reference.key().as_ref(),
            amount.to_le_bytes().as_ref()
        ],
        bump
    )]
    pub payment: Account<'info, Payment>,

    /// CHECK: The address the QR code names as the payee; it is the only
    /// account allowed to receive the lamports.
    #[account(mut)]
    pub seller: UncheckedAccount<'info>,

    /// CHECK: Arbitrary 32-byte value chosen when the QR was made. It is only
    /// used as a PDA seed and as an address that watchers can look up.
    pub reference: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

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

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct PayFromBudget<'info> {
    /// Session key: fee payer and invoice-rent payer (refunded from the
    /// budget in this same transaction). Never the buyer's wallet.
    #[account(mut)]
    pub session_key: Signer<'info>,

    #[account(
        mut,
        seeds = [BUDGET_SEED, budget.buyer.as_ref()],
        bump = budget.bump
    )]
    pub budget: Account<'info, Budget>,

    #[account(
        init_if_needed,
        payer = session_key,
        space = PAYMENT_SPACE,
        seeds = [
            PAYMENT_SEED,
            seller.key().as_ref(),
            reference.key().as_ref(),
            amount.to_le_bytes().as_ref()
        ],
        bump
    )]
    pub payment: Account<'info, Payment>,

    /// CHECK: The address the QR code names as the payee; the only account
    /// allowed to receive the lamports.
    #[account(mut)]
    pub seller: UncheckedAccount<'info>,

    /// CHECK: Arbitrary 32-byte value chosen when the QR was made; a PDA
    /// seed and an address watchers can look up.
    pub reference: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

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

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct ClosePayment<'info> {
    #[account(
        mut,
        seeds = [
            PAYMENT_SEED,
            payment.seller.as_ref(),
            payment.reference.as_ref(),
            amount.to_le_bytes().as_ref()
        ],
        bump = payment.bump,
        close = seller
    )]
    pub payment: Account<'info, Payment>,

    #[account(mut)]
    pub seller: Signer<'info>,
}

pub fn settle_payment_handler<'info, 'ctx>(
    ctx: Context<'ctx, SettlePayment<'info>>,
    amount: u64,
    memo: String,
) -> Result<()> {
    require!(amount > 0, PaymentError::InvalidAmount);
    require!(
        memo.as_bytes().len() <= PAYMENT_MAX_MEMO,
        PaymentError::MemoTooLong
    );
    // `Pubkey::default()` doubles as "not initialised yet"; it must never
    // be a real seller or the invoice could be re-opened after settling.
    require_keys_neq!(
        ctx.accounts.seller.key(),
        Pubkey::default(),
        PaymentError::InvalidSeller
    );

    let payment = &mut ctx.accounts.payment;

    if payment.seller == Pubkey::default() {
        // First touch: open the invoice and settle it in one go.
        payment.seller = ctx.accounts.seller.key();
        payment.reference = ctx.accounts.reference.key();
        payment.amount = amount;
        payment.memo = memo;
        payment.state = PaymentState::Pending;
        payment.payer = Pubkey::default();
        payment.created_at = Clock::get()?.unix_timestamp;
        payment.settled_at = 0;
        payment.bump = ctx.bumps.payment;
    } else {
        // The invoice already exists; the seeds above guarantee the seller,
        // reference and amount all match this exact invoice, so the only
        // thing left to check is that nobody paid twice.
        require!(
            payment.state == PaymentState::Pending,
            PaymentError::AlreadySettled
        );
    }

    // Lamports move from the payer straight to the seller.
    anchor_lang::system_program::transfer(
        CpiContext::new(
            anchor_lang::system_program::ID,
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.payer.to_account_info(),
                to: ctx.accounts.seller.to_account_info(),
            },
        ),
        amount,
    )?;

    let payment = &mut ctx.accounts.payment;
    payment.state = PaymentState::Settled;
    payment.payer = ctx.accounts.payer.key();
    payment.settled_at = Clock::get()?.unix_timestamp;

    msg!(
        "payment settled: {} lamports from {}",
        amount,
        payment.payer
    );
    Ok(())
}

pub fn close_payment_handler<'info, 'ctx>(
    _ctx: Context<'ctx, ClosePayment<'info>>,
    amount: u64,
) -> Result<()> {
    msg!("payment closed: {} lamports invoice", amount);
    Ok(())
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

    // Top the session key's fee float back up (only when it is short).
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

    let budget = &mut ctx.accounts.budget;
    budget.buyer = ctx.accounts.buyer.key();
    budget.session_key = session;
    budget.monthly_limit = monthly_limit;
    budget.spent = 0;
    budget.month_index = current_month_index()?;
    // close_budget validates `bump = budget.bump`; without this the
    // stored bump stays 0 and closing can never pass its seeds check.
    budget.bump = ctx.bumps.budget;
    Ok(())
}

pub fn pay_from_budget_handler<'info, 'ctx>(
    ctx: Context<'ctx, PayFromBudget<'info>>,
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

    // Only the session key registered at open time may spend, and the
    // budget is addressed through its own seeds (`[budget, buyer]`), so
    // no other wallet's budget can ever be named here.
    require_keys_eq!(
        ctx.accounts.session_key.key(),
        ctx.accounts.budget.session_key,
        PaymentError::InvalidSessionKey
    );

    // Monthly cap, lazily reset on the calendar-month boundary.
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

    // The pot must cover the price plus the invoice rent that
    // `init_if_needed` just advanced from the session key's float.
    let invoice_rent = Rent::get()?.minimum_balance(PAYMENT_SPACE);
    let budget_rent = Rent::get()?.minimum_balance(BUDGET_SPACE);
    let pot = ctx
        .accounts
        .budget
        .to_account_info()
        .lamports()
        .saturating_sub(budget_rent);
    require!(
        pot >= amount.saturating_add(invoice_rent),
        PaymentError::InsufficientBudget
    );

    // An invoice that already exists at this address is bound to this
    // seller, reference and amount by its seeds — the only question left
    // is whether someone already paid it.
    if ctx.accounts.payment.seller != Pubkey::default() {
        require!(
            ctx.accounts.payment.state == PaymentState::Pending,
            PaymentError::AlreadySettled
        );
    }

    // Move the money: pot → seller (the price) and pot → session key
    // (refund of the invoice rent it advanced).
    let total = amount.saturating_add(invoice_rent);
    ctx.accounts.budget.to_account_info().sub_lamports(total)?;
    ctx.accounts.seller.to_account_info().add_lamports(amount)?;
    ctx.accounts
        .session_key
        .to_account_info()
        .add_lamports(invoice_rent)?;

    // Record the invoice (first touch) and mark it settled.
    {
        let payment = &mut ctx.accounts.payment;
        if payment.seller == Pubkey::default() {
            payment.seller = ctx.accounts.seller.key();
            payment.reference = ctx.accounts.reference.key();
            payment.amount = amount;
            payment.memo = memo;
            payment.state = PaymentState::Pending;
            payment.payer = Pubkey::default();
            payment.created_at = Clock::get()?.unix_timestamp;
            payment.settled_at = 0;
            payment.bump = ctx.bumps.payment;
        }
        payment.state = PaymentState::Settled;
        payment.payer = ctx.accounts.session_key.key();
        payment.settled_at = Clock::get()?.unix_timestamp;
    }
    {
        let budget = &mut ctx.accounts.budget;
        budget.spent = spent.saturating_add(amount);
        budget.month_index = now;
    }

    msg!(
        "payment settled from budget: {} lamports, session {}",
        amount,
        ctx.accounts.session_key.key()
    );
    Ok(())
}

pub fn close_budget_handler<'info, 'ctx>(_ctx: Context<'ctx, CloseBudget<'info>>) -> Result<()> {
    msg!("budget closed; remaining lamports returned to the buyer");
    Ok(())
}
