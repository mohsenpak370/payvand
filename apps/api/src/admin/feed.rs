//! The payment feed: cluster activity shaped for the admin page.

use axum::extract::{Json, Query, State};
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};

use crate::payments::{ApiResult, PaymentRecord, PaymentStatus};

use super::require_admin;

#[derive(Debug, Deserialize)]
pub struct PaymentListQuery {
    pub limit: Option<i64>,
}

/// One row of the admin feed: the invoice exactly as the chain knows it.
#[derive(Debug, Serialize)]
pub struct AdminPayment {
    pub id: String,
    pub seller: String,
    pub amount_lamports: u64,
    pub memo: String,
    pub status: PaymentStatus,
    pub settled_by: Option<String>,
    pub created_at_unix: i64,
    pub settled_at_unix: i64,
}

#[derive(Debug, Serialize)]
pub struct PaymentsPage {
    pub items: Vec<AdminPayment>,
    pub total: usize,
    pub cluster: String,
    pub endpoint: String,
}

impl From<PaymentRecord> for AdminPayment {
    fn from(record: PaymentRecord) -> Self {
        Self {
            id: record.id,
            seller: record.seller,
            amount_lamports: record.amount_lamports,
            memo: record.memo,
            status: record.status,
            settled_by: record.settled_by,
            created_at_unix: record.created_at_unix,
            settled_at_unix: record.settled_at_unix,
        }
    }
}

/// The pure part of the feed — newest first, capped, deterministic even
/// when several invoices share a second — so it is testable without a cluster.
pub(super) fn shape_feed(
    mut records: Vec<PaymentRecord>,
    limit: usize,
) -> (Vec<AdminPayment>, usize) {
    let total = records.len();
    records.sort_by(|a, b| {
        b.created_at_unix
            .cmp(&a.created_at_unix)
            .then_with(|| b.settled_at_unix.cmp(&a.settled_at_unix))
            .then_with(|| a.id.cmp(&b.id))
    });
    records.truncate(limit);
    (records.into_iter().map(AdminPayment::from).collect(), total)
}

/// `GET /api/admin/payments` — live transaction activity, read straight
/// from the cluster. The admin check comes before any RPC work, so an
/// outsider never makes the chain do anything on their behalf.
pub async fn list_payments(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Query(query): Query<PaymentListQuery>,
) -> ApiResult<Json<PaymentsPage>> {
    require_admin(&state.db, &headers).await?;
    let limit = query.limit.unwrap_or(20).clamp(1, 100);

    // The page is what the admin sees, so only the markers it can show are
    // worth a history read; the rest answer from themselves and fill in on a
    // later poll (or from the cache, which makes repeats free).
    let all = crate::payments::fetch_all_payments(limit as usize, &state.db).await?;
    let (items, total) = shape_feed(all.records, limit as usize);
    Ok(Json(PaymentsPage {
        items,
        total,
        cluster: all.cluster,
        endpoint: all.endpoint,
    }))
}
