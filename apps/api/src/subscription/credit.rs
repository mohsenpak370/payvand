//! Crediting a settled invoice with the period it bought — when the browser
//! confirms it, or when a status read finds one nobody confirmed.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use crate::auth;
use crate::payments::{self, bad, ApiError, ApiResult, PaymentQuery, PaymentStatus};

use super::period::latest;
use super::platform::{cluster, platform_seller};
use super::pricing::PERIOD_SECS;

/// How far back a status read looks for a paid-but-unconfirmed invoice. Three
/// days covers a buyer who paid on Friday and came back on Monday; older than
/// that the invoice is abandoned rather than pending, and is not re-read
/// forever.
const REPAIR_WINDOW_SECS: i64 = 3 * 24 * 60 * 60;
/// How many such invoices one read may check — the newest is the one that paid.
const REPAIR_INVOICES: usize = 3;
/// Repairs for one account are spaced out: opening a page is not a chain poll.
const REPAIR_INTERVAL_SECS: u64 = 60;

/// When each account was last checked for an unconfirmed payment.
fn repair_cache() -> &'static Mutex<HashMap<String, Instant>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Credits the invoices this account was quoted recently and never confirmed.
///
/// A payment can settle with nobody left to say so: the tab closed while the
/// wallet was approving, the confirm was refused, or the browser forgot the
/// invoice id. The money is on chain either way, so the month it bought is not
/// something to lose — the next ask about the subscription is where it is
/// noticed, and therefore where it is repaired.
///
pub(super) async fn credit_unconfirmed(state: &crate::AppState, user_id: &str) -> usize {
    {
        let mut seen = repair_cache().lock().await;
        let now = Instant::now();
        let interval = Duration::from_secs(REPAIR_INTERVAL_SECS);
        if seen
            .get(user_id)
            .is_some_and(|at| now.duration_since(*at) < interval)
        {
            return 0;
        }
        // The map is one string per account that has opened the shops page; an
        // hour of it is enough, and keeps a long-lived instance bounded.
        seen.retain(|_, at| now.duration_since(*at) < interval * 60);
        seen.insert(user_id.to_string(), now);
    }

    let Some(ids) = unconfirmed_invoices(state, user_id).await else {
        return 0;
    };
    let mut written = 0;
    for id in ids {
        match credit_invoice(state, user_id, &id).await {
            Ok(true) => written += 1,
            Ok(false) => {}
            // An invoice nobody has paid yet answers "not settled": that is the
            // ordinary case for a link that was opened and left, not a fault.
            Err(ApiError::BadRequest(_)) => {}
            Err(error) => {
                tracing::info!(%user_id, %id, %error, "subscription repair skipped")
            }
        }
    }
    written
}

/// The account's recent invoices that no period was recorded for.
async fn unconfirmed_invoices(state: &crate::AppState, user_id: &str) -> Option<Vec<String>> {
    let connection = state.db.connect().ok()?;
    let mut rows = connection
        .query(
            "SELECT i.payment_id FROM subscription_invoices i \
             WHERE i.user_id = ?1 AND i.created_at >= ?2 \
             AND NOT EXISTS (SELECT 1 FROM subscriptions s \
                             WHERE s.user_id = i.user_id AND s.payment_id = i.payment_id) \
             ORDER BY i.created_at DESC LIMIT ?3",
            (
                user_id.to_string(),
                auth::unix_now() - REPAIR_WINDOW_SECS,
                REPAIR_INVOICES as i64,
            ),
        )
        .await
        .ok()?;
    let mut ids = Vec::new();
    while let Some(row) = rows.next().await.ok()? {
        ids.push(row.get::<String>(0).ok()?);
    }
    Some(ids)
}

/// Verifies one invoice against the cluster and records the period it buys.
///
/// Both the browser saying "I paid" and a status read repairing a payment
/// nobody confirmed go through this, so the checks live in one place: the
/// invoice must have been quoted to this account, it must have settled, it must
/// have paid the platform, and it must have paid the amount it was quoted at.
///
/// Answers whether a period was written. An invoice that was already credited
/// writes nothing — that is what a reload after paying looks like, and it must
/// not buy a second month.
pub(super) async fn credit_invoice(
    state: &crate::AppState,
    user_id: &str,
    payment_id: &str,
) -> ApiResult<bool> {
    let connection = state.db.connect().map_err(auth::internal)?;

    let mut rows = connection
        .query(
            "SELECT currency, amount FROM subscription_invoices \
             WHERE payment_id = ?1 AND user_id = ?2",
            (payment_id.to_string(), user_id.to_string()),
        )
        .await
        .map_err(auth::internal)?;
    let (currency, amount): (String, i64) = match rows.next().await.map_err(auth::internal)? {
        Some(row) => (
            row.get(0).map_err(auth::internal)?,
            row.get(1).map_err(auth::internal)?,
        ),
        None => return Err(bad("That subscription invoice was not created here.")),
    };
    drop(rows);

    let mut paid = connection
        .query(
            "SELECT 1 FROM subscriptions WHERE user_id = ?1 AND payment_id = ?2",
            (user_id.to_string(), payment_id.to_string()),
        )
        .await
        .map_err(auth::internal)?;
    if paid.next().await.map_err(auth::internal)?.is_some() {
        return Ok(false);
    }
    drop(paid);

    let record = payments::fetch_payment(
        payment_id,
        PaymentQuery {
            cluster: Some(cluster()),
            endpoint: None,
        },
        &state.db,
    )
    .await?;

    if record.status != PaymentStatus::Settled {
        return Err(bad("That payment has not settled yet."));
    }
    // The invoice is ours only if it paid the platform the amount we quoted.
    let seller = platform_seller()?;
    if record.seller != seller.to_string() {
        return Err(bad("That payment did not go to the platform account."));
    }
    let paid_option = record
        .options
        .iter()
        .find(|option| option.currency == currency)
        .ok_or_else(|| bad("That payment is not in the invoice's currency."))?;
    if paid_option.amount != u64::try_from(amount).unwrap_or(u64::MAX) {
        return Err(bad("That payment is not the invoiced amount."));
    }

    let now = auth::unix_now();
    let (_, current_expiry, _, _) = latest(state, user_id).await?;
    // A renewal after a lapsed period starts now; one bought early adds to the
    // period already paid for, so nobody loses days by paying on time.
    let start = current_expiry.max(now);
    connection
        .execute(
            // `OR IGNORE` because two requests can reach this per invoice (the
            // browser's confirm and a status read's repair): the period is one,
            // and the row's uniqueness is what says so.
            "INSERT OR IGNORE INTO subscriptions \
             (id, user_id, currency, amount, payment_id, paid_at, expires_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                auth::random_id(),
                user_id.to_string(),
                currency,
                amount,
                payment_id.to_string(),
                now,
                start + PERIOD_SECS,
            ),
        )
        .await
        .map_err(auth::internal)?;

    Ok(true)
}
