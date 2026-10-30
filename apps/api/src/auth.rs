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
/// Password reset links live for one hour and can be used once.
pub const RESET_TTL_SECS: i64 = 60 * 60;

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

#[derive(Debug, Deserialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

/// The answer to every forgot-password request — identical whether or not
/// the address has an account, so the endpoint cannot be used to discover
/// which emails are registered.
#[derive(Debug, Serialize)]
pub struct ResetSentResponse {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct ResetPasswordRequest {
    pub token: String,
    pub password: String,
}

/// Step 1 of password recovery: email a one-hour, single-use link.
///
/// Follows the same best-effort semantics as Beacon's implementation:
/// delivery runs off the async runtime, failures are only logged server-
/// side, and the response never depends on whether the send succeeded.
pub async fn forgot_password(
    State(state): State<crate::AppState>,
    Json(request): Json<ForgotPasswordRequest>,
) -> ApiResult<Json<ResetSentResponse>> {
    let email = normalize_email(&request.email)?;
    let connection = state.db.connect().map_err(internal)?;

    let generic = ResetSentResponse {
        ok: true,
        message: "If an account exists for that email, a reset link is on its way."
            .to_string(),
    };

    let Some((user, _)) = read_user_with_hash(&connection, &email).await? else {
        return Ok(Json(generic));
    };

    let token = create_reset_record(&connection, &user.id).await?;
    let link = format!("{}/reset-password?token={token}", frontend_base_url());
    let body = format!(
        "Use this link to reset your Payvand password:\n\n{link}\n\n\
         If you did not request this, you can ignore this email."
    );
    let subject = "Reset your Payvand password";

    let recipient = email.clone();
    match tokio::task::spawn_blocking(move || crate::email::send(&recipient, subject, &body)).await
    {
        Ok(Ok(())) => {}
        Ok(Err(error)) => eprintln!("[email] password reset delivery failed: {error}"),
        Err(error) => eprintln!("[email] password reset task failed: {error}"),
    }

    Ok(Json(generic))
}

/// Step 2: exchange the link's token for a new password. A successful
/// reset revokes every session of the account — a changed password means
/// the old sessions can no longer be trusted.
pub async fn reset_password(
    State(state): State<crate::AppState>,
    Json(request): Json<ResetPasswordRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let password = validate_password(&request.password)?;
    let token = request.token.trim();
    if token.is_empty() || token.len() > 128 {
        return Err(invalid_reset_link());
    }

    let connection = state.db.connect().map_err(internal)?;
    let now = unix_now();

    let user_id: String = {
        let mut rows = connection
            .query(
                "SELECT user_id FROM password_resets \
                 WHERE token_hash = ?1 AND expires_at > ?2",
                (token_hash(token), now),
            )
            .await
            .map_err(internal)?;
        match rows.next().await.map_err(internal)? {
            Some(row) => row.get(0).map_err(internal)?,
            None => return Err(invalid_reset_link()),
        }
    };

    let hash = hash_password(&password)?;
    connection
        .execute(
            "UPDATE users SET password_hash = ?1 WHERE id = ?2",
            (hash, user_id.clone()),
        )
        .await
        .map_err(internal)?;
    // Single use: drop this link and any other outstanding one.
    let _ = connection
        .execute(
            "DELETE FROM password_resets WHERE user_id = ?1",
            [user_id.as_str()],
        )
        .await;
    // Every existing session of this account is revoked.
    let _ = connection
        .execute(
            "DELETE FROM sessions WHERE user_id = ?1",
            [user_id.as_str()],
        )
        .await;

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// Creates — and returns — a fresh reset token, replacing any outstanding
/// links so only the newest email works.
async fn create_reset_record(
    connection: &libsql::Connection,
    user_id: &str,
) -> ApiResult<String> {
    let token = hex(&random_bytes::<32>());
    let now = unix_now();
    connection
        .execute("DELETE FROM password_resets WHERE user_id = ?1", [user_id])
        .await
        .map_err(internal)?;
    connection
        .execute(
            "INSERT INTO password_resets (token_hash, user_id, created_at, expires_at) \
             VALUES (?1, ?2, ?3, ?4)",
            (token_hash(&token), user_id, now, now + RESET_TTL_SECS),
        )
        .await
        .map_err(internal)?;
    Ok(token)
}

/// Where reset links point. `FRONTEND_BASE_URL` in production (Beacon
/// names it the same), localhost in development.
fn frontend_base_url() -> String {
    std::env::var("FRONTEND_BASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim_end_matches('/').to_string())
        .unwrap_or_else(|| "http://localhost:3000".to_string())
}

fn invalid_reset_link() -> ApiError {
    ApiError::BadRequest("This reset link is invalid or has expired.".into())
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

pub(crate) fn nullable_text(row: &libsql::Row, index: i32) -> ApiResult<Option<String>> {
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

    /// Handler tests run against a temp-file database: unlike `:memory:`,
    /// every `db.connect()` sees the same schema and data.
    async fn test_state() -> (crate::AppState, std::path::PathBuf) {
        use std::sync::Arc;
        let path =
            std::env::temp_dir().join(format!("payvand-auth-test-{}.db", random_id()));
        let db = libsql::Builder::new_local(&path)
            .build()
            .await
            .expect("temp db");
        crate::db::migrate(&db).await.expect("migrate");
        (crate::AppState { db: Arc::new(db) }, path)
    }

    async fn count(state: &crate::AppState, sql: &str) -> i64 {
        let connection = state.db.connect().expect("connect");
        let mut rows = connection.query(sql, ()).await.expect("query");
        match rows.next().await.expect("row") {
            Some(row) => row.get(0).expect("count"),
            None => 0,
        }
    }

    fn request(email: &str) -> ForgotPasswordRequest {
        ForgotPasswordRequest {
            email: email.to_string(),
        }
    }

    #[tokio::test]
    async fn forgot_password_answers_the_same_for_every_address() {
        let (state, path) = test_state().await;

        let _ = super::signup(
            State(state.clone()),
            Json(SignupRequest {
                email: "ada@example.com".into(),
                password: "correct horse battery staple".into(),
            }),
        )
        .await
        .expect("signup");

        let unknown = super::forgot_password(State(state.clone()), Json(request("nobody@example.com")))
            .await
            .expect("generic answer");
        let known = super::forgot_password(State(state.clone()), Json(request("ada@example.com")))
            .await
            .expect("generic answer");

        // Identical message either way — no account enumeration.
        assert_eq!(unknown.0.message, known.0.message);
        assert!(unknown.0.ok);
        // Exactly one live link, even after two requests.
        assert_eq!(
            count(&state, "SELECT COUNT(*) FROM password_resets").await,
            1
        );
        // The unknown address created nothing.

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn reset_flow_swaps_the_password_and_revokes_sessions() {
        let (state, path) = test_state().await;

        let signup = super::signup(
            State(state.clone()),
            Json(SignupRequest {
                email: "ada@example.com".into(),
                password: "correct horse battery staple".into(),
            }),
        )
        .await
        .expect("signup");

        // The real token only ever exists in the email; the test mints one
        // through the same helper the handler uses.
        let connection = state.db.connect().expect("connect");
        let token = create_reset_record(&connection, &signup.0.user.id)
            .await
            .expect("reset record");
        drop(connection);

        let _ = super::reset_password(
            State(state.clone()),
            Json(ResetPasswordRequest {
                token: token.clone(),
                password: "a brand new password".into(),
            }),
        )
        .await
        .expect("reset accepted");

        // Single use: the record is gone, and so is every session.
        assert_eq!(
            count(&state, "SELECT COUNT(*) FROM password_resets").await,
            0
        );
        assert_eq!(count(&state, "SELECT COUNT(*) FROM sessions").await, 0);

        // Old password refused, new password accepted.
        let old = super::signin(
            State(state.clone()),
            Json(SigninRequest {
                email: "ada@example.com".into(),
                password: "correct horse battery staple".into(),
            }),
        )
        .await;
        assert!(old.is_err(), "the old password must stop working");

        let fresh = super::signin(
            State(state.clone()),
            Json(SigninRequest {
                email: "ada@example.com".into(),
                password: "a brand new password".into(),
            }),
        )
        .await;
        assert!(fresh.is_ok(), "the new password must work");

        // Reusing the consumed link fails.
        let reuse = super::reset_password(
            State(state.clone()),
            Json(ResetPasswordRequest {
                token,
                password: "yet another password".into(),
            }),
        )
        .await;
        assert!(reuse.is_err(), "a used link must not work twice");

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn expired_and_bogus_links_are_rejected() {
        let (state, path) = test_state().await;

        let connection = state.db.connect().expect("connect");
        let hash = hash_password("correct horse battery staple").expect("hash");
        connection
            .execute(
                "INSERT INTO users (id, email, password_hash, wallet, created_at) \
                 VALUES ('user-1', 'ada@example.com', ?, NULL, 1)",
                [hash],
            )
            .await
            .expect("insert");
        let token = create_reset_record(&connection, "user-1")
            .await
            .expect("reset record");
        // Age the link past its expiry.
        connection
            .execute("UPDATE password_resets SET expires_at = 1", ())
            .await
            .expect("age");
        drop(connection);

        let expired = super::reset_password(
            State(state.clone()),
            Json(ResetPasswordRequest {
                token,
                password: "a brand new password".into(),
            }),
        )
        .await;
        assert!(expired.is_err(), "an expired link must not work");

        let bogus = super::reset_password(
            State(state.clone()),
            Json(ResetPasswordRequest {
                token: "not-a-real-token".into(),
                password: "a brand new password".into(),
            }),
        )
        .await;
        assert!(bogus.is_err(), "a bogus link must not work");

        let _ = std::fs::remove_file(path);
    }
}
