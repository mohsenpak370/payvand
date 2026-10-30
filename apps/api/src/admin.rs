//! Admin API — platform oversight for the people running Payvand.
//!
//! Authorization is configuration, not schema: `ADMIN_EMAILS` (comma
//! separated) is matched against the signed-in account's address, so
//! granting access never needs a migration or a bootstrap row. Every
//! handler re-checks the session first, then the address — a stolen
//! response shape is useless without a live admin session.

use axum::extract::{Json, Path, Query, State};
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};

use crate::auth::{self, User};
use crate::db::SharedDb;
use crate::payments::{ApiError, ApiResult};

/// True when the address is listed in `ADMIN_EMAILS`.
/// Comparison happens on the normalized (trimmed, lowercased) form.
pub fn is_admin(email: &str) -> bool {
    std::env::var("ADMIN_EMAILS")
        .map(|raw| {
            raw.split(',')
                .map(|entry| entry.trim().to_lowercase())
                .any(|entry| !entry.is_empty() && entry == email)
        })
        .unwrap_or(false)
}

/// The signed-in account, when it has admin access.
pub async fn require_admin(db: &SharedDb, headers: &HeaderMap) -> ApiResult<User> {
    let user = auth::require_user(db, headers).await?;
    if is_admin(&user.email) {
        Ok(user)
    } else {
        Err(ApiError::Forbidden(
            "This account does not have admin access.".into(),
        ))
    }
}

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
}

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
const LIST_SQL_TAIL: &str = " ORDER BY u.created_at DESC LIMIT 200";

