//! What a sponsored SOL settlement owes each party.
//!
//! This is only arithmetic: the fee policy, the invoice's price and the number
//! of signatures decide the split before any lamports move. Keeping it here
//! makes the handler a short sequence of guards, transfers and records.

use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::PaymentError;
use crate::helpers::{fee_for_signatures, shop_multiplier};
use crate::state::{Config, FeeMode, ShopPolicy};

/// The price's destinations: the seller's proceeds, the two rents the sponsor
/// advanced, and whatever the percentage took above those costs.
pub(crate) struct SettlementSplit {
    pub(crate) receipt_rent: u64,
    pub(crate) fee: u64,
    pub(crate) to_seller: u64,
    pub(crate) to_sponsor: u64,
    pub(crate) to_treasury: u64,
}

pub(crate) fn settle_split(
    config: &Config,
    amount: u64,
    shop_policy: Option<&ShopPolicy>,
    seller: &Pubkey,
    signatures: u64,
) -> Result<SettlementSplit> {
    // Seller-pays is the only mode this release settles: the payer pays the
    // price, and the seller absorbs the permanent marker's rent plus whatever the
    // fee adds above it (nothing, while the rent is the larger of the two).
    require!(
        config.fee_mode == FeeMode::SellerPays,
        PaymentError::FeeModeUnsupported
    );

    let payment_rent = Rent::get()?.minimum_balance(PAYMENT_SPACE);
    let receipt_rent = Rent::get()?.minimum_balance(PAYMENT_RECEIPT_SPACE);
    // The shop's rate when the invoice names a policy of its own seller, and
    // `Config`'s otherwise.
    let multiplier = shop_multiplier(shop_policy, seller)?;
    // The floor is the marker's rent plus the network fee, exactly as on the
    // token paths: this settlement's sponsor paid the signatures and advanced
    // both records, and the price is what pays it back. A relayer settlement
    // carries two signatures — the payer's and the relayer's — and a wallet
    // sponsoring itself carries one, so the charge and the refund are scaled to
    // what this path actually spends.
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
    let to_seller = amount - payment_rent - receipt_rent - fee;

    Ok(SettlementSplit {
        receipt_rent,
        fee,
        to_seller,
        to_sponsor,
        to_treasury,
    })
}
