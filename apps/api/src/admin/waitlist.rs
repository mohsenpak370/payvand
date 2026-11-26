//! The seller waitlist, listed and pruned by an operator.

use axum::extract::{Json, Path, State};
use axum::http::HeaderMap;
use serde::Serialize;

use crate::auth;
use crate::payments::{ApiError, ApiResult};

use super::{require_admin, scalar};

/// One address waiting for a live cluster, with the day it arrived.
#[derive(Debug, Serialize)]
pub struct WaitlistEntry {
    pub email: String,
    pub created_at_unix: i64,
}

/// The list plus its total. The list is small by nature, and the cap keeps a
/// spammed table from turning into one enormous response.
#[derive(Debug, Serialize)]
pub struct WaitlistPage {
    pub items: Vec<WaitlistEntry>,
    pub total: i64,
}

const MAX_WAITLIST_ROWS: i64 = 500;

/// Newest first, so the people who just asked to be told are at the top.
pub async fn waitlist(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<WaitlistPage>> {
    require_admin(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;

    let total = scalar(&connection, "SELECT COUNT(*) FROM waitlist").await?;
    let mut rows = connection
        .query(
            "SELECT email, created_at FROM waitlist \
             ORDER BY created_at DESC, email ASC LIMIT ?1",
            [MAX_WAITLIST_ROWS],
        )
        .await
        .map_err(auth::internal)?;

    let mut items = Vec::new();
    while let Some(row) = rows.next().await.map_err(auth::internal)? {
        items.push(WaitlistEntry {
            email: row.get(0).map_err(auth::internal)?,
            created_at_unix: row.get(1).map_err(auth::internal)?,
        });
    }

    Ok(Json(WaitlistPage { items, total }))
}

/// Removes one address. The list is made of other people's addresses, so an
/// address that asked to be forgotten should not need a database console.
pub async fn remove_waitlist(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Path(email): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    require_admin(&state.db, &headers).await?;
    // Same normalization as joining, so the path form matches the stored row.
    let email = auth::normalize_email(&email)?;
    let connection = state.db.connect().map_err(auth::internal)?;
    let removed = connection
        .execute("DELETE FROM waitlist WHERE email = ?1", [email.as_str()])
        .await
        .map_err(auth::internal)?;

    if removed == 0 {
        return Err(ApiError::NotFound(
            "That address is not on the waiting list.".into(),
        ));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}
