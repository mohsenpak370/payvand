//! Bearer sessions: issuing them, and turning a request's token back into
//! the account it belongs to.

use axum::http::{header, HeaderMap};
use sha2::{Digest, Sha256};

use super::support::{hex, internal, nullable_text, random_id, unix_now};
use super::users::User;
use crate::db::SharedDb;
use crate::payments::{ApiError, ApiResult};

/// Sessions last 30 days.
pub const SESSION_TTL_SECS: i64 = 30 * 24 * 60 * 60;

/// Rejects a request that has no valid session. Used by handlers that must
/// only be reachable by a signed-in account.
pub async fn require_user(db: &SharedDb, headers: &HeaderMap) -> ApiResult<User> {
    let token = bearer_token(headers).ok_or(ApiError::Unauthorized)?;
    resolve_session(db, token)
        .await?
        .ok_or(ApiError::Unauthorized)
}

/// The signed-in account, when the request carries a usable session.
///
/// For the endpoints a stranger may also use: a buyer who scanned a printed code
/// and never signed in still has to be able to pay, and an account is only what
/// we happen to know about them. A missing, expired or malformed token is `None`
/// rather than an error here — the caller's own limits hold either way.
pub async fn optional_user(db: &SharedDb, headers: &HeaderMap) -> Option<User> {
    let token = bearer_token(headers)?;
    resolve_session(db, token).await.ok().flatten()
}

pub(super) fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    value
        .strip_prefix("Bearer ")
        .filter(|token| !token.is_empty())
}

async fn resolve_session(db: &SharedDb, token: &str) -> ApiResult<Option<User>> {
    let connection = db.connect().map_err(internal)?;
    let now = unix_now();
    let mut rows = connection
        .query(
            "SELECT u.id, u.email, u.wallet, u.created_at \
             FROM sessions s JOIN users u ON u.id = s.user_id \
             WHERE s.token_hash = ?1 AND s.expires_at > ?2",
            (token_hash(token), now),
        )
        .await
        .map_err(internal)?;

    match rows.next().await.map_err(internal)? {
        Some(row) => Ok(Some(User {
            id: row.get(0).map_err(internal)?,
            email: row.get(1).map_err(internal)?,
            wallet: nullable_text(&row, 2)?,
            created_at: row.get(3).map_err(internal)?,
        })),
        None => Ok(None),
    }
}

pub(crate) async fn issue_session(
    connection: &libsql::Connection,
    user_id: &str,
) -> ApiResult<String> {
    let token = random_id();
    let now = unix_now();
    connection
        .execute(
            "INSERT INTO sessions (token_hash, user_id, created_at, expires_at) \
             VALUES (?1, ?2, ?3, ?4)",
            (
                token_hash(&token),
                user_id.to_string(),
                now,
                now + SESSION_TTL_SECS,
            ),
        )
        .await
        .map_err(internal)?;
    Ok(token)
}

/// The SHA-256 digest stored in place of a session or reset token.
pub(super) fn token_hash(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

#[cfg(test)]
mod tests;
