//! SOL invoices and the SOL monthly allowance.
//!
//! Settlement is single-use and leaves a permanent record: the invoice account
//! is created, settled and closed inside the settle transaction, and a receipt
//! keyed by the invoice address survives it. The receipt's `settled_at` is the
//! replay guard, which is why the invoice account can be `init` rather than
//! `init_if_needed`.

use anchor_lang::prelude::*;
use anchor_lang::{AccountsClose, Lamports};

use crate::constants::*;
use crate::errors::PaymentError;
use crate::helpers::{current_month_index, ensure_treasury_funded, fee_for, pay_from_treasury};
use crate::state::{Budget, Config, FeeMode, Payment, PaymentReceipt, PaymentState};

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct SettlePayment<'info> {
    /// Pays for the invoice account, the receipt, and the transfer itself.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Short-lived: created, settled and closed in this one transaction. Its
    /// rent goes back to the seller, who fronts no signature for it.
    #[account(
        init,
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

    /// The permanent record. `settled_at != 0` means this invoice is done, so
    /// the same invoice can never be settled twice.
    #[account(
        init_if_needed,
        payer = payer,
        space = PAYMENT_RECEIPT_SPACE,
        seeds = [PAYMENT_RECEIPT_SEED, payment.key().as_ref()],
        bump
    )]
    pub receipt: Account<'info, PaymentReceipt>,

    /// CHECK: Arbitrary 32-byte value chosen when the QR was made. It is only
    /// used as a PDA seed and as an address that watchers can look up.
    pub reference: UncheckedAccount<'info>,

    /// The fee policy. Read-only here: only the admin writes it.
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    /// The fee destination, pinned by seed so the payer cannot name another
    /// account. It also refunds the receipt rent the payer just advanced.
    #[account(mut, seeds = [TREASURY_SEED], bump = config.treasury_bump)]
    pub treasury: SystemAccount<'info>,

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

    /// Supplies the session float, so a policy change reaches every new budget.
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct PayFromBudget<'info> {
    /// Session key: fee payer and rent payer (made whole by the pot in this
    /// same transaction). Never the buyer's wallet.
    #[account(mut)]
    pub session_key: Signer<'info>,

    #[account(
        mut,
        seeds = [BUDGET_SEED, budget.buyer.as_ref()],
        bump = budget.bump
    )]
    pub budget: Account<'info, Budget>,

    #[account(
        init,
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

    #[account(
        init_if_needed,
        payer = session_key,
        space = PAYMENT_RECEIPT_SPACE,
        seeds = [PAYMENT_RECEIPT_SEED, payment.key().as_ref()],
        bump
    )]
    pub receipt: Account<'info, PaymentReceipt>,

    /// CHECK: Arbitrary 32-byte value chosen when the QR was made; a PDA
    /// seed and an address watchers can look up.
    pub reference: UncheckedAccount<'info>,

    /// The fee policy. Read-only here.
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    /// The fee destination and the refund source, pinned by seed.
    #[account(mut, seeds = [TREASURY_SEED], bump = config.treasury_bump)]
    pub treasury: SystemAccount<'info>,

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

