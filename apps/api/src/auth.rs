//! Email + password accounts with database-backed sessions.
//!
//! Sessions are opaque 32 byte random tokens returned in the response body;
//! the client sends them back as `Authorization: Bearer <token>`. Only the
//! SHA-256 digest of a token is stored, so a leaked database dump cannot be
//! replayed as a live session.
//!
//! `users.wallet` is intentionally nullable: it is the slot for linking a
//! Solana address later so the same account can also sign in with a wallet.

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use axum::extract::{Json, State};
use axum::http::{header, HeaderMap};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::db::SharedDb;
use crate::payments::{ApiError, ApiResult};

/// Sessions last 30 days.
pub const SESSION_TTL_SECS: i64 = 30 * 24 * 60 * 60;
pub const MIN_PASSWORD_LEN: usize = 8;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub wallet: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct SignupRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct SigninRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub user: User,
    pub token: String,
}

pub async fn signup(
    State(state): State<crate::AppState>,
    Json(request): Json<SignupRequest>,
) -> ApiResult<Json<AuthResponse>> {
    let email = normalize_email(&request.email)?;
    let password = validate_password(&request.password)?;
    let hash = hash_password(&password)?;

    let connection = state.db.connect().map_err(internal)?;
    let now = unix_now();
    let user_id = random_id();

    let inserted = connection
        .execute(
            "INSERT INTO users (id, email, password_hash, wallet, created_at) \
             VALUES (?1, ?2, ?3, NULL, ?4)",
            (user_id.clone(), email.clone(), hash, now),
        )
        .await;

    match inserted {
        Ok(_) => {}
        Err(error) if is_unique_violation(&error) => {
            return Err(ApiError::BadRequest(
                "An account with that email already exists.".to_string(),
            ));
        }
        Err(error) => return Err(internal(error)),
    }

    let user = read_user(&connection, &user_id)
        .await?
        .ok_or_else(|| ApiError::Internal("the new account could not be read back".into()))?;
    let token = issue_session(&connection, &user.id).await?;

    Ok(Json(AuthResponse { user, token }))
}

pub async fn signin(
    State(state): State<crate::AppState>,
    Json(request): Json<SigninRequest>,
) -> ApiResult<Json<AuthResponse>> {
    let email = normalize_email(&request.email)?;
    let connection = state.db.connect().map_err(internal)?;

    let Some((user, hash)) = read_user_with_hash(&connection, &email).await? else {
        return Err(invalid_credentials());
    };

    if !verify_password(&request.password, &hash) {
        return Err(invalid_credentials());
    }

    // Opportunistic cleanup keeps the sessions table from growing forever.
    let _ = connection
        .execute("DELETE FROM sessions WHERE expires_at <= ?", [unix_now()])
        .await;

    let token = issue_session(&connection, &user.id).await?;
    Ok(Json(AuthResponse { user, token }))
}

