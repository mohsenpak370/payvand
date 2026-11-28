//! The request and response bodies of `/api/sponsor`.

use serde::{Deserialize, Serialize};

/// The invoice a sponsored settle pays, as the buyer's page knows it. The relayer
/// builds the whole settlement from these fields and nothing else.
#[derive(Debug, Deserialize)]
pub struct SettleFields {
    pub buyer: String,
    pub seller: String,
    pub reference: String,
    /// The invoice's mint. Empty means a SOL invoice, which has no token
    /// accounts and is settled by `settle_payment` instead.
    #[serde(default)]
    pub mint: String,
    pub amount: u64,
    #[serde(default)]
    pub memo: String,
    pub cluster: String,
    pub endpoint: Option<String>,
}

/// `POST /api/sponsor/prepare` — the invoice to build a settle for.
pub type PrepareRequest = SettleFields;

/// The settle the relayer built and signed as fee payer, base64 wire bytes. The
/// buyer's slot is empty; the wallet signs it and the page sends it back to
/// `POST /api/sponsor/settle` unchanged.
#[derive(Debug, Serialize)]
pub struct PrepareResponse {
    pub transaction: String,
    /// The relayer's signature, which is the transaction's id once it lands.
    pub signature: String,
}

/// `POST /api/sponsor/settle` — the prepared transaction with the buyer's
/// signature added, and the invoice it was prepared for.
#[derive(Debug, Deserialize)]
pub struct SettleRequest {
    /// The fully signed wire transaction, base64. Its message must be the one
    /// `prepare` built for these fields, byte for byte.
    pub transaction: String,
    #[serde(flatten)]
    pub fields: SettleFields,
}

#[derive(Debug, Serialize)]
pub struct SettleResponse {
    pub signature: String,
}

#[derive(Debug, Serialize)]
pub struct InfoResponse {
    pub available: bool,
    pub address: Option<String>,
    /// Lamports the relayer fronts for one settle — both record rents and the
    /// two signatures — all of which the same transaction refunds to it.
    pub needs_lamports: u64,
    pub per_wallet_daily: u32,
}

/// `?wallet=` — the address the daily cap is counted against. A signed-in
/// caller's linked wallet stands in when the query omits one.
#[derive(Debug, Deserialize)]
pub struct InfoQuery {
    pub wallet: Option<String>,
}
