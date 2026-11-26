//! The starter-token claims the faucet has sent, newest first.
//!
//! A claim row is written before the transfer is sent and stamped with the
//! signature once the cluster accepts it, so a row with no signature is a claim
//! in flight (or one whose release failed) — shown as pending, not hidden.

use axum::extract::{Json, Query, State};
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};

use crate::auth;
use crate::payments::ApiResult;

use super::{require_admin, scalar};

#[derive(Debug, Deserialize)]
pub struct AirdropQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub(super) const DEFAULT_AIRDROP_PAGE: i64 = 20;
pub(super) const MAX_AIRDROP_PAGE: i64 = 100;

#[derive(Debug, Serialize)]
pub struct Airdrop {
    pub user_id: String,
    pub email: String,
    pub wallet: String,
    pub cluster: String,
    pub sol_lamports: i64,
    pub token_base_units: i64,
    /// Empty while the transfer is still being sent.
    pub signature: String,
    pub claimed_at: i64,
}

#[derive(Debug, Serialize)]
pub struct AirdropPage {
    pub items: Vec<Airdrop>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
    /// Everything the faucet has sent, over every page: lamports, and base
    /// units of each stablecoin (every claim sends the same amount of both).
    pub total_sol_lamports: i64,
    pub total_token_base_units: i64,
}

/// `GET /api/admin/airdrops` — the faucet's claims, with their totals.
pub async fn airdrops(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Query(query): Query<AirdropQuery>,
) -> ApiResult<Json<AirdropPage>> {
    require_admin(&state.db, &headers).await?;
    let limit = query
        .limit
        .unwrap_or(DEFAULT_AIRDROP_PAGE)
        .clamp(1, MAX_AIRDROP_PAGE);
    let offset = query.offset.unwrap_or(0).max(0);
    let connection = state.db.connect().map_err(auth::internal)?;

    let total = scalar(&connection, "SELECT COUNT(*) FROM faucet_claims").await?;
    // Only claims that reached the cluster count toward what was sent.
    let total_sol_lamports = scalar(
        &connection,
        "SELECT COALESCE(SUM(sol_lamports), 0) FROM faucet_claims WHERE signature != ''",
    )
    .await?;
    let total_token_base_units = scalar(
        &connection,
        "SELECT COALESCE(SUM(token_base_units), 0) FROM faucet_claims WHERE signature != ''",
    )
    .await?;

    let mut rows = connection
        .query(
            "SELECT f.user_id, COALESCE(u.email, ''), f.wallet, f.cluster, f.sol_lamports, \
             f.token_base_units, f.signature, f.claimed_at \
             FROM faucet_claims f LEFT JOIN users u ON u.id = f.user_id \
             ORDER BY f.claimed_at DESC, f.user_id ASC LIMIT ?1 OFFSET ?2",
            (limit, offset),
        )
        .await
        .map_err(auth::internal)?;

    let mut items = Vec::new();
    while let Some(row) = rows.next().await.map_err(auth::internal)? {
        items.push(Airdrop {
            user_id: row.get(0).map_err(auth::internal)?,
            email: row.get(1).map_err(auth::internal)?,
            wallet: row.get(2).map_err(auth::internal)?,
            cluster: row.get(3).map_err(auth::internal)?,
            sol_lamports: row.get(4).map_err(auth::internal)?,
            token_base_units: row.get(5).map_err(auth::internal)?,
            signature: row.get(6).map_err(auth::internal)?,
            claimed_at: row.get(7).map_err(auth::internal)?,
        });
    }

    Ok(Json(AirdropPage {
        items,
        total,
        limit,
        offset,
        total_sol_lamports,
        total_token_base_units,
    }))
}
