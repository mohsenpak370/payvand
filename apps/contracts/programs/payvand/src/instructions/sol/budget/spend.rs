//! The session-key half of the allowance: settle an invoice out of the pot.
//!
//! The monthly cap, the pot balance and the registered session key are all
//! checked here; the price pays the seller, refunds the key's advances and
//! leaves the platform its margin, just as on the wallet settle path.

use anchor_lang::prelude::*;
use anchor_lang::{AccountsClose, Lamports};

use crate::constants::*;
use crate::errors::PaymentError;
use crate::events::PaymentSettled;
use crate::helpers::{current_month_index, fee_for};
use crate::state::{Budget, Config, FeeMode, Payment, PaymentReceipt, PaymentState};

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

    /// The permanent marker; `settled_at` is the replay guard. The session key
    /// advances its rent and the pot reimburses it, so the seller carries the
    /// cost through the reduced proceeds.
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

    /// The fee destination, pinned by seed.
    #[account(mut, seeds = [TREASURY_SEED], bump = config.treasury_bump)]
    pub treasury: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
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

    // The session key advances both rents and pays the signature; the pot
    // reimburses both rents and the network fee, so the key's net cost is zero.
    // The seller is paid the price less the invoice rent, the marker's rent and
    // the fee; the invoice account's close then returns the invoice rent to the
    // seller, and the marker's rent stays with the marker — which is how the
    // seller, and not the treasury, ends up paying for the permanent record.
    // The reimbursements and the seller's reduction cancel out, so the pot pays
    // exactly the invoice amount.
    let config = &ctx.accounts.config;
    require!(
        config.fee_mode == FeeMode::SellerPays,
        PaymentError::FeeModeUnsupported
    );

    let payment_rent = Rent::get()?.minimum_balance(PAYMENT_SPACE);
    let receipt_rent = Rent::get()?.minimum_balance(PAYMENT_RECEIPT_SPACE);
    let fee = fee_for(config, amount, None, receipt_rent)?;
    let budget_rent = Rent::get()?.minimum_balance(BUDGET_SPACE);
    require!(
        amount
            > payment_rent
                .saturating_add(receipt_rent)
                .saturating_add(fee),
        PaymentError::InvoiceTooSmall
    );

    let pot = ctx
        .accounts
        .budget
        .to_account_info()
        .lamports()
        .saturating_sub(budget_rent);
    // Same split as the wallet path: the price pays the seller less the two
    // costs and the fee, repays the session key both rents and the signature it
    // paid, and leaves the platform the margin above the costs.
    let network_fee = config.policy.network_fee_lamports;
    require!(fee >= network_fee, PaymentError::FeeOverflow);
    let to_seller = amount - payment_rent - receipt_rent - fee;
    let to_session = payment_rent
        .saturating_add(receipt_rent)
        .saturating_add(network_fee);
    let to_treasury = fee - network_fee;
    // `to_seller + to_session + to_treasury` is exactly `amount`, so the pot is
    // asked for the price and nothing more — the seller's share already carries
    // the costs the session key gets back.
    let outflow = to_seller
        .saturating_add(to_session)
        .saturating_add(to_treasury);
    require!(pot >= outflow, PaymentError::InsufficientBudget);

    ctx.accounts
        .budget
        .to_account_info()
        .sub_lamports(outflow)?;
    ctx.accounts
        .seller
        .to_account_info()
        .add_lamports(to_seller)?;
    if to_treasury > 0 {
        ctx.accounts
            .treasury
            .to_account_info()
            .add_lamports(to_treasury)?;
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
        receipt.settled_at = now_ts;
    }

    {
        let budget = &mut ctx.accounts.budget;
        budget.spent = spent.saturating_add(amount);
        budget.month_index = now;
    }

    // Same auto-close as the wallet path: the invoice rent the session key
    // fronted comes back to the seller, and the marker is the only surviving
    // account.
    ctx.accounts
        .payment
        .close(ctx.accounts.seller.to_account_info())?;

    msg!(
        "payment settled from budget: {} lamports, fee {} to treasury, session {} repaid {}",
        amount,
        to_treasury,
        ctx.accounts.session_key.key(),
        to_session
    );
    emit!(PaymentSettled {
        amount,
        fee,
        seller_proceeds: amount - fee - receipt_rent,
        payer,
        seller,
        mint: Pubkey::default(),
        reference,
        invoice,
        memo,
    });
    Ok(())
}
