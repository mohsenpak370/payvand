//! Builds the API record from an account, marker or settlement event.

use solana_pubkey::Pubkey;

use super::{
    super::config::currency_for_mint,
    super::model::{
        PaymentOption, PaymentRecord, PaymentStatus, SOL_CURRENCY, SOL_DECIMALS, STABLE_DECIMALS,
    },
    accounts::{
        decode_payment, decode_token_payment, OnchainPayment, OnchainReceipt, OnchainTokenPayment,
        ReceiptMarker,
    },
};

/// The record for whatever kind of invoice lives at an address — SOL or token.
/// Anything else (a budget, a stranger's account) is not an invoice.
pub(crate) fn record_for(
    pubkey: Pubkey,
    data: &[u8],
    cluster: String,
    endpoint: String,
) -> Option<PaymentRecord> {
    if let Some(payment) = decode_payment(data) {
        return Some(record_from_chain(pubkey, payment, cluster, endpoint));
    }
    decode_token_payment(data)
        .map(|payment| token_record_from_chain(pubkey, payment, cluster, endpoint))
}

/// Placeholder returned for an address with no receipt and no invoice account —
/// to every watcher it is an invoice that has not been paid yet.
///
/// `Missing` is deliberately not used here: an address that derives an invoice
/// is a real invoice, whether or not anyone has paid it. Invalid input is a
/// rejected request, not a status.
pub(crate) fn pending_record(pubkey: Pubkey, cluster: String, endpoint: String) -> PaymentRecord {
    PaymentRecord {
        id: pubkey.to_string(),
        status: PaymentStatus::Pending,
        payment_pda: pubkey.to_string(),
        seller: String::new(),
        seller_user: None,
        reference: String::new(),
        amount_lamports: 0,
        options: Vec::new(),
        memo: String::new(),
        name: String::new(),
        description: String::new(),
        items: Vec::new(),
        discount: None,
        allow_discounts: false,
        cluster,
        endpoint,
        settled_by: None,
        created_at_unix: 0,
        settled_at_unix: 0,
        fee: None,
    }
}

pub(crate) fn record_from_chain(
    pubkey: Pubkey,
    payment: OnchainPayment,
    cluster: String,
    endpoint: String,
) -> PaymentRecord {
    PaymentRecord {
        id: pubkey.to_string(),
        status: if payment.state == 1 {
            PaymentStatus::Settled
        } else {
            PaymentStatus::Pending
        },
        payment_pda: pubkey.to_string(),
        seller: payment.seller.to_string(),
        seller_user: None,
        reference: payment.reference.to_string(),
        amount_lamports: payment.amount,
        // A chain read describes one address, which is one option: the seller
        // screen knows the rest from the invoice's own code.
        options: vec![PaymentOption {
            currency: SOL_CURRENCY.to_string(),
            mint: String::new(),
            decimals: SOL_DECIMALS,
            amount: payment.amount,
            payment_pda: pubkey.to_string(),
        }],
        memo: payment.memo,
        name: String::new(),
        description: String::new(),
        items: Vec::new(),
        discount: None,
        allow_discounts: false,
        cluster,
        endpoint,
        settled_by: (payment.payer != Pubkey::default()).then(|| payment.payer.to_string()),
        created_at_unix: payment.created_at,
        settled_at_unix: payment.settled_at,
        fee: None,
    }
}

/// The record for a settled invoice whose settlement could not be read back:
/// the marker says "settled" and who it belongs to, which is the part the
/// cluster cannot lose. A status read still answers, it just cannot name the
/// payer or the price until the next attempt.
pub(crate) fn marker_record(
    marker: ReceiptMarker,
    cluster: String,
    endpoint: String,
) -> PaymentRecord {
    PaymentRecord {
        id: marker.invoice.to_string(),
        status: PaymentStatus::Settled,
        payment_pda: marker.invoice.to_string(),
        seller: String::new(),
        seller_user: None,
        reference: String::new(),
        amount_lamports: 0,
        options: Vec::new(),
        memo: String::new(),
        name: String::new(),
        description: String::new(),
        items: Vec::new(),
        discount: None,
        allow_discounts: false,
        cluster,
        endpoint,
        settled_by: None,
        created_at_unix: 0,
        settled_at_unix: marker.settled_at,
        fee: None,
    }
}

/// The record for a settled invoice, built from its event. The marker outlives
/// the invoice account, so this is what a watcher sees after payment.
pub(crate) fn receipt_record(
    receipt: OnchainReceipt,
    cluster: String,
    endpoint: String,
) -> PaymentRecord {
    let invoice = receipt.invoice.to_string();
    let (currency, mint, decimals, amount_lamports) = if receipt.mint == Pubkey::default() {
        (
            SOL_CURRENCY.to_string(),
            String::new(),
            SOL_DECIMALS,
            receipt.amount,
        )
    } else {
        (
            currency_for_mint(&receipt.mint, &cluster)
                .unwrap_or("token")
                .to_string(),
            receipt.mint.to_string(),
            STABLE_DECIMALS,
            0,
        )
    };

    PaymentRecord {
        id: invoice.clone(),
        status: PaymentStatus::Settled,
        payment_pda: invoice.clone(),
        seller: receipt.seller.to_string(),
        seller_user: None,
        reference: receipt.reference.to_string(),
        amount_lamports,
        options: vec![PaymentOption {
            currency,
            mint,
            decimals,
            amount: receipt.amount,
            payment_pda: invoice,
        }],
        memo: receipt.memo,
        name: String::new(),
        description: String::new(),
        items: Vec::new(),
        discount: None,
        allow_discounts: false,
        cluster,
        endpoint,
        settled_by: (receipt.payer != Pubkey::default()).then(|| receipt.payer.to_string()),
        created_at_unix: receipt.created_at,
        settled_at_unix: receipt.settled_at,
        fee: receipt.fee,
    }
}

pub(crate) fn token_record_from_chain(
    pubkey: Pubkey,
    payment: OnchainTokenPayment,
    cluster: String,
    endpoint: String,
) -> PaymentRecord {
    let currency = currency_for_mint(&payment.mint, &cluster)
        .unwrap_or("token")
        .to_string();
    PaymentRecord {
        id: pubkey.to_string(),
        status: if payment.state == 1 {
            PaymentStatus::Settled
        } else {
            PaymentStatus::Pending
        },
        payment_pda: pubkey.to_string(),
        seller: payment.seller.to_string(),
        seller_user: None,
        reference: payment.reference.to_string(),
        // A token invoice charges no SOL; the option below carries the price.
        amount_lamports: 0,
        options: vec![PaymentOption {
            currency,
            mint: payment.mint.to_string(),
            decimals: STABLE_DECIMALS,
            amount: payment.amount,
            payment_pda: pubkey.to_string(),
        }],
        memo: payment.memo,
        name: String::new(),
        description: String::new(),
        items: Vec::new(),
        discount: None,
        allow_discounts: false,
        cluster,
        endpoint,
        settled_by: (payment.payer != Pubkey::default()).then(|| payment.payer.to_string()),
        created_at_unix: payment.created_at,
        settled_at_unix: payment.settled_at,
        fee: None,
    }
}

#[cfg(test)]
mod tests;
