//! The shapes the discount endpoints take and answer with.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountCode {
    pub id: String,
    pub code: String,
    pub percent_off: u32,
}

/// What a seller posts to create a code.
#[derive(Debug, Deserialize)]
pub struct CreateDiscountRequest {
    pub code: String,
    pub percent_off: u32,
}

/// What a buyer posts to check a code against an invoice price: the code, the
/// seller's **wallet address** and the price.
///
/// The seller is named by its address, never by an account id: a payment code
/// is data a buyer can edit, and an account id in it would let them point the
/// lookup at their own list. The address is the one the invoice's own PDA was
/// derived from, so it cannot be swapped for someone else's.
#[derive(Debug, Deserialize)]
pub struct ValidateDiscountRequest {
    pub code: String,
    /// Optional only so a page cached before this field existed answers with a
    /// sentence instead of a deserialization error; it is required in practice.
    #[serde(default)]
    pub seller: Option<String>,
    pub price_lamports: u64,
    /// The wallet asking. A code is once per buyer, and the buyer is the wallet
    /// that pays — an account is not needed to open a payment link. Absent on
    /// older pages, which are priced but not counted as a use.
    #[serde(default)]
    pub buyer: Option<String>,
}

/// The priced result the buyer's screen shows above the pay button.
#[derive(Debug, Serialize)]
pub struct DiscountPreview {
    pub code: String,
    pub percent_off: u32,
    pub discounted_lamports: u64,
    pub saved_lamports: u64,
}

/// What records a use of a code: the code, the seller whose list it belongs to
/// and the settled invoice the buyer paid. The buyer is read off that invoice's
/// stored settlement — the request cannot name someone else as the user.
#[derive(Debug, Clone, Deserialize)]
pub struct UseDiscountRequest {
    pub code: String,
    pub seller: String,
    pub invoice: String,
}
