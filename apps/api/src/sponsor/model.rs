//! The request and response bodies of `/api/sponsor`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct SettleRequest {
    /// The buyer-signed wire transaction, base64. Its fee payer is the sponsor
    /// and its message must be exactly the one this module rebuilds.
    pub transaction: String,
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

#[derive(Debug, Serialize)]
pub struct SettleResponse {
    pub signature: String,
}

#[derive(Debug, Serialize)]
pub struct InfoResponse {
    pub available: bool,
    pub address: Option<String>,
    /// Lamports a wallet needs before it can settle a token invoice itself.
    pub needs_lamports: u64,
    pub per_wallet_daily: u32,
}

/// `?wallet=` — the address the daily cap is counted against. A signed-in
/// caller's linked wallet stands in when the query omits one.
#[derive(Debug, Deserialize)]
pub struct InfoQuery {
    pub wallet: Option<String>,
}