pub async fn list_users(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Query(query): Query<UserListQuery>,
) -> ApiResult<Json<Vec<AdminUser>>> {
    require_admin(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;
    let now = auth::unix_now();

    let search = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|term| !term.is_empty());

    let mut rows = match search {
        Some(term) => {
            // Literal substring match: wildcards from the input are stripped
            // rather than interpreted.
            let pattern = format!("%{}%", term.replace(['%', '_'], " "));
            let sql = format!("{LIST_SQL_HEAD} WHERE u.email LIKE ?2{LIST_SQL_TAIL}");
            connection
                .query(&sql, (now, pattern))
                .await
                .map_err(auth::internal)?
        }
        None => {
            let sql = format!("{LIST_SQL_HEAD}{LIST_SQL_TAIL}");
            connection
                .query(&sql, [now])
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
    Ok(Json(users))
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
    // Children first — SQLite foreign keys are not guaranteed to be on.
    connection
        .execute("DELETE FROM sessions WHERE user_id = ?1", [id.as_str()])
        .await
        .map_err(auth::internal)?;
    connection
        .execute(
            "DELETE FROM discount_codes WHERE user_id = ?1",
            [id.as_str()],
        )
        .await
        .map_err(auth::internal)?;
    connection
        .execute(
            "DELETE FROM password_resets WHERE user_id = ?1",
            [id.as_str()],
        )
        .await
        .map_err(auth::internal)?;
    let removed = connection
        .execute("DELETE FROM users WHERE id = ?1", [id.as_str()])
        .await
        .map_err(auth::internal)?;

    if removed == 0 {
        return Err(ApiError::NotFound("No account with that id.".into()));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn scalar(connection: &libsql::Connection, sql: &str) -> ApiResult<i64> {
    let mut rows = connection.query(sql, ()).await.map_err(auth::internal)?;
    first_count(&mut rows).await
}

async fn first_count(rows: &mut libsql::Rows) -> ApiResult<i64> {
    match rows.next().await.map_err(auth::internal)? {
        Some(row) => Ok(row.get(0).map_err(auth::internal)?),
        None => Ok(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{random_id, ForgotPasswordRequest, SignupRequest};
    use axum::extract::{Json as JsonExtractor, Query as QueryExtractor, State as StateExtractor};

    async fn test_state() -> (crate::AppState, std::path::PathBuf) {
        let path =
            std::env::temp_dir().join(format!("payvand-admin-test-{}.db", random_id()));
        let db = libsql::Builder::new_local(&path)
            .build()
            .await
            .expect("temp db");
        crate::db::migrate(&db).await.expect("migrate");
        (crate::AppState { db: std::sync::Arc::new(db) }, path)
    }

    fn bearer(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            format!("Bearer {token}").parse().expect("header value"),
        );
        headers
    }

    async fn signup(state: &crate::AppState, email: &str) -> String {
        crate::auth::signup(
            StateExtractor(state.clone()),
            JsonExtractor(SignupRequest {
                email: email.to_string(),
                password: "correct horse battery staple".into(),
            }),
        )
        .await
        .expect("signup")
        .0
        .token
    }

    /// One test owns `ADMIN_EMAILS` end to end: `set_var` is process-global,
    /// so no other test may read it concurrently.
    #[tokio::test]
    async fn admin_gate_stats_search_and_delete() {
        std::env::set_var("ADMIN_EMAILS", " Boss@Example.com, other@example.com ");

        let (state, path) = test_state().await;

        // Parsing: normalization, typos, empty and missing lists.
        assert!(is_admin("boss@example.com"), "normalized list matches");
        assert!(!is_admin("anyone@example.com"));
        assert!(!is_admin("boss@"));
        assert!(!is_admin("boss@example.com "));
        std::env::remove_var("ADMIN_EMAILS");
        assert!(!is_admin("boss@example.com"), "unset list grants nothing");
        assert!(!is_admin(""));
        std::env::set_var("ADMIN_EMAILS", "boss@example.com");

        let boss = signup(&state, "boss@example.com").await;
        let pleb = signup(&state, "pleb@example.com").await;

        let admin_headers = bearer(&boss);
        let user_headers = bearer(&pleb);

        // Signed in but not listed: 403. No session at all: 401.
        let denied = require_admin(&state.db, &user_headers).await;
        assert!(matches!(denied, Err(ApiError::Forbidden(_))));
        let anonymous = require_admin(&state.db, &HeaderMap::new()).await;
        assert!(matches!(anonymous, Err(ApiError::Unauthorized)));

        // Stats see both accounts.
        let stats = stats(StateExtractor(state.clone()), admin_headers.clone())
            .await
            .expect("stats");
        assert_eq!(stats.0.users, 2);
        assert_eq!(stats.0.active_sessions, 2);
        assert_eq!(stats.0.pending_resets, 0);

        // Search narrows the list.
        let all = list_users(
            StateExtractor(state.clone()),
            admin_headers.clone(),
            QueryExtractor(UserListQuery { search: None }),
        )
        .await
        .expect("list");
        assert_eq!(all.0.len(), 2);
        let found = list_users(
            StateExtractor(state.clone()),
            admin_headers.clone(),
            QueryExtractor(UserListQuery {
                search: Some("pleb".into()),
            }),
        )
        .await
        .expect("search");
        assert_eq!(found.0.len(), 1);
        assert_eq!(found.0[0].email, "pleb@example.com");
        assert_eq!(found.0[0].sessions, 1);

        // A non-admin cannot list anything.
        let denied_list = list_users(
            StateExtractor(state.clone()),
            user_headers.clone(),
            QueryExtractor(UserListQuery { search: None }),
        )
        .await;
        assert!(matches!(denied_list, Err(ApiError::Forbidden(_))));

        // Deleting the pleb removes the account and its session.
        let pleb_id = found.0[0].id.clone();
        let _ = delete_user(
            StateExtractor(state.clone()),
            admin_headers.clone(),
            Path(pleb_id.clone()),
        )
        .await
        .expect("delete");
        let after = list_users(
            StateExtractor(state.clone()),
            admin_headers.clone(),
            QueryExtractor(UserListQuery { search: None }),
        )
        .await
        .expect("list");
        assert_eq!(after.0.len(), 1);
        assert_eq!(stats_after_delete(&state).await, 1);

        // Unknown id: 404. Own account: refused.
        let missing = delete_user(
            StateExtractor(state.clone()),
            admin_headers.clone(),
            Path("no-such-user".into()),
        )
        .await;
        assert!(matches!(missing, Err(ApiError::NotFound(_))));
        let boss_id = after.0[0].id.clone();
        let self_delete = delete_user(
            StateExtractor(state.clone()),
            admin_headers.clone(),
            Path(boss_id),
        )
        .await;
        assert!(matches!(self_delete, Err(ApiError::BadRequest(_))));

        // Forgot-password bookkeeping is removed together with the account.
        let _ = crate::auth::signup(
            StateExtractor(state.clone()),
            JsonExtractor(SignupRequest {
                email: "temp@example.com".into(),
                password: "correct horse battery staple".into(),
            }),
        )
        .await;
        let _ = crate::auth::forgot_password(
            StateExtractor(state.clone()),
            JsonExtractor(ForgotPasswordRequest {
                email: "temp@example.com".into(),
            }),
        )
        .await;
        assert!(
            scalar(
                &state.db.connect().expect("connect"),
                "SELECT COUNT(*) FROM password_resets"
            )
            .await
            .expect("count")
                >= 1
        );

        std::env::remove_var("ADMIN_EMAILS");
        let _ = std::fs::remove_file(path);
    }

    async fn stats_after_delete(state: &crate::AppState) -> i64 {
        scalar(
            &state.db.connect().expect("connect"),
            "SELECT COUNT(*) FROM users",
        )
        .await
        .expect("count")
    }
}
