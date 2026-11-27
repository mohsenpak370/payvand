//! Password recovery: an emailed one-hour, single-use link, then the reset
//! that consumes it.

use axum::extract::{Json, State};
use serde::{Deserialize, Serialize};

use super::credentials::{hash_password, normalize_email, validate_password};
use super::session::token_hash;
use super::support::{frontend_base_url, hex, internal, random_bytes, unix_now};
use super::users::read_user_with_hash;
use crate::payments::{ApiError, ApiResult};

/// Password reset links live for one hour and can be used once.
pub const RESET_TTL_SECS: i64 = 60 * 60;

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
    // Bounds the mail this endpoint can be made to send to one address; the
    // answer stays the same either way.
    let may_send =
        crate::rate_limit::hit("forgot-email", &email, 3, crate::rate_limit::HOUR).is_ok();
    let connection = state.db.connect().map_err(internal)?;

    let generic = ResetSentResponse {
        ok: true,
        message: "If an account exists for that email, a reset link is on its way.".to_string(),
    };

    let Some((user, _)) = read_user_with_hash(&connection, &email).await? else {
        return Ok(Json(generic));
    };
    if !may_send {
        return Ok(Json(generic));
    }

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
pub(super) async fn create_reset_record(
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

fn invalid_reset_link() -> ApiError {
    ApiError::BadRequest("This reset link is invalid or has expired.".into())
}

#[cfg(test)]
mod tests;
