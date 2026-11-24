//! SOL invoices and the SOL monthly allowance.
//!
//! Settlement is single-use and leaves a permanent marker: the invoice account
//! is created, settled and closed inside the settle transaction, and a receipt
//! keyed by the invoice address survives it. The marker's `settled_at` is the
//! replay guard, which is why the invoice account can be `init` rather than
//! `init_if_needed`. The payment's own facts are in the `PaymentSettled` event
//! the same transaction logs.

use anchor_lang::prelude::*;
use anchor_lang::{AccountsClose, Lamports};

use crate::constants::*;
use crate::errors::PaymentError;
use crate::events::{BudgetOpened, PaymentSettled};
use crate::helpers::{current_month_index, fee_for, fee_for_signatures, shop_multiplier};
use crate::state::{Budget, Config, FeeMode, Payment, PaymentReceipt, PaymentState, ShopPolicy};

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct SettlePayment<'info> {
    /// Pays the price and nothing else. It owns no lamports beyond the invoice:
    /// the two records are the sponsor's to advance and the network fee is the
    /// sponsor's to pay, so a wallet holding exactly the price can settle.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Short-lived: created, settled and closed in this one transaction. Its
    /// rent is advanced by the sponsor and returns to the seller when the
    /// account closes, who fronts no signature for it.
    #[account(
        init,
        payer = sponsor,
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

    /// The permanent marker. `settled_at != 0` means this invoice is done, so
    /// the same invoice can never be settled twice. The sponsor fronts its rent
    /// and the price repays it, so the seller is the one who ends up paying for
    /// the record.
    #[account(
        init_if_needed,
        payer = sponsor,
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
    /// account. Nothing else about a SOL settlement touches it.
    #[account(mut, seeds = [TREASURY_SEED], bump = config.treasury_bump)]
    pub treasury: SystemAccount<'info>,

    pub system_program: Program<'info, System>,

    /// Who fronts what the payment costs in lamports: the invoice's rent, the
    /// marker's rent and the transaction's own fee. It receives both rents back
    /// plus `network_fee_lamports` out of the price in this same instruction, so
    /// its net cost is the fee it paid and nothing else. A funded wallet passes
    /// itself — one key, one signature — and a wallet holding no SOL passes a
    /// relayer's key, which turns a payer who cannot afford a signature into a
    /// payer without renting it the rest.
    ///
    /// Required rather than optional, unlike `shop_policy`: an absent sponsor
    /// would have to mean the payer fronts the records, which is the cost this
    /// account exists to move off the payer.
    #[account(mut)]
    pub sponsor: Signer<'info>,

    /// The shop that priced this invoice, when its seller has one. Optional and
    /// trailing, so every invoice printed before shop rates existed settles
    /// exactly as it did: no account means `Config`'s rate. The seeds derive it
    /// from the seller and shop key it records, so a stand-in address is refused
    /// here; whether it belongs to *this* invoice's seller is the handler's
    /// named check.
    #[account(
        seeds = [
            SHOP_POLICY_SEED,
            shop_policy.seller.as_ref(),
            shop_policy.shop_key.as_ref()
        ],
        bump = shop_policy.bump
    )]
    pub shop_policy: Option<Account<'info, ShopPolicy>>,
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

    /// The shop that priced this invoice: optional and trailing, exactly as on
    /// `settle_payment`. The allowance path is where a shop rate matters most —
    /// a buyer paying silently from a pot still pays the shop's rate, because the
    /// policy travels with the invoice's seller.
    #[account(
        seeds = [
            SHOP_POLICY_SEED,
            shop_policy.seller.as_ref(),
            shop_policy.shop_key.as_ref()
        ],
        bump = shop_policy.bump
    )]
    pub shop_policy: Option<Account<'info, ShopPolicy>>,
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
    // price, and the seller absorbs the permanent marker's rent plus whatever the
    // fee adds above it (nothing, while the rent is the larger of the two).
    let config = &ctx.accounts.config;
    require!(
        config.fee_mode == FeeMode::SellerPays,
        PaymentError::FeeModeUnsupported
    );

    let payment_rent = Rent::get()?.minimum_balance(PAYMENT_SPACE);
    let receipt_rent = Rent::get()?.minimum_balance(PAYMENT_RECEIPT_SPACE);
    // The shop's rate when the invoice names a policy of its own seller, and
    // `Config`'s otherwise.
    let multiplier = shop_multiplier(
        ctx.accounts.shop_policy.as_deref(),
        &ctx.accounts.seller.key(),
    )?;
    // The floor is the marker's rent plus the network fee, exactly as on the
    // token paths: this settlement's sponsor paid the signatures and advanced
    // both records, and the price is what pays it back. A relayer settlement
    // carries two signatures — the payer's and the relayer's — and a wallet
    // sponsoring itself carries one, so the charge and the refund are scaled to
    // what this path actually spends.
    let signatures = if ctx.accounts.sponsor.key() != ctx.accounts.payer.key() {
        2
    } else {
        1
    };
    let fee = fee_for_signatures(config, amount, None, receipt_rent, multiplier, signatures)?;
    // The seller receives `amount - rent - receipt rent - fee` now, plus the
    // invoice rent when the account closes, so this subtraction must not
    // underflow.
    require!(
        amount
            > payment_rent
                .saturating_add(receipt_rent)
                .saturating_add(fee),
        PaymentError::InvoiceTooSmall
    );

    // What the price owes the two parties that are not the seller. `fee_for`
    // keeps `fee` at or above `network_fee`, so `to_treasury` cannot go negative
    // and the split never pays out more than it charged; the check says so out
    // loud rather than letting a bad policy underflow.
    let network_fee = config
        .policy
        .network_fee_lamports
        .checked_mul(signatures)
        .ok_or(PaymentError::FeeOverflow)?;
    require!(fee >= network_fee, PaymentError::FeeOverflow);
    // Both rents and the fee the sponsor paid: the invoice's rent is closed to
    // the seller below, and the marker's stays with the marker, so the sponsor is
    // repaid both out of the price in this same instruction.
    let to_sponsor = payment_rent
        .saturating_add(receipt_rent)
        .saturating_add(network_fee);
    // The platform's margin: what the percentage took above the two costs.
    let to_treasury = fee - network_fee;

    anchor_lang::system_program::transfer(
        CpiContext::new(
            anchor_lang::system_program::ID,
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.payer.to_account_info(),
                to: ctx.accounts.seller.to_account_info(),
            },
        ),
        amount - payment_rent - receipt_rent - fee,
    )?;

    // The sponsor's advance comes back to it, not to the payer: the payer paid
    // the price, and the price is what carries these costs.
    anchor_lang::system_program::transfer(
        CpiContext::new(
            anchor_lang::system_program::ID,
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.payer.to_account_info(),
                to: ctx.accounts.sponsor.to_account_info(),
            },
        ),
        to_sponsor,
    )?;

    if to_treasury > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                anchor_lang::system_program::ID,
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.payer.to_account_info(),
                    to: ctx.accounts.treasury.to_account_info(),
                },
            ),
            to_treasury,
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
        receipt.settled_at = now;
    }

    // Auto-close. The invoice account and its rent go to the seller inside the
    // settle transaction — there is no separate seller close step left for
    // anyone to race or misuse.
    ctx.accounts
        .payment
        .close(ctx.accounts.seller.to_account_info())?;

    msg!(
        "payment settled: {} lamports from {}, fee {} to treasury, sponsor {} repaid {}",
        amount,
        payer,
        to_treasury,
        ctx.accounts.sponsor.key(),
        to_sponsor
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
    let multiplier = shop_multiplier(
        ctx.accounts.shop_policy.as_deref(),
        &ctx.accounts.seller.key(),
    )?;
    let fee = fee_for(config, amount, None, receipt_rent, multiplier)?;
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

pub fn close_budget_handler<'info, 'ctx>(_ctx: Context<'ctx, CloseBudget<'info>>) -> Result<()> {
    msg!("budget closed; remaining lamports returned to the buyer");
    Ok(())
}
