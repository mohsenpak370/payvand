//! The `/api/reusable` routes.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;

use crate::payments::ApiError;
use crate::AppState;

use super::links::{create_link, mint_invoice};
use super::model::{CreateReusableRequest, ReusableLink};

/// `POST /api/reusable` — the seller's own code for a repeatable price.
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateReusableRequest>,
) -> Result<Json<ReusableLink>, ApiError> {
    let user = crate::auth::require_user(&state.db, &headers).await?;
    Ok(Json(create_link(&state.db, &user, request).await?))
}

/// `POST /api/reusable/{id}/invoice` — a fresh invoice for this visit.
pub(crate) async fn invoice(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let (payment, qr_payload) = mint_invoice(&state.db, &id).await?;
    Ok(Json(
        serde_json::json!({ "payment": payment, "qrPayload": qr_payload }),
    ))
}
