//! Payment endpoints that compose the payments and discounts modules.

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;

use crate::payments::{ApiError, CreatePaymentRequest, PaymentQuery};
use crate::AppState;

/// Validates a payment request the seller's wallet just wrote on chain and
/// returns the canonical record together with the QR payload to render.
/// Only reachable by a signed-in account.
pub(crate) async fn create_payment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreatePaymentRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user = crate::auth::require_user(&state.db, &headers).await?;
    let discount_code = request.discount_code.clone();
    let mut payment = crate::payments::create_payment(request)?;
    // The invoice is recorded here, with its name, its facts and the account
    // that made it: the feed lists these rows and asks the cluster only whether
    // each one settled.
    crate::payments::remember_invoice(&state.db, &payment, Some(&user.id)).await;

    // The buyer's scan-to-pay screen asks the API for this seller's codes.
    payment.seller_user = Some(user.id.clone());
    // A code attached to the invoice is priced here, before the payload is
    // built, so the buyer's screen can apply it without anyone typing it.
    if let Some(code) = discount_code {
        payment.discount =
            Some(crate::discounts::resolve_for_seller(&state.db, &user.id, &code).await?);
    }
    let qr_payload = payment.qr_payload();
    Ok(Json(
        serde_json::json!({ "payment": payment, "qrPayload": qr_payload }),
    ))
}

/// Live status of a payment, read straight from the cluster.
pub(crate) async fn get_payment(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<PaymentQuery>,
) -> Result<Json<crate::payments::PaymentRecord>, ApiError> {
    Ok(Json(
        crate::payments::fetch_payment(&id, query, &state.db).await?,
    ))
}

/// Batch status read — the seller's screen watches the invoice address and
/// every discounted variant in a single round trip.
pub(crate) async fn watch_payments(
    State(state): State<AppState>,
    Json(request): Json<crate::payments::WatchRequest>,
) -> Result<Json<Vec<crate::payments::PaymentRecord>>, ApiError> {
    Ok(Json(crate::payments::fetch_many(request, &state.db).await?))
}
