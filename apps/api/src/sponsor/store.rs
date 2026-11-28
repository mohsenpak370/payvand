//! The sponsored settles the daily caps are counted from, in libSQL.
//!
//! A row is written the moment the relayer signs a settle as fee payer
//! ([`record_signed`]), because a transaction carrying the relayer's signature
//! can be sent by whoever holds it — this API is only one way to the cluster.
//! `submitted_at` is set when this API sent it ([`mark_submitted`]).
//!
//! The two caps read different rows on purpose:
//!
//! - the deployment's cap counts every signature handed out, so it bounds what
//!   the relayer can be made to spend however its transactions reach the chain;
//! - a wallet's cap counts only settles this API submitted for it, which needs
//!   the wallet's own signature — so naming somebody else's wallet in a prepare
//!   request cannot use up their allowance.

use crate::auth;
use crate::payments::ApiResult;
use crate::AppState;

/// A rolling day, so a cap cannot be gamed at midnight.
const DAY_SECS: i64 = 24 * 60 * 60;

/// Sponsored settles in the last rolling day: for one wallet, the ones this API
/// submitted for it; for the whole deployment (`None`), every one the relayer
/// signed.
pub(super) async fn recent_count(state: &AppState, wallet: Option<&str>) -> ApiResult<i64> {
    let connection = state.db.connect().map_err(auth::internal)?;
    let since = auth::unix_now() - DAY_SECS;
    let mut rows = match wallet {
        Some(wallet) => {
            connection
                .query(
                    "SELECT COUNT(*) FROM sponsored_settlements \
                     WHERE wallet = ?1 AND created_at > ?2 AND submitted_at IS NOT NULL",
                    (wallet, since),
                )
                .await
        }
        None => {
            connection
                .query(
                    "SELECT COUNT(*) FROM sponsored_settlements WHERE created_at > ?1",
                    [since],
                )
                .await
        }
    }
    .map_err(auth::internal)?;
    let count: i64 = match rows.next().await.map_err(auth::internal)? {
        Some(row) => row.get(0).map_err(auth::internal)?,
        None => 0,
    };
    Ok(count)
}

/// Records a settle the relayer has just signed as fee payer. `signature` is
/// the relayer's, which is the transaction's id once it lands.
pub(super) async fn record_signed(
    state: &AppState,
    user_id: Option<&str>,
    wallet: &str,
    cluster: &str,
    amount: u64,
    signature: &str,
) -> ApiResult<()> {
    let connection = state.db.connect().map_err(auth::internal)?;
    connection
        .execute(
            "INSERT OR IGNORE INTO sponsored_settlements \
             (id, wallet, user_id, cluster, amount, created_at, submitted_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
            (
                signature.to_string(),
                wallet.to_string(),
                match user_id {
                    Some(id) => libsql::Value::Text(id.to_owned()),
                    None => libsql::Value::Null,
                },
                cluster.to_string(),
                amount as i64,
                auth::unix_now(),
            ),
        )
        .await
        .map_err(auth::internal)?;
    Ok(())
}

/// Whether the relayer signed this settle here — the only settles
/// `POST /api/sponsor/settle` will submit.
pub(super) async fn was_signed(state: &AppState, signature: &str) -> ApiResult<bool> {
    let connection = state.db.connect().map_err(auth::internal)?;
    let mut rows = connection
        .query(
            "SELECT 1 FROM sponsored_settlements WHERE id = ?1",
            [signature],
        )
        .await
        .map_err(auth::internal)?;
    Ok(rows.next().await.map_err(auth::internal)?.is_some())
}

/// Marks a signed settle as sent by this API, which is what a wallet's own cap
/// counts.
pub(super) async fn mark_submitted(state: &AppState, signature: &str) -> ApiResult<()> {
    let connection = state.db.connect().map_err(auth::internal)?;
    connection
        .execute(
            "UPDATE sponsored_settlements SET submitted_at = ?2 WHERE id = ?1",
            (signature.to_string(), auth::unix_now()),
        )
        .await
        .map_err(auth::internal)?;
    Ok(())
}

#[cfg(test)]
mod tests;