pub fn settle_payment_handler<'info, 'ctx>(
    ctx: Context<'ctx, SettlePayment<'info>>,
    amount: u64,
    memo: String,
) -> Result<()> {
    require!(amount > 0, PaymentError::InvalidAmount);
    require!(memo.len() <= PAYMENT_MAX_MEMO, PaymentError::MemoTooLong);
    // `Pubkey::default()` is not a real seller, and a payment to it would burn
    // the invoice.
    require_keys_neq!(
        ctx.accounts.seller.key(),
        Pubkey::default(),
        PaymentError::InvalidSeller
    );

    // A receipt with a settlement timestamp means this invoice is done. The
    // account is only ever created below, and `settled_at` is written in the
    // same instruction that creates it, so "exists" always means "settled".
    require!(
        ctx.accounts.receipt.settled_at == 0,
        PaymentError::AlreadySettled
    );

    // Seller-pays is the only mode this release settles: the payer pays the
    // price, the seller absorbs the fee, and the treasury hands the receipt
    // rent back so the payer is charged exactly what the QR showed.
    let config = &ctx.accounts.config;
    require!(
        config.fee_mode == FeeMode::SellerPays,
        PaymentError::FeeModeUnsupported
    );

    let fee = fee_for(config, amount)?;
    let payment_rent = Rent::get()?.minimum_balance(PAYMENT_SPACE);
    let receipt_rent = Rent::get()?.minimum_balance(PAYMENT_RECEIPT_SPACE);
    // The seller receives `amount - rent - fee` now, plus the rent when the
    // invoice account closes, so this subtraction must not underflow.
    require!(
        amount > payment_rent.saturating_add(fee),
        PaymentError::InvoiceTooSmall
    );
    // Checked before any transfer: a settlement must not start moving lamports
    // it cannot finish refunding.
    ensure_treasury_funded(&ctx.accounts.treasury.to_account_info(), receipt_rent)?;

    anchor_lang::system_program::transfer(
        CpiContext::new(
            anchor_lang::system_program::ID,
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.payer.to_account_info(),
                to: ctx.accounts.seller.to_account_info(),
            },
        ),
        amount - payment_rent - fee,
    )?;

    if fee > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                anchor_lang::system_program::ID,
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.payer.to_account_info(),
                    to: ctx.accounts.treasury.to_account_info(),
                },
            ),
            fee,
        )?;
    }

    let now = Clock::get()?.unix_timestamp;
    let payer = ctx.accounts.payer.key();
    let seller = ctx.accounts.seller.key();
    let reference = ctx.accounts.reference.key();
    let invoice = ctx.accounts.payment.key();

    {
        let payment = &mut ctx.accounts.payment;
        payment.seller = seller;
        payment.reference = reference;
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
        receipt.mint = Pubkey::default();
        receipt.amount = amount;
        receipt.memo = memo;
        receipt.payer = payer;
        receipt.created_at = now;
        receipt.settled_at = now;
        receipt.closed_at = now;
        receipt.bump = ctx.bumps.receipt;
    }

    // Auto-close. The record now lives in the receipt, so the invoice account
    // and its rent go to the seller inside the settle transaction — there is
    // no separate seller close step left for anyone to race or misuse.
    ctx.accounts
        .payment
        .close(ctx.accounts.seller.to_account_info())?;

    // The receipt is permanent, so the payer does not keep its rent: the
    // treasury funds the record and pays the payer back in the same
    // transaction.
    pay_from_treasury(
        &ctx.accounts.treasury.to_account_info(),
        &ctx.accounts.payer.to_account_info(),
        receipt_rent,
        ctx.accounts.config.treasury_bump,
    )?;

    msg!(
        "payment settled: {} lamports from {}, fee {} to treasury",
        amount,
        payer,
        fee
    );
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
    Ok(())
}

