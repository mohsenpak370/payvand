//! Shared by the wallet tests: a migrated database, signed-in accounts and
//! deterministic wallets.

use axum::extract::{Json as JsonExtractor, State as StateExtractor};
use axum::http::HeaderMap;
use ed25519_dalek::{Signer, SigningKey};

use crate::auth;

pub(super) async fn test_state() -> (crate::AppState, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!("payvand-wallet-test-{}.db", auth::random_id()));
    let db = libsql::Builder::new_local(&path)
        .build()
        .await
        .expect("temp db");
    crate::db::migrate(&db).await.expect("migrate");
    (
        crate::AppState {
            db: std::sync::Arc::new(db),
        },
        path,
    )
}

pub(super) fn bearer(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {token}").parse().expect("header value"),
    );
    headers
}

pub(super) async fn signup(state: &crate::AppState, email: &str) -> String {
    crate::auth::signup(
        StateExtractor(state.clone()),
        JsonExtractor(crate::auth::SignupRequest {
            email: email.to_string(),
            password: "correct horse battery staple".into(),
        }),
    )
    .await
    .expect("signup")
    .0
    .token
}

/// A deterministic wallet: address in base58, key ready to sign.
pub(super) fn wallet_fixture(seed: u8) -> (String, SigningKey) {
    let key = SigningKey::from_bytes(&[seed; 32]);
    let address = bs58::encode(key.verifying_key().to_bytes()).into_string();
    (address, key)
}

pub(super) fn sign_base58(key: &SigningKey, message: &str) -> String {
    bs58::encode(key.sign(message.as_bytes()).to_bytes()).into_string()
}
