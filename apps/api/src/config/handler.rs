//! The `GET /api/config` route.

use axum::extract::Query;
use axum::http::header;
use axum::response::IntoResponse;
use axum::Json;

use super::cache::{cached_config, remember_config};
use super::model::ConfigQuery;
use super::read::read_config;
use crate::payments::{self, ApiResult};

/// The fee policy, the mints and the rent minimums, from one read a minute.
///
/// Public, because a checkout quotes before anyone signs in. The response is
/// cacheable for the same minute it is kept: a CDN in front of the API then
/// answers every tab of every buyer for free.
pub async fn config(Query(query): Query<ConfigQuery>) -> ApiResult<impl IntoResponse> {
    let cluster = query.cluster.unwrap_or_else(payments::default_cluster);
    let payload = match cached_config(&cluster) {
        Some(cached) => cached,
        None => {
            let endpoint = payments::resolve_endpoint(None, &cluster)?;
            let read = read_config(&endpoint, &cluster).await?;
            remember_config(&cluster, &read);
            read
        }
    };
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=60")],
        Json(payload),
    ))
}
