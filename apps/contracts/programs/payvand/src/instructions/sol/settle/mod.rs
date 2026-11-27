//! Paying a SOL invoice straight from the payer's wallet.
//!
//! The payer spends the price only: a sponsor advances both records and the
//! transaction fee, and the price repays it here. The invoice is auto-closed to
//! the seller; the receipt is the marker that keeps the invoice single-use.
//!
//! `split` decides who gets what before any lamports move; this module only
//! applies that decision and records what happened.

pub mod split;

use anchor_lang::prelude::*;
use anchor_lang::AccountsClose;

use crate::constants::*;
use crate::errors::PaymentError;
use crate::events::PaymentSettled;
use crate::state::{Config, Payment, PaymentReceipt, PaymentState};

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
    /// Required rather than optional: an absent sponsor would have to mean the
    /// payer fronts the records, which is the cost this account exists to move
    /// off the payer.
    #[account(mut)]
    pub sponsor: Signer<'info>,
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

    // A relayer settlement carries two signatures — the payer's and the
    // relayer's — and a wallet sponsoring itself carries one, so the charge and
    // the refund are scaled to what this path actually spends.
    let signatures = if ctx.accounts.sponsor.key() != ctx.accounts.payer.key() {
        2
    } else {
        1
    };
    let split = split::settle_split(&ctx.accounts.config, amount, signatures)?;

    anchor_lang::system_program::transfer(
        CpiContext::new(
            anchor_lang::system_program::ID,
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.payer.to_account_info(),
                to: ctx.accounts.seller.to_account_info(),
            },
        ),
        split.to_seller,
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
        split.to_sponsor,
    )?;

    if split.to_treasury > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                anchor_lang::system_program::ID,
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.payer.to_account_info(),
                    to: ctx.accounts.treasury.to_account_info(),
                },
            ),
            split.to_treasury,
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
        split.to_treasury,
        ctx.accounts.sponsor.key(),
        split.to_sponsor
    );
    emit!(PaymentSettled {
        amount,
        fee: split.fee,
        seller_proceeds: amount - split.fee - split.receipt_rent,
        payer,
        seller,
        mint: Pubkey::default(),
        reference,
        invoice,
        memo,
    });
    Ok(())
}