pub fn pay_from_budget_handler<'info, 'ctx>(
    ctx: Context<'ctx, PayFromBudget<'info>>,
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

    // Only the session key registered at open time may spend, and the
    // budget is addressed through its own seeds (`[budget, buyer]`), so
    // no other wallet's budget can ever be named here.
    require_keys_eq!(
        ctx.accounts.session_key.key(),
        ctx.accounts.budget.session_key,
        PaymentError::InvalidSessionKey
    );

    // Same replay rule as the wallet path.
    require!(
        ctx.accounts.receipt.settled_at == 0,
        PaymentError::AlreadySettled
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

    // The session key advances both rents; the pot reimburses both, pays the
    // seller the price less the invoice rent and the fee, sends the fee to the
    // treasury, and the invoice account's close returns that rent to the seller.
    // The treasury then refunds the receipt rent to the pot, so the pot pays
    // exactly the invoice amount.
    let config = &ctx.accounts.config;
    require!(
        config.fee_mode == FeeMode::SellerPays,
        PaymentError::FeeModeUnsupported
    );

    let fee = fee_for(config, amount)?;
    let payment_rent = Rent::get()?.minimum_balance(PAYMENT_SPACE);
    let receipt_rent = Rent::get()?.minimum_balance(PAYMENT_RECEIPT_SPACE);
    let budget_rent = Rent::get()?.minimum_balance(BUDGET_SPACE);
    require!(
        amount > payment_rent.saturating_add(fee),
        PaymentError::InvoiceTooSmall
    );
    ensure_treasury_funded(&ctx.accounts.treasury.to_account_info(), receipt_rent)?;

    let pot = ctx
        .accounts
        .budget
        .to_account_info()
        .lamports()
        .saturating_sub(budget_rent);
    let to_seller = amount - payment_rent - fee;
    let to_session = payment_rent.saturating_add(receipt_rent);
    let outflow = to_seller.saturating_add(fee).saturating_add(to_session);
    // The treasury refunds the receipt rent into the pot just below, before the
    // pot is debited, so that refund is already part of what the pot can pay:
    // the pot only has to hold the price itself. Checking the raw pot against
    // `outflow` made the last receipt rent of every limit unspendable, and the
    // client's preflight, which quotes the pot against the price, would then be
    // right while the settle still failed.
    require!(
        pot.saturating_add(receipt_rent) >= outflow,
        PaymentError::InsufficientBudget
    );

    // The treasury funds the permanent receipt, refunding the pot so it pays
    // exactly the invoice amount. This is a CPI and must run before the direct
    // lamport moves below: the runtime refuses to start a CPI while the current
    // instruction frame has an outstanding lamport delta.
    pay_from_treasury(
        &ctx.accounts.treasury.to_account_info(),
        &ctx.accounts.budget.to_account_info(),
        receipt_rent,
        ctx.accounts.config.treasury_bump,
    )?;

    ctx.accounts
        .budget
        .to_account_info()
        .sub_lamports(outflow)?;
    ctx.accounts
        .seller
        .to_account_info()
        .add_lamports(to_seller)?;
    if fee > 0 {
        ctx.accounts.treasury.to_account_info().add_lamports(fee)?;
    }
    ctx.accounts
        .session_key
        .to_account_info()
        .add_lamports(to_session)?;

    let now_ts = Clock::get()?.unix_timestamp;
    let payer = ctx.accounts.session_key.key();
    let seller = ctx.accounts.seller.key();
    let reference = ctx.accounts.reference.key();
    let invoice = ctx.accounts.payment.key();

    {
        let payment = &mut ctx.accounts.payment;
        payment.seller = seller;
        payment.reference = reference;
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
        receipt.mint = Pubkey::default();
        receipt.amount = amount;
        receipt.memo = memo;
        receipt.payer = payer;
        receipt.created_at = now_ts;
        receipt.settled_at = now_ts;
        receipt.closed_at = now_ts;
        receipt.bump = ctx.bumps.receipt;
    }

    {
        let budget = &mut ctx.accounts.budget;
        budget.spent = spent.saturating_add(amount);
        budget.month_index = now;
    }

    // Same auto-close as the wallet path: the rent the pot fronted comes back
    // to the seller, and the receipt is the only surviving record.
    ctx.accounts
        .payment
        .close(ctx.accounts.seller.to_account_info())?;

    msg!(
        "payment settled from budget: {} lamports, fee {} to treasury, session {}",
        amount,
        fee,
        ctx.accounts.session_key.key()
    );
    Ok(())
}

pub fn close_budget_handler<'info, 'ctx>(_ctx: Context<'ctx, CloseBudget<'info>>) -> Result<()> {
    msg!("budget closed; remaining lamports returned to the buyer");
    Ok(())
}
