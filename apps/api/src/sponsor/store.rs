//! The sponsored settlements the daily caps are counted from, in libSQL.

use crate::auth;
use crate::payments::ApiResult;
use crate::AppState;

/// A rolling day, so a cap cannot be gamed at midnight.
const DAY_SECS: i64 = 24 * 60 * 60;

/// Sponsored settlements in the last rolling day, for one account or the whole
/// deployment. `user_id` is the account because that is what a session proves;
/// the wallet a sponsored payment pays from is named in the row.
pub(super) async fn recent_count(state: &AppState, wallet: Option<&str>) -> ApiResult<i64> {
    let connection = state.db.connect().map_err(auth::internal)?;
    let since = auth::unix_now() - DAY_SECS;
    let mut rows = match wallet {
        Some(wallet) => {
            connection
                .query(
                    "SELECT COUNT(*) FROM sponsored_settlements \
                     WHERE wallet = ?1 AND created_at > ?2",
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

/// Records a settlement whose transaction the cluster has accepted, so the caps
/// count what was actually sponsored rather than what was attempted.
pub(super) async fn record(
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
            "INSERT INTO sponsored_settlements \
             (id, wallet, user_id, cluster, amount, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
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

#[cfg(test)]
mod tests;
