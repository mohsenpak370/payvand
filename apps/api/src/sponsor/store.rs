//! The settles the relayer signed, and what became of each, in libSQL.
//!
//! A row is written the moment the relayer signs a settle as fee payer
//! ([`record_signed`]): from then on whoever holds the transaction can send it,
//! and this API is only one way to the cluster. Once the transaction can no
//! longer land (its blockhash has expired, [`RESOLVE_AFTER_SECS`]), the row is
//! resolved against the cluster ([`set_outcome`]):
//!
//! - `landed`: the settle succeeded, and the program refunded the relayer in the
//!   same transaction. It cost nothing, and **no cap counts it**.
//! - `expired`: it never reached the chain (the buyer closed the wallet prompt).
//!   Nothing was spent; nothing counts it either.
//! - `failed`: it landed and failed. The cluster still charged the relayer the
//!   fee of both signatures, and nothing refunds that: this is the one real
//!   loss, [`LOSS_PER_FAILURE`] lamports, and what the limits count.
//!
//! A row the API could not resolve yet (the cluster did not answer) counts as a
//! failure until it is resolved, so an outage cannot hide losses. See
//! `docs/payment-costs.md`.

use crate::auth;
use crate::payments::ApiResult;
use crate::AppState;

/// A rolling day, so a limit cannot be gamed at midnight.
const DAY_SECS: i64 = 24 * 60 * 60;

/// How long after signing a settle can still land: the blockhash it carries is
/// valid for 150 blocks, about 60–90 seconds. Twice that, to be safe; until
/// then a row is in flight and its outcome is not asked for.
pub(super) const RESOLVE_AFTER_SECS: i64 = 180;

/// What a settle that landed and failed cost the relayer: the base fee of its
/// two signatures. The relayer signs no priority price (invariant 3), so this is
/// exact.
pub(super) const LOSS_PER_FAILURE: i64 = 2 * 5_000;

/// What became of one signed settle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    Landed,
    Failed,
    Expired,
}

impl Outcome {
    fn as_str(self) -> &'static str {
        match self {
            Outcome::Landed => "landed",
            Outcome::Failed => "failed",
            Outcome::Expired => "expired",
        }
    }
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
             (id, wallet, user_id, cluster, amount, created_at, submitted_at, outcome) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL)",
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

/// Marks a signed settle as sent by this API. Informational: it tells an
/// operator which settles went through `settle` and which were sent elsewhere.
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

/// Signed settles that can no longer land and have no outcome yet, oldest
/// first, as `(signature, cluster)`.
pub(super) async fn unresolved(state: &AppState, limit: usize) -> ApiResult<Vec<(String, String)>> {
    let connection = state.db.connect().map_err(auth::internal)?;
    let mut rows = connection
        .query(
            &format!(
                "SELECT id, cluster FROM sponsored_settlements \
                 WHERE outcome IS NULL AND created_at <= ?1 AND created_at > ?2 \
                 ORDER BY created_at LIMIT {limit}"
            ),
            (
                auth::unix_now() - RESOLVE_AFTER_SECS,
                auth::unix_now() - DAY_SECS,
            ),
        )
        .await
        .map_err(auth::internal)?;
    let mut found = Vec::new();
    while let Some(row) = rows.next().await.map_err(auth::internal)? {
        found.push((
            row.get::<String>(0).map_err(auth::internal)?,
            row.get::<String>(1).map_err(auth::internal)?,
        ));
    }
    Ok(found)
}

/// What the cluster said about one signed settle.
pub(super) async fn set_outcome(
    state: &AppState,
    signature: &str,
    outcome: Outcome,
) -> ApiResult<()> {
    let connection = state.db.connect().map_err(auth::internal)?;
    connection
        .execute(
            "UPDATE sponsored_settlements SET outcome = ?2 WHERE id = ?1",
            (signature.to_string(), outcome.as_str()),
        )
        .await
        .map_err(auth::internal)?;
    Ok(())
}

/// Settles in the last rolling day that cost the relayer money, for one wallet
/// or the deployment (`None`): every `failed` one, and every one old enough to
/// have an outcome that does not have one yet — unknown counts as lost until
/// the cluster says otherwise. Completed and expired settles are never here.
pub(super) async fn losses(state: &AppState, wallet: Option<&str>) -> ApiResult<i64> {
    let filter = "created_at > ?1 AND (outcome = 'failed' \
                  OR (outcome IS NULL AND created_at <= ?2))";
    let since = auth::unix_now() - DAY_SECS;
    let settled_by = auth::unix_now() - RESOLVE_AFTER_SECS;
    count(
        state,
        filter,
        wallet,
        vec![
            libsql::Value::Integer(since),
            libsql::Value::Integer(settled_by),
        ],
    )
    .await
}

/// Settles signed recently enough that they may still land — the relayer's
/// exposure right now, which no outcome has bounded yet.
pub(super) async fn in_flight(state: &AppState, wallet: Option<&str>) -> ApiResult<i64> {
    count(
        state,
        "created_at > ?1",
        wallet,
        vec![libsql::Value::Integer(
            auth::unix_now() - RESOLVE_AFTER_SECS,
        )],
    )
    .await
}

async fn count(
    state: &AppState,
    filter: &str,
    wallet: Option<&str>,
    mut values: Vec<libsql::Value>,
) -> ApiResult<i64> {
    let connection = state.db.connect().map_err(auth::internal)?;
    let sql = match wallet {
        Some(wallet) => {
            values.push(libsql::Value::Text(wallet.to_owned()));
            format!(
                "SELECT COUNT(*) FROM sponsored_settlements WHERE {filter} AND wallet = ?{}",
                values.len()
            )
        }
        None => format!("SELECT COUNT(*) FROM sponsored_settlements WHERE {filter}"),
    };
    let mut rows = connection
        .query(&sql, values)
        .await
        .map_err(auth::internal)?;
    let count: i64 = match rows.next().await.map_err(auth::internal)? {
        Some(row) => row.get(0).map_err(auth::internal)?,
        None => 0,
    };
    Ok(count)
}

#[cfg(test)]
mod tests;
