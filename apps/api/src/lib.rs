//! Axum router shared by the local server and the Vercel function.

pub mod admin;
pub mod auth;
pub mod config;
pub mod db;
pub mod discounts;
pub mod email;
pub mod faucet;
pub mod payments;
pub mod push;
mod push_budget;
pub mod rate_limit;
pub mod reusable;
pub mod saved;
pub mod shops;
pub mod solana_pay;
pub mod sponsor;
pub mod subscription;
pub mod waitlist;
pub mod wallet;

mod handlers;
mod router;
mod throttle;

pub use router::build_router;

use axum::Router;
use solana_sdk::signer::keypair::Keypair;
use std::sync::Arc;

/// Everything a handler needs. The database handle is shared; libSQL opens
/// connections from it cheaply.
#[derive(Clone)]
pub struct AppState {
    pub db: db::SharedDb,
}

/// A server-held keypair from the environment: the solana CLI's JSON array of
/// 64 bytes, or the same bytes base58 encoded. Absent or unreadable is `None`,
/// never a panic — a misconfigured variable disables a feature instead of
/// taking the API down.
pub(crate) fn keypair_from_env(name: &str) -> Option<Keypair> {
    let raw = std::env::var(name).ok()?;
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if raw.starts_with('[') {
        let bytes: Vec<u8> = serde_json::from_str(raw).ok()?;
        return Keypair::try_from(bytes.as_slice()).ok();
    }
    // Decoded here rather than with `from_base58_string`, which panics on a
    // malformed secret.
    let bytes = bs58::decode(raw).into_vec().ok()?;
    Keypair::try_from(bytes.as_slice()).ok()
}

/// Opens the database, runs the idempotent migrations and builds the router.
pub async fn app() -> Router {
    let database = db::open_and_migrate()
        .await
        .expect("DATABASE_URL could not be opened or migrated");
    build_router(AppState {
        db: Arc::new(database),
    })
}
