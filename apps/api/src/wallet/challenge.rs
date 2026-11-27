//! The signed challenge that proves a wallet belongs to whoever asked: issued
//! once, checked against its purpose, and consumed so it cannot be replayed.

use axum::extract::{Json, State};

use crate::auth;
use crate::payments::{self, ApiError, ApiResult};

use super::model::{ChallengePurpose, ChallengeRequest, ChallengeResponse};

/// Challenges live for five minutes and are single use.
pub const CHALLENGE_TTL_SECS: i64 = 300;

/// Step 1: issue the text the wallet must sign.
pub async fn challenge(
    State(state): State<crate::AppState>,
    Json(request): Json<ChallengeRequest>,
) -> ApiResult<Json<ChallengeResponse>> {
    // Reject malformed addresses before touching the database.
    payments::parse_pubkey(&request.address, "address")?;

    let connection = state.db.connect().map_err(auth::internal)?;
    let now = auth::unix_now();
    let expires_at = now + CHALLENGE_TTL_SECS;
    let nonce = auth::random_id();

    let message = format!(
        "Payvand wants you to {label} with your Solana account:\n\
         {address}\n\
         \n\
         This signature only proves this wallet is yours. It authorises no transaction.\n\
         \n\
         Nonce: {nonce}\n\
         Expires: {expires_at}",
        label = request.purpose.label(),
        address = request.address,
    );

    // One live challenge per address: asking again simply replaces it.
    connection
        .execute(
            "INSERT OR REPLACE INTO wallet_challenges \
             (address, purpose, nonce, message, issued_at, expires_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                request.address.clone(),
                request.purpose.as_str().to_string(),
                nonce,
                message.clone(),
                now,
                expires_at,
            ),
        )
        .await
        .map_err(auth::internal)?;

    // Opportunistic cleanup of anything long expired.
    let _ = connection
        .execute(
            "DELETE FROM wallet_challenges WHERE expires_at <= ?1",
            [now],
        )
        .await;

    Ok(Json(ChallengeResponse {
        message,
        purpose: request.purpose,
        expires_at,
    }))
}

/// Loads the stored challenge, checks it, verifies the signature over its
/// exact text, and consumes it. `purpose` must match the one requested.
pub(super) async fn consume_challenge(
    connection: &libsql::Connection,
    address: &str,
    purpose: ChallengePurpose,
    signature: &str,
) -> ApiResult<()> {
    let now = auth::unix_now();

    let mut rows = connection
        .query(
            "SELECT purpose, message, expires_at FROM wallet_challenges WHERE address = ?1",
            [address],
        )
        .await
        .map_err(auth::internal)?;

    let row = rows.next().await.map_err(auth::internal)?.ok_or_else(|| {
        ApiError::BadRequest("That request expired or was already used — start again.".into())
    })?;

    let stored_purpose: String = row.get(0).map_err(auth::internal)?;
    let message: String = row.get(1).map_err(auth::internal)?;
    let expires_at: i64 = row.get(2).map_err(auth::internal)?;

    if expires_at <= now {
        let _ = connection
            .execute(
                "DELETE FROM wallet_challenges WHERE address = ?1",
                [address],
            )
            .await;
        return Err(ApiError::BadRequest(
            "That request expired — start again.".into(),
        ));
    }

    let wanted = ChallengePurpose::parse(&stored_purpose)
        .ok_or_else(|| ApiError::Internal("unknown challenge purpose".into()))?;
    if wanted != purpose {
        return Err(ApiError::BadRequest(
            "That signature was requested for a different purpose.".into(),
        ));
    }

    verify_signature(address, &message, signature)?;

    // Single use: a consumed challenge can never be replayed.
    let _ = connection
        .execute(
            "DELETE FROM wallet_challenges WHERE address = ?1",
            [address],
        )
        .await;
    Ok(())
}

/// Checks `signature` (base58, 64 bytes) over `message` for `address`.
fn verify_signature(address: &str, message: &str, signature: &str) -> ApiResult<()> {
    let pubkey = payments::parse_pubkey(address, "address")?;
    let bytes: [u8; 64] = bs58::decode(signature)
        .into_vec()
        .map_err(|_| ApiError::BadRequest("signature must be base58 encoded".into()))?
        .try_into()
        .map_err(|_| ApiError::BadRequest("signature must be 64 bytes".into()))?;

    let key = ed25519_dalek::VerifyingKey::from_bytes(&pubkey.to_bytes())
        .map_err(|_| ApiError::BadRequest("address is not a valid public key".into()))?;
    let signature = ed25519_dalek::Signature::from_bytes(&bytes);

    key.verify_strict(message.as_bytes(), &signature)
        .map_err(|_| ApiError::BadRequest("that signature does not match this wallet".into()))
}
