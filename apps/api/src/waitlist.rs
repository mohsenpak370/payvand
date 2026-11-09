//! The waiting list for real-money use.
//!
//! Payvand runs against a Solana test cluster: the instructions are real, the
//! money is not. A seller who wants to take actual payments leaves an address
//! here, and one row per email is the whole feature — no account, no session,
//! nothing to sign in with, because someone on the waiting list has not
//! started using Payvand yet.

use axum::extract::{Json, State};
use serde::{Deserialize, Serialize};

use crate::auth;
use crate::payments::ApiResult;

#[derive(Debug, Deserialize)]
pub struct JoinRequest {
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct JoinResponse {
    pub ok: bool,
    /// True when the address was already on the list. The form says the same
    /// thing either way, so the endpoint never confirms who has joined.
    pub already: bool,
}

/// Adds an address to the list. Public on purpose: the seller who wants live
/// payments has no account yet.
pub async fn join(
    State(state): State<crate::AppState>,
    Json(request): Json<JoinRequest>,
) -> ApiResult<Json<JoinResponse>> {
    // One rule for addresses across the API, so the list matches signup.
    let email = auth::normalize_email(&request.email)?;

    let connection = state.db.connect().map_err(auth::internal)?;
    // The email is the primary key, so joining twice is a no-op rather than a
    // duplicate row or an error the seller has to understand.
    let inserted = connection
        .execute(
            "INSERT INTO waitlist (email, created_at) VALUES (?1, ?2) \
             ON CONFLICT(email) DO NOTHING",
            (email, auth::unix_now()),
        )
        .await
        .map_err(auth::internal)?;

    Ok(Json(JoinResponse {
        ok: true,
        already: inserted == 0,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::random_id;
    use axum::extract::{Json as JsonExtractor, State as StateExtractor};

    async fn test_state() -> (crate::AppState, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!("payvand-waitlist-test-{}.db", random_id()));
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

    async fn join_email(state: &crate::AppState, email: &str) -> JoinResponse {
        join(
            StateExtractor(state.clone()),
            JsonExtractor(JoinRequest {
                email: email.to_string(),
            }),
        )
        .await
        .expect("join")
        .0
    }

    #[tokio::test]
    async fn stores_one_row_per_address() {
        let (state, path) = test_state().await;

        let first = join_email(&state, "  Seller@Example.COM ").await;
        assert!(first.ok);
        assert!(!first.already);

        // The same address in a different case is the same seller.
        let second = join_email(&state, "seller@example.com").await;
        assert!(second.ok);
        assert!(second.already);

        let other = join_email(&state, "another@example.com").await;
        assert!(!other.already);

        let connection = state.db.connect().expect("connection");
        let mut rows = connection
            .query("SELECT email FROM waitlist ORDER BY email", ())
            .await
            .expect("query");
        let mut emails = Vec::new();
        while let Some(row) = rows.next().await.expect("row") {
            emails.push(row.get::<String>(0).expect("email"));
        }
        assert_eq!(emails, vec!["another@example.com", "seller@example.com"]);

        drop(connection);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn rejects_what_is_not_an_address() {
        let (state, path) = test_state().await;

        for bad in [
            "",
            "   ",
            "seller",
            "seller@",
            "@example.com",
            "seller@example",
        ] {
            let result = join(
                StateExtractor(state.clone()),
                JsonExtractor(JoinRequest {
                    email: bad.to_string(),
                }),
            )
            .await;
            assert!(result.is_err(), "{bad:?} should be rejected");
        }

        let _ = std::fs::remove_file(path);
    }
}
