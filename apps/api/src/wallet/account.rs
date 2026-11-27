//! Attaching a wallet to an account, signing in with it, and detaching it.

use axum::extract::{Json, State};
use axum::http::HeaderMap;

use crate::auth;
use crate::payments::{ApiError, ApiResult};

use super::challenge::consume_challenge;
use super::model::{ChallengePurpose, WalletSignatureRequest};

/// Step 2 (link): attach the signed wallet to the signed-in account.
pub async fn link(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Json(request): Json<WalletSignatureRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;

    consume_challenge(
        &connection,
        &request.address,
        ChallengePurpose::Link,
        &request.signature,
    )
    .await?;

    let updated = connection
        .execute(
            "UPDATE users SET wallet = ?1 WHERE id = ?2",
            (request.address.clone(), user.id.clone()),
        )
        .await;
    match updated {
        Ok(0) => return Err(ApiError::Unauthorized),
        Ok(_) => {}
        Err(error) if auth::is_unique_violation(&error) => {
            return Err(ApiError::BadRequest(
                "That wallet is already linked to another account.".into(),
            ));
        }
        Err(error) => return Err(auth::internal(error)),
    }

    let user = auth::read_user(&connection, &user.id)
        .await?
        .ok_or_else(|| ApiError::Internal("the account could not be read back".into()))?;
    Ok(Json(serde_json::json!({ "user": user })))
}

/// Step 2 (signin): open a session for the account this wallet is linked to.
pub async fn signin(
    State(state): State<crate::AppState>,
    Json(request): Json<WalletSignatureRequest>,
) -> ApiResult<Json<auth::AuthResponse>> {
    let connection = state.db.connect().map_err(auth::internal)?;

    consume_challenge(
        &connection,
        &request.address,
        ChallengePurpose::Signin,
        &request.signature,
    )
    .await?;

    let mut rows = connection
        .query(
            "SELECT id FROM users WHERE wallet = ?1",
            [request.address.as_str()],
        )
        .await
        .map_err(auth::internal)?;

    let user_id: String = match rows.next().await.map_err(auth::internal)? {
        Some(row) => row.get(0).map_err(auth::internal)?,
        None => {
            return Err(ApiError::BadRequest(
                "That wallet is not linked to an account yet. \
                 Sign in with email, then link the wallet from your profile."
                    .into(),
            ))
        }
    };

    let user = auth::read_user(&connection, &user_id)
        .await?
        .ok_or_else(|| ApiError::Internal("the account could not be read back".into()))?;
    let token = auth::issue_session(&connection, &user.id).await?;
    Ok(Json(auth::AuthResponse { user, token }))
}

/// Detach the wallet. Email and password keep working, and wallet
/// sign-in stops immediately until it is linked again.
pub async fn unlink(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<serde_json::Value>> {
    let user = auth::require_user(&state.db, &headers).await?;
    if user.wallet.is_none() {
        return Err(ApiError::BadRequest(
            "No wallet is linked to this account.".into(),
        ));
    }

    let connection = state.db.connect().map_err(auth::internal)?;
    connection
        .execute(
            "UPDATE users SET wallet = NULL WHERE id = ?1",
            [user.id.clone()],
        )
        .await
        .map_err(auth::internal)?;

    let user = auth::read_user(&connection, &user.id)
        .await?
        .ok_or_else(|| ApiError::Internal("the account could not be read back".into()))?;
    Ok(Json(serde_json::json!({ "user": user })))
}
