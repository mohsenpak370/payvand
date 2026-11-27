//! The request and response bodies of the wallet routes.

use serde::{Deserialize, Serialize};

use crate::payments::{self, PaymentStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChallengePurpose {
    /// Attach the wallet to the account that is signed in right now.
    Link,
    /// Open a session for the account this wallet is linked to.
    Signin,
}

impl ChallengePurpose {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Link => "link",
            Self::Signin => "sign in",
        }
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Link => "link",
            Self::Signin => "signin",
        }
    }

    pub(super) fn parse(raw: &str) -> Option<Self> {
        match raw {
            "link" => Some(Self::Link),
            "signin" => Some(Self::Signin),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ChallengeRequest {
    pub address: String,
    pub purpose: ChallengePurpose,
}

#[derive(Debug, Serialize)]
pub struct ChallengeResponse {
    /// The exact text to hand the wallet — never rebuilt client side.
    pub message: String,
    pub purpose: ChallengePurpose,
    pub expires_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct WalletSignatureRequest {
    pub address: String,
    pub signature: String,
}

#[derive(Debug, Serialize)]
pub struct MyTransaction {
    pub id: String,
    /// `"sent"` or `"received"`.
    pub direction: String,
    /// Who paid, or who was paid.
    pub counterparty: String,
    /// `sol`, `usdc` or `eurc`: the currency this invoice was paid in.
    pub currency: String,
    /// The amount in that currency's base units, which is what a total per
    /// coin needs; `amount_lamports` is zero on a token invoice.
    pub amount: u64,
    /// Decimals of `amount`: 9 for SOL, 6 for the stablecoins.
    pub decimals: u8,
    pub amount_lamports: u64,
    pub memo: String,
    /// What the invoice was called and what it was for, when it was made in
    /// this app. The chain stores neither, so an invoice made elsewhere — or
    /// before this was recorded — has empty strings.
    pub name: String,
    pub description: String,
    /// The receipt lines the invoice was priced from in the seller's shop, in
    /// the order they were written. Empty for a hand-priced invoice, and for
    /// one made before the app started keeping them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<payments::PaymentItem>,
    pub status: PaymentStatus,
    pub created_at_unix: i64,
    pub settled_at_unix: i64,
}

#[derive(Debug, Serialize)]
pub struct TransactionsResponse {
    pub wallet: String,
    pub items: Vec<MyTransaction>,
    /// How many the wallet has in total, before the limit was applied.
    pub total: usize,
    pub cluster: String,
}

/// `?scan=1` asks for the program scan even when our rows could answer.
#[derive(Debug, Deserialize)]
pub struct FeedQuery {
    /// Only the allowance needs it today: a payment signed by a session key is
    /// one this API never wrote an invoice row for, and which wallet owns that
    /// key is knowable only from the chain.
    #[serde(default)]
    pub scan: Option<String>,
}
