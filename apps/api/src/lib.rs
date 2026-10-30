//! Axum router shared by the local server and the Vercel function.

pub mod admin;
pub mod auth;
pub mod db;
pub mod discounts;
pub mod email;
pub mod payments;

use axum::extract::{Path, Query, State};
use axum::http::{header, Method};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use payments::{ApiError, CreatePaymentRequest, PaymentQuery};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

/// Everything a handler needs. The database handle is shared; libSQL opens
/// connections from it cheaply.
#[derive(Clone)]
pub struct AppState {
    pub db: db::SharedDb,
}

/// Opens the database, runs the idempotent migrations and builds the router.
pub async fn app() -> Router {
    let database = db::open_and_migrate()
        .await
        .expect("DATABASE_URL could not be opened or migrated");
    build_router(AppState {
        db: Arc::new(database),
    })
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/health", get(health))
        .route("/api/health", get(health))
        .route("/api/auth/signup", post(auth::signup))
        .route("/api/auth/signin", post(auth::signin))
        .route("/api/auth/forgot-password", post(auth::forgot_password))
        .route("/api/auth/reset-password", post(auth::reset_password))
        .route("/api/auth/signout", post(auth::signout))
        .route("/api/auth/me", get(auth::me))
        .route("/api/payments", post(create_payment))
        .route("/api/payments/watch", post(watch_payments))
        .route("/api/payments/{id}", get(get_payment))
        .route("/api/admin/stats", get(admin::stats))
        .route("/api/admin/users", get(admin::list_users))
        .route("/api/admin/users/{id}", delete(admin::delete_user))
        .route("/api/admin/payments", get(admin::list_payments))
        .route("/api/discounts", get(discounts::list).post(discounts::create))
        .route("/api/discounts/validate", post(discounts::validate))
        .route("/api/discounts/{id}", delete(discounts::delete))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([Method::GET, Method::POST, Method::DELETE])
                .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn index() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "service": "payvand-api",
        "routes": [
            "/health",
            "/api/health",
            "/api/auth/signup",
            "/api/auth/signin",
            "/api/auth/forgot-password",
            "/api/auth/reset-password",
            "/api/auth/signout",
            "/api/auth/me",
            "/api/payments",
            "/api/payments/watch",
            "/api/payments/{id}",
            "/api/discounts",
            "/api/discounts/validate",
            "/api/discounts/{id}",
            "/api/admin/stats",
            "/api/admin/users",
            "/api/admin/users/{id}",
            "/api/admin/payments"
        ]
    }))
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": true }))
}

/// Validates a payment request the seller's wallet just wrote on chain and
/// returns the canonical record together with the QR payload to render.
/// Only reachable by a signed-in account.
async fn create_payment(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(request): Json<CreatePaymentRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user = auth::require_user(&state.db, &headers).await?;
    let mut payment = payments::create_payment(request)?;
    // The buyer's scan-to-pay screen asks the API for this seller's codes.
    payment.seller_user = Some(user.id);
    let qr_payload = payment.qr_payload();
    Ok(Json(
        serde_json::json!({ "payment": payment, "qrPayload": qr_payload }),
    ))
}

/// Live status of a payment, read straight from the cluster.
async fn get_payment(
    Path(id): Path<String>,
    Query(query): Query<PaymentQuery>,
) -> Result<Json<payments::PaymentRecord>, ApiError> {
    Ok(Json(payments::fetch_payment(&id, query).await?))
}

/// Batch status read — the seller's screen watches the invoice address and
/// every discounted variant in a single round trip.
async fn watch_payments(
    Json(request): Json<payments::WatchRequest>,
) -> Result<Json<Vec<payments::PaymentRecord>>, ApiError> {
    Ok(Json(payments::fetch_many(request).await?))
}
