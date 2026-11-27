//! The account handlers: signup, signin, signout and `me`.

use axum::extract::{Json, State};
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};

use super::credentials::{hash_password, normalize_email, validate_password, verify_password};
use super::session::{bearer_token, issue_session, require_user, token_hash};
use super::support::{frontend_base_url, internal, is_unique_violation, random_id, unix_now};
use super::users::{read_user, read_user_with_hash, User};
use crate::payments::{ApiError, ApiResult};

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
            // The caller is told plainly — a signup form that answers "maybe"
            // is worse for the person who forgot they had an account — and the
            // address is told too, so an attempt to reuse it is not silent.
            // What keeps this from being a way to enumerate the user table is
            // the rate limit in front of the endpoint, not the wording.
            if crate::rate_limit::hit("signup-notice", &email, 3, crate::rate_limit::HOUR).is_ok() {
                notice_existing_account(&email).await;
            }
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
    // Per account, not just per address: one caller behind many proxies must
    // not turn a password into a guessing game either.
    crate::rate_limit::hit("signin-email", &email, 10, 15 * 60)?;
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

/// Tells an address that someone tried to sign up with it, with the way back
/// in for the person who owns it. Deliberately fire-and-forget: a mail server
/// hiccup must not change what the caller is told.
async fn notice_existing_account(email: &str) {
    let recipient = email.to_string();
    let body = format!(
        "Someone tried to create a Payvand account with this email address.\n\n\
         If that was you, you already have an account — sign in here:\n\n\
         {}/signin\n\n\
         If you have forgotten your password, use \"Forgot password?\" on that page.\n\n\
         If it was not you, nothing has changed and you can ignore this message.",
        frontend_base_url()
    );
    let subject = "A signup attempt for your Payvand account";
    match tokio::task::spawn_blocking(move || crate::email::send(&recipient, subject, &body)).await
    {
        Ok(Ok(())) => {}
        Ok(Err(error)) => eprintln!("[email] signup notice delivery failed: {error}"),
        Err(error) => eprintln!("[email] signup notice task failed: {error}"),
    }
}

fn invalid_credentials() -> ApiError {
    ApiError::BadRequest("That email and password combination is not valid.".into())
}
