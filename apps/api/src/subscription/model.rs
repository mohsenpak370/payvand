//! The request and response bodies of `/api/subscription`.

use serde::{Deserialize, Serialize};

use super::pricing::Quote;

/// The subscription as one account sees it.
#[derive(Debug, Serialize)]
pub struct SubscriptionStatus {
    pub active: bool,
    /// Unix timestamp of the end of the paid period; 0 when none was ever paid.
    pub expires_at_unix: i64,
    pub currency: Option<String>,
    pub amount: Option<u64>,
    pub price_eur_cents: u64,
    pub period_days: i64,
    /// The plan's price in each accepted currency, converted at `rated_at_unix`.
    /// Empty when no rate could be read.
    pub quotes: Vec<Quote>,
    /// When the rate behind `quotes` was read; 0 when there is no quote.
    pub rated_at_unix: i64,
    /// False when the exchange rate could not be read. The period and the plan
    /// are still answered — only the prices are missing, so the card can still
    /// say where the subscription stands while the buttons wait.
    pub rates_available: bool,
}

#[derive(Debug, Deserialize)]
pub struct InvoiceRequest {
    pub currency: String,
}

/// What the seller's screen needs to pay: the invoice code and the id to confirm.
#[derive(Debug, Serialize)]
pub struct InvoiceResponse {
    pub payment_id: String,
    pub payload: String,
    pub currency: String,
    pub amount: u64,
    pub expires_at_unix: i64,
}

#[derive(Debug, Deserialize)]
pub struct ConfirmRequest {
    pub payment_id: String,
}
