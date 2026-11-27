//! The account's paid period, and the gate shop mutations go through.

use crate::auth;
use crate::payments::ApiResult;

#[cfg(test)]
use super::pricing::PERIOD_SECS;

pub(super) async fn latest(
    state: &crate::AppState,
    user_id: &str,
) -> ApiResult<(bool, i64, Option<String>, Option<u64>)> {
    let connection = state.db.connect().map_err(auth::internal)?;
    let mut rows = connection
        .query(
            "SELECT currency, amount, expires_at FROM subscriptions \
             WHERE user_id = ?1 ORDER BY expires_at DESC LIMIT 1",
            [user_id.to_string()],
        )
        .await
        .map_err(auth::internal)?;
    match rows.next().await.map_err(auth::internal)? {
        Some(row) => {
            let currency: String = row.get(0).map_err(auth::internal)?;
            let amount: i64 = row.get(1).map_err(auth::internal)?;
            let expires_at: i64 = row.get(2).map_err(auth::internal)?;
            Ok((
                expires_at > auth::unix_now(),
                expires_at,
                Some(currency),
                u64::try_from(amount).ok(),
            ))
        }
        None => Ok((false, 0, None, None)),
    }
}

/// Mutations a lapsed subscription refuses.
pub async fn require_active(state: &crate::AppState, user_id: &str) -> ApiResult<()> {
    let (active, _, _, _) = latest(state, user_id).await?;
    if active {
        return Ok(());
    }
    Err(crate::payments::ApiError::Forbidden(
        "The shops feature needs an active subscription (€1.70 for 30 days).".into(),
    ))
}

/// Grants one paid period, for the suites that need a subscribed seller without
/// a cluster to settle against.
#[cfg(test)]
pub(crate) async fn activate_for_tests(db: &libsql::Database, user_id: &str) {
    let connection = db.connect().expect("connection");
    connection
        .execute(
            "INSERT INTO subscriptions \
             (id, user_id, currency, amount, payment_id, paid_at, expires_at) \
             VALUES (?1, ?2, 'eurc', 1700000, ?3, ?4, ?5)",
            (
                auth::random_id(),
                user_id.to_string(),
                auth::random_id(),
                auth::unix_now(),
                auth::unix_now() + PERIOD_SECS,
            ),
        )
        .await
        .expect("grant subscription");
}
