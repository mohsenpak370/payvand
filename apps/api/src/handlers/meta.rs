//! Service metadata and the embedded API reference.

use axum::http::header;
use axum::response::IntoResponse;
use axum::Json;

pub(crate) async fn index() -> Json<serde_json::Value> {
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
            "/api/auth/wallet/challenge",
            "/api/auth/wallet/link",
            "/api/auth/wallet/signin",
            "/api/auth/wallet/unlink",
            "/api/me/transactions",
            "/api/payments",
            "/api/payments/watch",
            "/api/payments/{id}",
            "/api/solana-pay/{token}",
            "/api/saved-payments",
            "/api/sponsor",
            "/api/sponsor/settle",
            "/api/faucet",
            "/api/discounts",
            "/api/discounts/validate",
            "/api/discounts/use",
            "/api/discounts/{id}",
            "/api/waitlist",
            "/api/shops",
            "/api/shops/{id}",
            "/api/shops/{id}/products",
            "/api/shops/{id}/products/{product_id}",
            "/api/admin/stats",
            "/api/admin/users",
            "/api/admin/users/{id}",
            "/api/admin/payments",
            "/api/admin/waitlist",
            "/api/admin/waitlist/{email}",
            "/api/admin/shops",
            "/api/admin/shops/{id}",
            "/api/admin/products",
            "/api/admin/products/{id}",
            "/api/admin/discounts",
            "/api/admin/discounts/{id}",
            "/api/openapi.json",
            "/api/docs",
            "/api/push/key",
            "/api/push/subscribe",
            "/api/push/unsubscribe",
            "/api/push/test",
            "/api/cron/allowance-nudge"
        ]
    }))
}

pub(crate) async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": true }))
}

/// `GET /api/openapi.json` — the description Swagger UI renders. Written by
/// hand and embedded at compile time: the document is the contract, and it
/// ships with the binary rather than being read from a disk a serverless
/// function does not have.
pub(crate) async fn openapi() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        include_str!("../../openapi.json"),
    )
}

/// `GET /api/docs` — the interactive reference itself.
pub(crate) async fn docs() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        include_str!("../../swagger.html"),
    )
}
