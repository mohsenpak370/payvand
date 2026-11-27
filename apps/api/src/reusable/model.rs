//! What a seller posts to make a reusable code, and what they get back.

use serde::{Deserialize, Serialize};

/// Where a reusable code lives in the web app: the path a QR encodes, with the
/// app's own origin in front of it.
pub const LINK_PATH_PREFIX: &str = "/r/";

/// A stored offer, as the seller sees it back.
#[derive(Debug, Clone, Serialize)]
pub struct ReusableLink {
    pub id: String,
    pub seller: String,
    pub cluster: String,
    pub currency: String,
    /// Price in the currency's base units: lamports for SOL, the token's own
    /// units for a stablecoin.
    pub amount: u64,
    pub memo: String,
    pub name: String,
    pub description: String,
    /// The code's own path, so the seller's screen never has to know the shape.
    pub path: String,
    pub created_at_unix: i64,
}

/// What the seller posts to make a reusable code.
#[derive(Debug, Deserialize)]
pub struct CreateReusableRequest {
    pub seller: String,
    pub currency: String,
    pub amount: u64,
    #[serde(default)]
    pub memo: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// The shop the code belongs to, when it was made on a shop's page.
    #[serde(default)]
    pub shop_id: Option<String>,
    pub cluster: String,
    #[serde(default)]
    pub endpoint: Option<String>,
}
