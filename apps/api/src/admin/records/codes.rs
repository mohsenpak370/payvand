//! Admin view and removal of discount codes.

use axum::extract::{Json, Path, Query, State};
use axum::http::HeaderMap;
use serde::Serialize;

use crate::auth;
use crate::payments::{ApiError, ApiResult};

use super::super::{first_count, require_admin, scalar};
use super::{record_page, record_search, RecordQuery};

#[derive(Debug, Serialize)]
pub struct AdminCode {
    pub id: String,
    pub code: String,
    pub percent_off: u32,
    pub owner: String,
    pub created_at: i64,
}

#[derive(Debug, Serialize)]
pub struct AdminCodePage {
    pub items: Vec<AdminCode>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

pub async fn list_codes(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Query(query): Query<RecordQuery>,
) -> ApiResult<Json<AdminCodePage>> {
    require_admin(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;
    let (limit, offset) = record_page(&query);
    let pattern = record_search(&query);

    let total = match &pattern {
        Some(pattern) => {
            let mut rows = connection
                .query(
                    "SELECT COUNT(*) FROM discount_codes d JOIN users u ON u.id = d.user_id \
                     WHERE d.code LIKE ?1 OR u.email LIKE ?1",
                    [pattern.as_str()],
                )
                .await
                .map_err(auth::internal)?;
            first_count(&mut rows).await?
        }
        None => scalar(&connection, "SELECT COUNT(*) FROM discount_codes").await?,
    };

    let select = "SELECT d.id, d.code, d.percent_off, u.email, d.created_at \
        FROM discount_codes d JOIN users u ON u.id = d.user_id";
    let mut rows = match &pattern {
        Some(pattern) => {
            let sql = format!(
                "{select} WHERE d.code LIKE ?1 OR u.email LIKE ?1 \
                 ORDER BY d.created_at DESC, d.id ASC LIMIT ?2 OFFSET ?3"
            );
            connection
                .query(&sql, (pattern.as_str(), limit, offset))
                .await
                .map_err(auth::internal)?
        }
        None => {
            let sql = format!("{select} ORDER BY d.created_at DESC, d.id ASC LIMIT ?1 OFFSET ?2");
            connection
                .query(&sql, (limit, offset))
                .await
                .map_err(auth::internal)?
        }
    };

    let mut items = Vec::new();
    while let Some(row) = rows.next().await.map_err(auth::internal)? {
        let percent: i64 = row.get(2).map_err(auth::internal)?;
        items.push(AdminCode {
            id: row.get(0).map_err(auth::internal)?,
            code: row.get(1).map_err(auth::internal)?,
            percent_off: u32::try_from(percent).unwrap_or(0),
            owner: row.get(3).map_err(auth::internal)?,
            created_at: row.get(4).map_err(auth::internal)?,
        });
    }

    Ok(Json(AdminCodePage {
        items,
        total,
        limit,
        offset,
    }))
}

pub async fn delete_code(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    require_admin(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;
    let removed = connection
        .execute("DELETE FROM discount_codes WHERE id = ?1", [id.as_str()])
        .await
        .map_err(auth::internal)?;
    if removed == 0 {
        return Err(ApiError::NotFound("No code with that id.".into()));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}
