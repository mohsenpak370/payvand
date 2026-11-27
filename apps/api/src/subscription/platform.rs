//! The platform account that sells the plan, on the configured cluster.

use sha2::{Digest, Sha256};
use solana_pubkey::Pubkey;
use std::str::FromStr;

use crate::auth;
use crate::payments::{bad, ApiResult};

pub(super) fn cluster() -> String {
    std::env::var("SOLANA_CLUSTER").unwrap_or_else(|_| "devnet".to_string())
}

/// The account that sells subscriptions: the platform's own wallet, which
/// receives the price through the program like any seller.
pub fn platform_seller() -> ApiResult<Pubkey> {
    let raw = std::env::var("PLATFORM_SELLER").map_err(|_| {
        bad("Subscriptions are not configured on this server yet (PLATFORM_SELLER).")
    })?;
    Pubkey::from_str(raw.trim())
        .map_err(|_| bad("Subscriptions are not configured correctly (PLATFORM_SELLER)."))
}

/// A reference for one subscription invoice: 32 fresh bytes, so two payments for
/// the same account never collide on their seeds.
pub(super) fn new_reference(user_id: &str) -> Pubkey {
    let mut hasher = Sha256::new();
    hasher.update(user_id.as_bytes());
    hasher.update(auth::random_id().as_bytes());
    hasher.update(auth::unix_now().to_le_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&digest);
    Pubkey::new_from_array(bytes)
}
