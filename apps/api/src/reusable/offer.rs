//! The terms a reusable code charges, and the invoice they describe at one
//! fresh reference.

use solana_pubkey::Pubkey;

use crate::payments::{ApiError, ApiResult, CreateOption, CreatePaymentRequest, SOL_CURRENCY};

/// The stored row, without the fields only a response needs.
pub(super) struct StoredLink {
    pub(super) seller: String,
    pub(super) seller_user: String,
    pub(super) cluster: String,
    pub(super) endpoint: String,
    pub(super) currency: String,
    pub(super) amount: u64,
    pub(super) memo: String,
    pub(super) name: String,
    pub(super) description: String,
}

impl StoredLink {
    pub(super) fn offer(&self) -> Offer<'_> {
        Offer {
            seller: &self.seller,
            cluster: &self.cluster,
            endpoint: Some(self.endpoint.clone()),
            currency: &self.currency,
            amount: self.amount,
            memo: &self.memo,
            name: &self.name,
            description: &self.description,
        }
    }
}

/// The terms a code charges, without the address they are bound to. Both the
/// stored row and the request being validated reduce to this.
pub(super) struct Offer<'a> {
    pub(super) seller: &'a str,
    pub(super) cluster: &'a str,
    pub(super) endpoint: Option<String>,
    pub(super) currency: &'a str,
    pub(super) amount: u64,
    pub(super) memo: &'a str,
    pub(super) name: &'a str,
    pub(super) description: &'a str,
}

/// The invoice a code's offer describes, at one reference.
pub(super) fn invoice_request(reference: &str, offer: &Offer<'_>) -> CreatePaymentRequest {
    CreatePaymentRequest {
        payment_pda: String::new(),
        seller: offer.seller.to_string(),
        reference: reference.to_string(),
        amount_lamports: if offer.currency == SOL_CURRENCY {
            offer.amount
        } else {
            0
        },
        options: vec![CreateOption {
            currency: offer.currency.to_string(),
            amount: offer.amount,
        }],
        memo: offer.memo.to_string(),
        name: Some(offer.name.to_string()),
        description: Some(offer.description.to_string()),
        items: Vec::new(),
        discount_code: None,
        allow_discounts: false,
        cluster: offer.cluster.to_string(),
        // An empty endpoint is "the cluster's own": a code made before the
        // seller pointed at a custom RPC still mints against the default one.
        endpoint: offer
            .endpoint
            .clone()
            .filter(|value| !value.trim().is_empty()),
    }
}

/// A reference no invoice has used: 32 random bytes, base58 like any address.
pub(super) fn fresh_reference() -> ApiResult<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|error| ApiError::Internal(format!("reference randomness: {error}")))?;
    Ok(Pubkey::new_from_array(bytes).to_string())
}

#[cfg(test)]
mod tests;
