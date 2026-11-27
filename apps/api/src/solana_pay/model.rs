//! The bodies the Solana Pay spec exchanges with a wallet.

use serde::{Deserialize, Serialize};

/// The wallet's account, as the Solana Pay spec sends it in the POST body.
#[derive(Debug, Deserialize)]
pub struct AccountRequest {
    pub account: String,
}

/// Some wallets put the account in the query string instead of the body.
#[derive(Debug, Default, Deserialize)]
pub struct AccountQuery {
    pub account: Option<String>,
}

/// `GET` response: what the wallet shows before asking for the transaction.
/// The spec requires `icon`; `message` is the optional human note.
#[derive(Debug, Serialize)]
pub struct RequestInfo {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub message: String,
}

/// `POST` response: the transaction the wallet signs and sends.
#[derive(Debug, Serialize)]
pub struct TransactionRequest {
    pub transaction: String,
    pub message: String,
}
