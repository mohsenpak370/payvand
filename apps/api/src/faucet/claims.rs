//! The once-per-account record in `faucet_claims`: reserved before sending,
//! recorded with the signature once sent, released if nothing landed.

use crate::auth;
use crate::payments::{ApiError, ApiResult};
use crate::AppState;

use super::amounts::{sol_lamports, token_base_units};

pub(super) async fn claimed(state: &AppState, user_id: &str) -> ApiResult<bool> {
    let connection = state.db.connect().map_err(auth::internal)?;
    let mut rows = connection
        .query(
            "SELECT 1 FROM faucet_claims WHERE user_id = ?1",
            [user_id.to_string()],
        )
        .await
        .map_err(auth::internal)?;
    Ok(rows.next().await.map_err(auth::internal)?.is_some())
}

/// Claims the slot before sending, so a double tap cannot send twice.
pub(super) async fn reserve(
    state: &AppState,
    user_id: &str,
    wallet: &str,
    cluster: &str,
) -> ApiResult<()> {
    let connection = state.db.connect().map_err(auth::internal)?;
    let inserted = connection
        .execute(
            "INSERT INTO faucet_claims \
             (user_id, wallet, cluster, sol_lamports, token_base_units, signature, claimed_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, '', ?6)",
            (
                user_id.to_string(),
                wallet.to_string(),
                cluster.to_string(),
                sol_lamports() as i64,
                token_base_units() as i64,
                auth::unix_now(),
            ),
        )
        .await;
    match inserted {
        Ok(_) => Ok(()),
        // Only a primary-key clash means "already claimed"; a database that is
        // down or a broken migration is not something to blame on the caller.
        Err(error) if error.to_string().to_ascii_lowercase().contains("unique") => Err(
            ApiError::Forbidden("This account has already claimed its starter tokens.".into()),
        ),
        Err(error) => {
            let message = error.to_string();
            tracing::error!(%message, "could not reserve a starter-token claim");
            Err(ApiError::Internal(
                "The starter tokens could not be prepared. Try again in a moment.".into(),
            ))
        }
    }
}

pub(super) async fn record(state: &AppState, user_id: &str, signature: &str) -> ApiResult<()> {
    let connection = state.db.connect().map_err(auth::internal)?;
    connection
        .execute(
            "UPDATE faucet_claims SET signature = ?2, claimed_at = ?3 WHERE user_id = ?1",
            (user_id.to_string(), signature.to_string(), auth::unix_now()),
        )
        .await
        .map_err(auth::internal)?;
    Ok(())
}

pub(super) async fn release(state: &AppState, user_id: &str) {
    let Ok(connection) = state.db.connect() else {
        return;
    };
    let _ = connection
        .execute(
            "DELETE FROM faucet_claims WHERE user_id = ?1 AND signature = ''",
            [user_id.to_string()],
        )
        .await;
}
