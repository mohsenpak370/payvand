//! Account oversight: dashboard totals, the account table and deletions.

use axum::extract::{Json, Path, Query, State};
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};

use crate::auth;
use crate::payments::{ApiError, ApiResult};

use super::{first_count, require_admin, scalar};

#[derive(Debug, Serialize)]
pub struct Stats {
    pub users: i64,
    pub active_sessions: i64,
    pub discount_codes: i64,
    pub pending_resets: i64,
}

pub async fn stats(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Stats>> {
    require_admin(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;

    Ok(Json(Stats {
        users: scalar(&connection, "SELECT COUNT(*) FROM users").await?,
        active_sessions: {
            let mut rows = connection
                .query(
                    "SELECT COUNT(*) FROM sessions WHERE expires_at > ?1",
                    [auth::unix_now()],
                )
                .await
                .map_err(auth::internal)?;
            first_count(&mut rows).await?
        },
        discount_codes: scalar(&connection, "SELECT COUNT(*) FROM discount_codes").await?,
        pending_resets: scalar(&connection, "SELECT COUNT(*) FROM password_resets").await?,
    }))
}

#[derive(Debug, Deserialize)]
pub struct UserListQuery {
    pub search: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// One page of the account table: bounded so a large table can never turn
/// into one enormous response.
pub(super) const DEFAULT_PAGE: i64 = 20;
pub(super) const MAX_PAGE: i64 = 100;

/// One row of the admin user table: the account plus what it currently
/// holds (live sessions, discount codes).
#[derive(Debug, Serialize)]
pub struct AdminUser {
    pub id: String,
    pub email: String,
    pub wallet: Option<String>,
    pub created_at: i64,
    pub sessions: i64,
    pub codes: i64,
}

const LIST_SQL_HEAD: &str = "SELECT u.id, u.email, u.wallet, u.created_at,
        (SELECT COUNT(*) FROM sessions s WHERE s.user_id = u.id AND s.expires_at > ?1),
        (SELECT COUNT(*) FROM discount_codes d WHERE d.user_id = u.id)
    FROM users u";
// The tails differ because the parameter numbers do: `now` is ?1, the
// search pattern (when present) takes the next slot, then limit and offset.
const LIST_SQL_TAIL_PLAIN: &str = " ORDER BY u.created_at DESC LIMIT ?2 OFFSET ?3";
const LIST_SQL_TAIL_SEARCH: &str = " ORDER BY u.created_at DESC LIMIT ?3 OFFSET ?4";

/// One page of accounts plus the total, so the UI can paginate.
#[derive(Debug, Serialize)]
pub struct UserPage {
    pub items: Vec<AdminUser>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

pub async fn list_users(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Query(query): Query<UserListQuery>,
) -> ApiResult<Json<UserPage>> {
    require_admin(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;
    let now = auth::unix_now();

    let limit = query.limit.unwrap_or(DEFAULT_PAGE).clamp(1, MAX_PAGE);
    let offset = query.offset.unwrap_or(0).max(0);

    // Literal substring match: wildcards from the input are stripped
    // rather than interpreted.
    let pattern = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(|term| format!("%{}%", term.replace(['%', '_'], " ")));

    let total = match &pattern {
        Some(pattern) => {
            let mut rows = connection
                .query(
                    "SELECT COUNT(*) FROM users u WHERE u.email LIKE ?1",
                    [pattern.as_str()],
                )
                .await
                .map_err(auth::internal)?;
            first_count(&mut rows).await?
        }
        None => scalar(&connection, "SELECT COUNT(*) FROM users").await?,
    };

    let mut rows = match &pattern {
        Some(pattern) => {
            let sql = format!("{LIST_SQL_HEAD} WHERE u.email LIKE ?2{LIST_SQL_TAIL_SEARCH}");
            connection
                .query(&sql, (now, pattern.as_str(), limit, offset))
                .await
                .map_err(auth::internal)?
        }
        None => {
            let sql = format!("{LIST_SQL_HEAD}{LIST_SQL_TAIL_PLAIN}");
            connection
                .query(&sql, (now, limit, offset))
                .await
                .map_err(auth::internal)?
        }
    };

    let mut users = Vec::new();
    while let Some(row) = rows.next().await.map_err(auth::internal)? {
        users.push(AdminUser {
            id: row.get(0).map_err(auth::internal)?,
            email: row.get(1).map_err(auth::internal)?,
            wallet: auth::nullable_text(&row, 2)?,
            created_at: row.get(3).map_err(auth::internal)?,
            sessions: row.get(4).map_err(auth::internal)?,
            codes: row.get(5).map_err(auth::internal)?,
        });
    }
    Ok(Json(UserPage {
        items: users,
        total,
        limit,
        offset,
    }))
}

pub async fn delete_user(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let admin = require_admin(&state.db, &headers).await?;
    if admin.id == id {
        return Err(ApiError::BadRequest(
            "You cannot delete your own account.".into(),
        ));
    }

    let connection = state.db.connect().map_err(auth::internal)?;
    // Children first — SQLite foreign keys are not guaranteed to be on, and a
    // row left behind would be an orphan pointing at an account that no longer
    // exists: a shop nobody can open, a saved code nobody can read.
    for sql in [
        "DELETE FROM sessions WHERE user_id = ?1",
        "DELETE FROM discount_codes WHERE user_id = ?1",
        "DELETE FROM password_resets WHERE user_id = ?1",
        "DELETE FROM saved_payments WHERE user_id = ?1",
        "DELETE FROM products WHERE user_id = ?1",
        "DELETE FROM shops WHERE user_id = ?1",
    ] {
        connection
            .execute(sql, [id.as_str()])
            .await
            .map_err(auth::internal)?;
    }
    let removed = connection
        .execute("DELETE FROM users WHERE id = ?1", [id.as_str()])
        .await
        .map_err(auth::internal)?;

    if removed == 0 {
        return Err(ApiError::NotFound("No account with that id.".into()));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}
