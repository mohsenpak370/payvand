//! What a token settlement owes each party.
//!
//! Both token settle paths — a wallet paying with `settle_token_payment` and a
//! session key paying with `pay_from_token_budget` — price an invoice the same
//! way: the buyer pays the price, the seller carries the fee, and the protocol's
//! own records come out of the pool. This is only arithmetic: the fee policy,
//! the invoice's price and the number of signatures decide the split before any
//! token moves, so each handler is a short sequence of guards, transfers and
//! records.

use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::PaymentError;
use crate::helpers::{fee_for_signatures, shop_multiplier};
use crate::state::{Config, FeeMode, ShopPolicy};

/// The price's destinations, and what the pool hands back for them.
pub(crate) struct TokenSplit {
    /// Taken from the price into the treasury's account for this mint; the
    /// seller receives `amount - fee`.
    pub(crate) fee: u64,
    /// Lamports the pool returns to whoever opened the records: the invoice's
    /// rent, the marker's rent and the network fee of every signature.
    pub(crate) refund: u64,
}

/// `signatures` is two when a relayer signs as fee payer and one when the
/// wallet is its own sponsor or a session key pays alone, so the seller is
/// charged exactly what the path pays and the refund hands back exactly that.
pub(crate) fn token_split(
    config: &Config,
    amount: u64,
    mint: &Pubkey,
    shop_policy: Option<&ShopPolicy>,
    seller: &Pubkey,
    signatures: u64,
) -> Result<TokenSplit> {
    // Same policy as the SOL paths: nothing but `SellerPays` is settled.
    require!(
        config.fee_mode == FeeMode::SellerPays,
        PaymentError::FeeModeUnsupported
    );

    let payment_rent = Rent::get()?.minimum_balance(TOKEN_PAYMENT_SPACE);
    let receipt_rent = Rent::get()?.minimum_balance(PAYMENT_RECEIPT_SPACE);
    // The shop's rate when the invoice names a policy of its own seller, and
    // `Config`'s otherwise.
    let multiplier = shop_multiplier(shop_policy, seller)?;
    let fee = fee_for_signatures(
        config,
        amount,
        Some(mint),
        receipt_rent,
        multiplier,
        signatures,
    )?;
    // The seller must still receive something. The `amount - 1` cap in the fee
    // makes this hold at any `fee_bps`; the guard says so out loud.
    require!(amount > fee, PaymentError::InvoiceTooSmall);

    // Whoever opened the records advanced both rents and paid the transaction's
    // own fee; the refund covers all three, so their net cost is zero and the
    // seller's deduction is what funds it.
    let refund = payment_rent.saturating_add(receipt_rent).saturating_add(
        config
            .policy
            .network_fee_lamports
            .checked_mul(signatures)
            .ok_or(PaymentError::FeeOverflow)?,
    );

    Ok(TokenSplit { fee, refund })
}
