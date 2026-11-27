//! A seller's own list of codes: list, create and delete, signed in.

use axum::extract::{Json, Path, State};
use axum::http::HeaderMap;

use crate::auth;
use crate::payments::{bad, ApiResult};

use super::model::{CreateDiscountRequest, DiscountCode};
use super::rules::{normalize_code, validate_percent};
use super::store::read_percent;

/// Codes per account — small on purpose so the list stays a list, not a system.
const MAX_CODES_PER_USER: usize = 20;

/// The signed-in account's codes, newest first.
pub async fn list(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<DiscountCode>>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;

    let mut rows = connection
        .query(
            "SELECT id, code, percent_off FROM discount_codes \
             WHERE user_id = ?1 ORDER BY created_at DESC",
            [user.id.clone()],
        )
        .await
        .map_err(auth::internal)?;

    let mut codes = Vec::new();
    while let Some(row) = rows.next().await.map_err(auth::internal)? {
        codes.push(DiscountCode {
            id: row.get(0).map_err(auth::internal)?,
            code: row.get(1).map_err(auth::internal)?,
            percent_off: read_percent(&row, 2)?,
        });
    }
    Ok(Json(codes))
}

/// Creates a code for the signed-in account. Only signed-in sellers can.
pub async fn create(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateDiscountRequest>,
) -> ApiResult<Json<DiscountCode>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let code = normalize_code(&request.code)?;
    let percent_off = validate_percent(request.percent_off)?;
    let connection = state.db.connect().map_err(auth::internal)?;

    let mut rows = connection
        .query(
            "SELECT COUNT(*) FROM discount_codes WHERE user_id = ?1",
            [user.id.clone()],
        )
        .await
        .map_err(auth::internal)?;
    let count: i64 = match rows.next().await.map_err(auth::internal)? {
        Some(row) => row.get(0).map_err(auth::internal)?,
        None => 0,
    };
    if count >= MAX_CODES_PER_USER as i64 {
        return Err(bad(format!(
            "You already have the maximum of {MAX_CODES_PER_USER} discount codes."
        )));
    }

    let id = auth::random_id();
    connection
        .execute(
            "INSERT INTO discount_codes (id, user_id, code, percent_off, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (
                id.clone(),
                user.id,
                code.clone(),
                i64::from(percent_off),
                auth::unix_now(),
            ),
        )
        .await
        .map_err(|error| {
            if auth::is_unique_violation(&error) {
                bad(format!("You already have a code called {code}."))
            } else {
                auth::internal(error)
            }
        })?;

    Ok(Json(DiscountCode {
        id,
        code,
        percent_off,
    }))
}

/// Deletes one of the signed-in account's codes.
pub async fn delete(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;
    let deleted = connection
        .execute(
            "DELETE FROM discount_codes WHERE id = ?1 AND user_id = ?2",
            (id, user.id),
        )
        .await
        .map_err(auth::internal)?;
    if deleted == 0 {
        return Err(bad("That discount code no longer exists."));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}
