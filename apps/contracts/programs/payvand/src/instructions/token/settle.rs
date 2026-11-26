//! Paying a token invoice straight from the payer's token account.
//!
//! `settle_token_payment` separates the two jobs a wallet used to do at once:
//! `payer` signs for the buyer's token account, and `sponsor` advances the two
//! record rents and receives the treasury's refund of them. A wallet holding SOL
//! passes itself as both — one signature, and the settlement ends where it began
//! — while a wallet holding only the token passes a relayer's key, which the
//! program refunds, so a sponsored settlement cannot leave the buyer's wallet
//! holding lamports it never owned.

use anchor_lang::prelude::*;
use anchor_lang::AccountsClose;
// Interface types, not `Account<TokenAccount>`: see the module note in
// `token/mod.rs` on why that makes `Token2022Unsupported` the error a
// Token-2022 caller sees.
use anchor_spl::token_interface::{TokenAccount, TokenInterface};

use super::payout::TokenPayout;
use super::split::token_split;
use crate::constants::*;
use crate::errors::PaymentError;
use crate::events::PaymentSettled;
use crate::helpers::{ensure_treasury_funded, mint_decimals, pay_from_treasury};
use crate::state::{Config, PaymentReceipt, PaymentState, ShopPolicy, TokenPayment};

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct SettleTokenPayment<'info> {
    /// The buyer. Its signature proves it owns `payer_token_account` — which is
    /// all a sponsor cannot do for it — and its wallet loses nothing: every
    /// lamport this settlement advances belongs to `sponsor` below.
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init,
        payer = sponsor,
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
    pub seller_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, token::mint = mint, token::authority = payer)]
    pub payer_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: The SPL Token mint this invoice is priced in. Its owner is the fee
    /// policy's currency rule, and the rejection is named: `Token2022Unsupported`
    /// rather than a failure inside a token CPI.
    #[account(
        constraint = mint.owner == &anchor_spl::token::ID
            && mint.data_len() == SPL_MINT_LEN
            @ PaymentError::Token2022Unsupported
    )]
    pub mint: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = sponsor,
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
    pub treasury_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,

    /// Who pays for this settlement's records. It advances the invoice's rent
    /// and the marker's (`init`, above) and receives the treasury's refund of
    /// both in this same instruction, so its net cost is the network fee — the
    /// fee it signs as — and nothing else. A funded wallet passes its own key
    /// here and is therefore its own sponsor; a wallet holding only the token
    /// passes a relayer's, which turns a zero-lamport buyer into a payer.
    ///
    /// The refund goes to this account and never to `payer` on purpose: a
    /// sponsor that topped a buyer up would otherwise hand it both rents —
    /// 3,939,360 lamports per settlement at mainnet's rate — for the price of
    /// one signature.
    #[account(mut)]
    pub sponsor: Signer<'info>,

    /// The shop that priced this invoice, when its seller has one: optional and
    /// trailing, so a funded or sponsored settle that names none pays `Config`'s
    /// rate exactly as before. The seeds prove the account is the canonical PDA
    /// for its own contents; the handler refuses one recorded under another
    /// seller by name.
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

    // Two signatures when a relayer signs as fee payer, one when the wallet is
    // its own sponsor: the seller is charged exactly what this path pays, and the
    // refund hands back exactly that.
    let signatures = if ctx.accounts.sponsor.key() != ctx.accounts.payer.key() {
        2
    } else {
        1
    };
    let split = token_split(
        &ctx.accounts.config,
        amount,
        &ctx.accounts.mint.key(),
        ctx.accounts.shop_policy.as_deref(),
        &ctx.accounts.seller.key(),
        signatures,
    )?;
    let (fee, refund) = (split.fee, split.refund);
    // Checked before any transfer so a settlement cannot start what the
    // treasury cannot finish.
    ensure_treasury_funded(&ctx.accounts.treasury.to_account_info(), refund)?;
    let decimals = mint_decimals(&ctx.accounts.mint)?;

    // The buyer signs for its own token account, so no seeds are needed.
    TokenPayout {
        token_program: ctx.accounts.token_program.key(),
        from: ctx.accounts.payer_token_account.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        seller_token_account: ctx.accounts.seller_token_account.to_account_info(),
        treasury_token_account: ctx.accounts.treasury_token_account.to_account_info(),
        authority: ctx.accounts.payer.to_account_info(),
    }
    .pay(amount, fee, decimals, &[])?;

    // Hand back the two rents the sponsor advanced and the network fee it paid.
    // A CPI, so it runs while no lamport delta is outstanding; the invoice's
    // close below does the direct move. The destination is the sponsor and never
    // the payer, which is what keeps a sponsored settlement from being a lamport
    // faucet for the buyer.
    pay_from_treasury(
        &ctx.accounts.treasury.to_account_info(),
        &ctx.accounts.sponsor.to_account_info(),
        refund,
        ctx.accounts.config.treasury_bump,
    )?;

    let now = Clock::get()?.unix_timestamp;
    let payer = ctx.accounts.payer.key();
    let seller = ctx.accounts.seller.key();
    let reference = ctx.accounts.reference.key();
    let mint = ctx.accounts.mint.key();
    let invoice = ctx.accounts.payment.key();
    let sponsor = ctx.accounts.sponsor.key();

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
        receipt.settled_at = now;
    }

    // The invoice's rent returns to the pool that advanced it, leaving the
    // permanent receipt as the only lasting cost — as in `settle_payment`.
    ctx.accounts
        .payment
        .close(ctx.accounts.treasury.to_account_info())?;

    // The sponsor is named in the log rather than in `PaymentSettled`: that
    // event is shared with the SOL paths, whose settlements have no sponsor, and
    // its shape is what the app and the watchers decode. The transaction's own
    // fee payer is the same key on chain.
    msg!(
        "token payment settled: {} of {}, fee {} to the treasury, sponsor {}",
        amount,
        mint,
        fee,
        sponsor
    );
    emit!(PaymentSettled {
        amount,
        fee,
        // A token seller is charged no lamports: the treasury keeps funding the
        // marker's SOL rent here, because a seller who never signs cannot be
        // made to pay SOL and a token amount cannot be priced in it without an
        // oracle.
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
