//! The session-key half of the token allowance: settle an invoice out of the
//! vault.
//!
//! The monthly cap, the vault balance and the registered session key are all
//! checked here; the vault pays the seller the price less the fee and the
//! treasury the fee, and the pool refunds what the session key advanced.

use anchor_lang::prelude::*;
use anchor_lang::AccountsClose;
use anchor_spl::associated_token::AssociatedToken;
// Interface types, not `Account<TokenAccount>`: see the module note in
// `token/mod.rs` on why that makes `Token2022Unsupported` the error a
// Token-2022 caller sees.
use anchor_spl::token_interface::{TokenAccount, TokenInterface};

use crate::constants::*;
use crate::errors::PaymentError;
use crate::events::PaymentSettled;
use crate::helpers::{
    current_month_index, ensure_treasury_funded, mint_decimals, pay_from_treasury,
};
use crate::instructions::token::payout::TokenPayout;
use crate::instructions::token::split::token_split;
use crate::state::{Config, PaymentReceipt, PaymentState, ShopPolicy, TokenBudget, TokenPayment};

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct PayFromTokenBudget<'info> {
    /// Session key: fee payer and rent payer (refunded from the pool in this
    /// same transaction). Never the buyer's wallet.
    #[account(mut)]
    pub session_key: Signer<'info>,

    #[account(
        mut,
        seeds = [TOKEN_BUDGET_SEED, budget.buyer.as_ref(), budget.mint.as_ref()],
        bump = budget.bump
    )]
    pub budget: Box<Account<'info, TokenBudget>>,

    /// The canonical vault and the only accepted one: the budget's associated
    /// token account for this mint. A token account that merely shares the mint
    /// and the budget's authority is a place to strand a budget's tokens, so it
    /// is not a vault.
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = budget
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: The budget's mint, compared against the budget below. Its owner is
    /// the currency policy, and the rejection is named: `Token2022Unsupported`.
    #[account(
        constraint = mint.owner == &anchor_spl::token::ID
            && mint.data_len() == SPL_MINT_LEN
            @ PaymentError::Token2022Unsupported
    )]
    pub mint: UncheckedAccount<'info>,

    /// CHECK: The address the QR code names as the payee. It receives the price
    /// less the fee in tokens and no lamports — the invoice's rent goes back to
    /// the pool that advanced it.
    #[account(mut)]
    pub seller: UncheckedAccount<'info>,

    /// The seller's own token account, which must already exist — the same rule
    /// `settle_token_payment` applies. Creating it here would spend the session
    /// float, and that float is the buyer's money: the rent would be a fee the
    /// buyer pays, and nobody refunds it. Opening the account is the seller's
    /// setup step, like having a wallet at all.
    #[account(mut, token::mint = mint, token::authority = seller)]
    pub seller_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

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
    pub payment: Box<Account<'info, TokenPayment>>,

    #[account(
        init_if_needed,
        payer = session_key,
        space = PAYMENT_RECEIPT_SPACE,
        seeds = [PAYMENT_RECEIPT_SEED, payment.key().as_ref()],
        bump
    )]
    pub receipt: Box<Account<'info, PaymentReceipt>>,

    /// CHECK: Arbitrary 32-byte value chosen when the QR was made.
    pub reference: UncheckedAccount<'info>,

    /// The fee policy: the rate this payment pays and the treasury's bump.
    /// Read-only here, as on the other two settle paths.
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,

    /// The SOL pool: it refunds the rents the session key advanced and receives
    /// the invoice rent back when the invoice closes, so the pot pays the price
    /// and the seller keeps the price less the fee.
    #[account(mut, seeds = [TREASURY_SEED], bump = config.treasury_bump)]
    pub treasury: SystemAccount<'info>,

    /// The fee's destination in this mint, opened once per mint by whoever runs
    /// the treasury — the same account `settle_token_payment` pays.
    #[account(mut, token::mint = mint, token::authority = treasury)]
    pub treasury_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,

    /// The shop that priced this invoice: optional and trailing, as on the other
    /// three settle paths, so an allowance payment from a pot pays the shop's
    /// rate when the invoice names one and `Config`'s when it does not.
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

    // Same pricing as `settle_token_payment`, at one signature: the session key
    // is the only signer.
    let split = token_split(
        &ctx.accounts.config,
        amount,
        &ctx.accounts.mint.key(),
        ctx.accounts.shop_policy.as_deref(),
        &ctx.accounts.seller.key(),
        1,
    )?;
    let (fee, refund) = (split.fee, split.refund);

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

    // The session key advanced the invoice and receipt rents to open these
    // records and paid the transaction's own fee; the pool refunds all three, so
    // the pot pays the price and those costs come out of the fee. Checked before
    // any transfer, so a pool that cannot refund cannot start a settlement.
    ensure_treasury_funded(&ctx.accounts.treasury.to_account_info(), refund)?;

    // The split, signed by the budget PDA: the price less the fee to the seller,
    // the fee to the treasury's own account for this mint.
    let buyer = ctx.accounts.budget.buyer;
    let mint = ctx.accounts.budget.mint;
    let bump = ctx.accounts.budget.bump;
    let seeds: &[&[u8]] = &[TOKEN_BUDGET_SEED, buyer.as_ref(), mint.as_ref(), &[bump]];
    let decimals = mint_decimals(&ctx.accounts.mint)?;
    TokenPayout {
        token_program: ctx.accounts.token_program.key(),
        from: ctx.accounts.vault.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        seller_token_account: ctx.accounts.seller_token_account.to_account_info(),
        treasury_token_account: ctx.accounts.treasury_token_account.to_account_info(),
        authority: ctx.accounts.budget.to_account_info(),
    }
    .pay(amount, fee, decimals, &[seeds])?;

    // Hand back the two rents the session key advanced and the network fee it
    // paid. A CPI, so it runs while no lamport delta is outstanding; the
    // invoice's close below does the direct move.
    pay_from_treasury(
        &ctx.accounts.treasury.to_account_info(),
        &ctx.accounts.session_key.to_account_info(),
        refund,
        ctx.accounts.config.treasury_bump,
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
        receipt.settled_at = now_ts;
    }

    let budget = &mut ctx.accounts.budget;
    budget.spent = spent.saturating_add(amount);
    budget.month_index = now;

    // The invoice rent follows the pool that funded it, not the seller: a
    // seller's proceeds are the price less the fee, and nothing else.
    ctx.accounts
        .payment
        .close(ctx.accounts.treasury.to_account_info())?;

    msg!(
        "token payment settled from budget: {} of {}, fee {} to the treasury, session {}",
        amount,
        mint,
        fee,
        ctx.accounts.session_key.key()
    );
    emit!(PaymentSettled {
        amount,
        fee,
        // As on the wallet path: a token settlement cannot charge its seller
        // SOL, so the treasury carries the marker's rent and the event says so
        // by leaving the proceeds at `amount - fee`.
        seller_proceeds: amount - fee,
        payer,
        seller,
        mint,
        reference,
        invoice,
        memo,
    });
    Ok(())
}