pub async fn signout(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<serde_json::Value>> {
    if let Some(token) = bearer_token(&headers) {
        let connection = state.db.connect().map_err(internal)?;
        let _ = connection
            .execute(
                "DELETE FROM sessions WHERE token_hash = ?",
                [token_hash(token)],
            )
            .await;
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn me(State(state): State<crate::AppState>, headers: HeaderMap) -> ApiResult<Json<User>> {
    let user = require_user(&state.db, &headers).await?;
    Ok(Json(user))
}

/// Rejects a request that has no valid session. Used by handlers that must
/// only be reachable by a signed-in account.
pub async fn require_user(db: &SharedDb, headers: &HeaderMap) -> ApiResult<User> {
    let token = bearer_token(headers).ok_or(ApiError::Unauthorized)?;
    resolve_session(db, token)
        .await?
        .ok_or(ApiError::Unauthorized)
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
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

async fn issue_session(connection: &libsql::Connection, user_id: &str) -> ApiResult<String> {
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

async fn read_user(connection: &libsql::Connection, id: &str) -> ApiResult<Option<User>> {
    let mut rows = connection
        .query(
            "SELECT id, email, wallet, created_at FROM users WHERE id = ?",
            [id],
        )
        .await
        .map_err(internal)?;
    read_user_row(&mut rows).await
}

async fn read_user_with_hash(
    connection: &libsql::Connection,
    email: &str,
) -> ApiResult<Option<(User, String)>> {
    let mut rows = connection
        .query(
            "SELECT id, email, wallet, created_at, password_hash FROM users WHERE email = ?",
            [email],
        )
        .await
        .map_err(internal)?;

    match rows.next().await.map_err(internal)? {
        Some(row) => {
            let user = User {
                id: row.get(0).map_err(internal)?,
                email: row.get(1).map_err(internal)?,
                wallet: nullable_text(&row, 2)?,
                created_at: row.get(3).map_err(internal)?,
            };
            let hash: String = row.get(4).map_err(internal)?;
            Ok(Some((user, hash)))
        }
        None => Ok(None),
    }
}

async fn read_user_row(rows: &mut libsql::Rows) -> ApiResult<Option<User>> {
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

fn nullable_text(row: &libsql::Row, index: i32) -> ApiResult<Option<String>> {
    match row.get::<libsql::Value>(index).map_err(internal)? {
        libsql::Value::Null => Ok(None),
        libsql::Value::Text(text) => Ok(Some(text)),
        other => Err(ApiError::Internal(format!(
            "unexpected value in column {index}: {other:?}"
        ))),
    }
}

pub fn hash_password(password: &str) -> ApiResult<String> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|error| ApiError::Internal(format!("could not hash the password: {error}")))
}

pub fn verify_password(password: &str, stored: &str) -> bool {
    PasswordHash::new(stored)
        .map(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
        .unwrap_or(false)
}

fn normalize_email(email: &str) -> ApiResult<String> {
    let normalized = email.trim().to_lowercase();
    if !normalized.is_empty()
        && normalized.len() <= 254
        && !normalized.chars().any(char::is_whitespace)
        && normalized.matches('@').count() == 1
        && {
            let (local, domain) = normalized.split_once('@').expect("one @ above");
            !local.is_empty() && domain.contains('.') && !domain.starts_with('.')
        }
    {
        return Ok(normalized);
    }
    Err(ApiError::BadRequest("Enter a valid email address.".into()))
}

fn validate_password(password: &str) -> ApiResult<String> {
    if password.len() >= MIN_PASSWORD_LEN {
        return Ok(password.to_string());
    }
    Err(ApiError::BadRequest(format!(
        "The password must be at least {MIN_PASSWORD_LEN} characters."
    )))
}

fn invalid_credentials() -> ApiError {
    ApiError::BadRequest("That email and password combination is not valid.".into())
}

pub(crate) fn is_unique_violation(error: &libsql::Error) -> bool {
    error.to_string().contains("UNIQUE constraint failed")
}

fn token_hash(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

/// 16 random bytes, hex encoded — used for user ids and session tokens.
pub(crate) fn random_id() -> String {
    hex(&random_bytes::<16>())
}

fn random_bytes<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    // `getrandom` is already in the tree through argon2; it reads the OS CSPRNG.
    getrandom::fill(&mut bytes).expect("the operating system RNG failed");
    bytes
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

pub(crate) fn internal(error: impl std::fmt::Display) -> ApiError {
    ApiError::Internal(error.to_string())
}

pub(crate) fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_malformed_emails() {
        for bad in [
            "",
            "nope",
            "a@b",
            "a@@b.com",
            "a b@c.com",
            "@b.com",
            "a@.com",
        ] {
            assert!(normalize_email(bad).is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    fn normalizes_a_valid_email() {
        let email = normalize_email("  Ada@Example.COM ").expect("valid");
        assert_eq!(email, "ada@example.com");
    }

    #[test]
    fn enforces_the_password_length() {
        assert!(validate_password("short").is_err());
        assert!(validate_password("longenough").is_ok());
    }

    #[test]
    fn hashes_and_verifies_passwords() {
        let hash = hash_password("correct horse battery staple").expect("hash");
        assert!(verify_password("correct horse battery staple", &hash));
        assert!(!verify_password("wrong password", &hash));
        assert!(!verify_password(
            "correct horse battery staple",
            "not-a-hash"
        ));
    }

    #[test]
    fn session_tokens_are_stored_hashed() {
        let token = random_id();
        assert_ne!(token_hash(&token), token);
        assert_eq!(token_hash(&token), token_hash(&token));
        assert_ne!(random_id(), random_id());
    }

    #[tokio::test]
    async fn migrates_and_reads_users_back() {
        let (_db, connection) = crate::db::in_memory().await.expect("in-memory schema");

        let hash = hash_password("correct horse battery staple").expect("hash");
        connection
            .execute(
                "INSERT INTO users (id, email, password_hash, wallet, created_at) \
                 VALUES ('user-1', 'ada@example.com', ?, NULL, 1)",
                [hash],
            )
            .await
            .expect("insert");

        let (user, stored) = read_user_with_hash(&connection, "ada@example.com")
            .await
            .expect("query")
            .expect("user exists");
        assert_eq!(user.id, "user-1");
        assert_eq!(user.wallet, None);
        assert!(verify_password("correct horse battery staple", &stored));
        assert!(read_user_with_hash(&connection, "missing@example.com")
            .await
            .expect("query")
            .is_none());
    }
}
