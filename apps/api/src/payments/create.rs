//! Validates a seller's intent and builds the canonical payment record.

use serde::Deserialize;

use super::{
    config::resolve_endpoint,
    error::{bad, parse_pubkey, ApiResult},
    items::validate_items,
    model::{
        PaymentItem, PaymentRecord, PaymentStatus, MAX_DESCRIPTION_LEN, MAX_MEMO_LEN, MAX_NAME_LEN,
    },
    options::build_options,
};

/// What the seller posts after its wallet has created the payment account.
#[derive(Debug, Deserialize)]
pub struct CreatePaymentRequest {
    /// Kept for older clients: the SOL invoice address they derived. Newer
    /// clients send `options` and leave this out, and the API derives every
    /// address itself.
    #[serde(default)]
    pub payment_pda: String,
    pub seller: String,
    pub reference: String,
    #[serde(default)]
    pub amount_lamports: u64,
    /// The currencies this invoice accepts, each with its own price. Empty
    /// means the older shape: one SOL price in `amount_lamports`.
    #[serde(default)]
    pub options: Vec<CreateOption>,
    pub memo: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// Receipt lines, when the invoice was built from a shop. Checked to add up
    /// to the SOL price, so a code can never itemise one total and charge
    /// another.
    #[serde(default)]
    pub items: Vec<PaymentItem>,
    /// Optional code to bake into this invoice. The handler resolves it to a
    /// percentage against the signed-in seller's own codes, so the body can
    /// name a code but never choose the discount.
    #[serde(default)]
    pub discount_code: Option<String>,
    /// Let the buyer pick one of the seller's codes on their own screen. A code
    /// in `discount_code` already decides the price, so the two are exclusive.
    #[serde(default)]
    pub allow_discounts: bool,
    pub cluster: String,
    pub endpoint: Option<String>,
}

/// One currency the seller chose to accept, with the price it charges in it.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateOption {
    pub currency: String,
    pub amount: u64,
}

/// Validates an intent and returns the canonical record plus the QR payload.
pub fn create_payment(request: CreatePaymentRequest) -> ApiResult<PaymentRecord> {
    parse_pubkey(&request.seller, "seller")?;
    parse_pubkey(&request.reference, "reference")?;
    if !request.payment_pda.is_empty() {
        parse_pubkey(&request.payment_pda, "payment_pda")?;
    }

    if request.options.is_empty() && request.amount_lamports == 0 {
        return Err(bad("amount_lamports must be greater than 0"));
    }
    if request.memo.len() > MAX_MEMO_LEN {
        return Err(bad(format!("memo must be at most {MAX_MEMO_LEN} bytes")));
    }

    if request.allow_discounts && request.discount_code.is_some() {
        return Err(bad(
            "An invoice carries a discount code or lets the buyer choose one, not both.",
        ));
    }

    // Derived before any field is moved out of the request.
    let options = build_options(&request)?;
    let items = validate_items(request.items, &options)?;

    let name = request.name.unwrap_or_default().trim().to_string();
    if name.len() > MAX_NAME_LEN {
        return Err(bad(format!("name must be at most {MAX_NAME_LEN} bytes")));
    }
    let description = request.description.unwrap_or_default().trim().to_string();
    if description.len() > MAX_DESCRIPTION_LEN {
        return Err(bad(format!(
            "description must be at most {MAX_DESCRIPTION_LEN} bytes"
        )));
    }

    let endpoint = resolve_endpoint(request.endpoint.as_deref(), &request.cluster)?;

    // The SOL option is what older readers understand, so it leads when the
    // seller accepts it; a token-only invoice leads with its first option.
    let id = options
        .iter()
        .find(|option| option.is_sol())
        .or_else(|| options.first())
        .map(|option| option.payment_pda.clone())
        .filter(|pda| !pda.is_empty())
        .unwrap_or_else(|| request.payment_pda.clone());
    let amount_lamports = options
        .iter()
        .find(|option| option.is_sol())
        .map(|option| option.amount)
        .unwrap_or(0);

    Ok(PaymentRecord {
        id: id.clone(),
        // Nothing is on chain yet: a freshly created invoice is unpaid.
        status: PaymentStatus::Pending,
        payment_pda: id,
        seller: request.seller,
        seller_user: None,
        reference: request.reference,
        amount_lamports,
        options,
        memo: request.memo,
        name,
        description,
        items,
        discount: None,
        allow_discounts: request.allow_discounts,
        cluster: request.cluster,
        endpoint,
        settled_by: None,
        created_at_unix: 0,
        settled_at_unix: 0,
        fee: None,
    })
}

#[cfg(test)]
mod tests;
