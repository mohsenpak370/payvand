//! Axum router shared by the local server and the Vercel function.

pub mod admin;
pub mod auth;
pub mod config;
pub mod db;
pub mod discounts;
pub mod email;
pub mod faucet;
pub mod payments;
pub mod push;
mod push_budget;
pub mod rate_limit;
pub mod saved;
pub mod shops;
pub mod solana_pay;
pub mod sponsor;
pub mod subscription;
pub mod waitlist;
pub mod wallet;

use axum::extract::{Path, Query, Request, State};
use axum::http::{header, Method};
use axum::response::IntoResponse;
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use payments::{ApiError, CreatePaymentRequest, PaymentQuery};
use solana_sdk::signer::keypair::Keypair;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

/// Everything a handler needs. The database handle is shared; libSQL opens
/// connections from it cheaply.
#[derive(Clone)]
pub struct AppState {
    pub db: db::SharedDb,
}

/// A server-held keypair from the environment: the solana CLI's JSON array of
/// 64 bytes, or the same bytes base58 encoded. Absent or unreadable is `None`,
/// never a panic — a misconfigured variable disables a feature instead of
/// taking the API down.
pub(crate) fn keypair_from_env(name: &str) -> Option<Keypair> {
    let raw = std::env::var(name).ok()?;
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if raw.starts_with('[') {
        let bytes: Vec<u8> = serde_json::from_str(raw).ok()?;
        return Keypair::try_from(bytes.as_slice()).ok();
    }
    // Decoded here rather than with `from_base58_string`, which panics on a
    // malformed secret.
    let bytes = bs58::decode(raw).into_vec().ok()?;
    Keypair::try_from(bytes.as_slice()).ok()
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
        .route("/api/auth/wallet/challenge", post(wallet::challenge))
        .route("/api/auth/wallet/link", post(wallet::link))
        .route("/api/auth/wallet/signin", post(wallet::signin))
        .route("/api/auth/wallet/unlink", post(wallet::unlink))
        .route("/api/me/transactions", get(wallet::transactions))
        // The numbers a quote needs — the fee policy, the mint addresses and the
        // cluster's rent minimums — read from the cluster once and cached for a
        // minute, so a browser tab does not read devnet for its own pricing.
        .route("/api/config", get(config::config))
        .route("/api/payments", post(create_payment))
        .route("/api/payments/watch", post(watch_payments))
        .route("/api/payments/{id}", get(get_payment))
        // Solana Pay transaction request: a wallet scans the QR, asks for a
        // transaction and signs it. GET is the metadata half of the spec.
        .route("/api/solana-pay/icon.svg", get(solana_pay::icon))
        .route(
            "/api/solana-pay/{token}",
            get(solana_pay::info).post(solana_pay::transaction),
        )
        .route("/api/saved-payments", get(saved::list).post(saved::save))
        // The relayer that pays a token invoice's network fee and record rents
        // for a buyer who holds only the token. `info` is what the checkout
        // asks before it builds anything; `settle` signs only a message it has
        // rebuilt byte for byte.
        .route("/api/sponsor", get(sponsor::info))
        .route("/api/sponsor/settle", post(sponsor::settle))
        // Devnet starter tokens for a newly linked wallet: the balance check
        // and the one claim per account, from the deployment's own treasury.
        .route("/api/faucet", get(faucet::status).post(faucet::claim))
        .route("/api/admin/stats", get(admin::stats))
        .route("/api/admin/users", get(admin::list_users))
        .route("/api/admin/users/{id}", delete(admin::delete_user))
        .route("/api/admin/payments", get(admin::list_payments))
        .route("/api/admin/waitlist", get(admin::waitlist))
        .route(
            "/api/admin/waitlist/{email}",
            delete(admin::remove_waitlist),
        )
        .route(
            "/api/discounts",
            get(discounts::list).post(discounts::create),
        )
        .route("/api/discounts/validate", post(discounts::validate))
        .route("/api/discounts/use", post(discounts::use_code))
        .route("/api/discounts/{id}", delete(discounts::delete))
        // Sellers who want to take real payments, before they have an account.
        .route("/api/waitlist", post(waitlist::join))
        // Push notifications: the VAPID public key the browser subscribes
        // with, this device's subscription, a test send, and the daily cron
        // that reminds people their allowance reset unrefilled.
        .route("/api/push/key", get(push::key))
        .route("/api/push/subscribe", post(push::subscribe))
        .route("/api/push/unsubscribe", post(push::unsubscribe))
        .route("/api/push/test", post(push::test))
        .route("/api/cron/allowance-nudge", get(push::nudge))
        // The seller's shops and their catalogues. A shop id is part of every
        // product path, and ownership is checked on each one.
        .route("/api/shops", get(shops::list).post(shops::create))
        // The shops subscription: the plan's three prices and the account's
        // period, the invoice that pays for one, and the confirmation that
        // records it once the cluster says the invoice settled.
        .route("/api/subscription", get(subscription::status))
        .route(
            "/api/subscription/invoice",
            post(subscription::create_invoice),
        )
        .route("/api/subscription/confirm", post(subscription::confirm))
        .route("/api/shops/{id}", get(shops::get).patch(shops::update))
        .route("/api/shops/{id}/products", post(shops::create_product))
        .route(
            "/api/shops/{id}/categories",
            get(shops::categories).post(shops::add_category),
        )
        .route(
            "/api/shops/{id}/categories/{name}",
            delete(shops::delete_category),
        )
        .route(
            "/api/shops/{id}/products/{product_id}",
            patch(shops::update_product).delete(shops::delete_product),
        )
        // Inside the CORS layer on purpose: a 429 still has to carry the
        // headers that let the browser read it.
        .layer(axum::middleware::from_fn(throttle))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                // PATCH is how a shop and a product are edited, so it has to be
                // here or the browser refuses the request before it is sent.
                .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
                .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

struct Throttle {
    bucket: &'static str,
    limit: u32,
    window_secs: u64,
}

/// The routes a stranger can hammer without an account: passwords, discount
/// codes and the waiting list. Everything else already needs a session, so its
/// volume is bounded by the account that spends it.
fn throttle_for(path: &str) -> Option<Throttle> {
    let (bucket, limit, window_secs) = match path {
        "/api/auth/signup" => ("signup", 10, rate_limit::HOUR),
        "/api/auth/signin" => ("signin", 30, rate_limit::MINUTE),
        "/api/auth/forgot-password" => ("forgot", 10, rate_limit::HOUR),
        "/api/auth/wallet/challenge" => ("challenge", 30, rate_limit::MINUTE),
        // Low on purpose: a person applies a code or two, while guessing one
        // is the only way to find another seller's discount.
        "/api/discounts/validate" => ("discount", 12, rate_limit::MINUTE),
        // Recording a use follows a payment, so it is bounded by real invoices
        // rather than by guessing, and it is cheap to answer either way.
        "/api/discounts/use" => ("discount-use", 30, rate_limit::MINUTE),
        "/api/waitlist" => ("waitlist", 10, rate_limit::HOUR),
        // The relayer takes a settle from anyone whose signature matches, so the
        // only protection in front of its SOL is a caller's pace. The handler
        // bounds each wallet; this bounds each address, whatever wallet it names.
        "/api/sponsor/settle" => ("sponsor-settle", 30, rate_limit::MINUTE),
        // One claim per account is the handler's rule; this is the floor under
        // a script that retries, and it keeps the treasury from being probed.
        "/api/faucet" => ("faucet", 6, rate_limit::HOUR),
        // A subscription is a row per device, and a test send spends a push
        // service's quota: both are cheap, but neither should be a loop.
        "/api/push/subscribe" | "/api/push/unsubscribe" => ("push-device", 30, rate_limit::HOUR),
        "/api/push/test" => ("push-test", 12, rate_limit::HOUR),
        _ => return None,
    };
    Some(Throttle {
        bucket,
        limit,
        window_secs,
    })
}

/// Counts a hit per caller before the request reaches a handler. Only POST is
/// counted: a page read costs nothing, a guess at a password does.
async fn throttle(request: Request, next: axum::middleware::Next) -> axum::response::Response {
    if request.method() == Method::POST {
        if let Some(policy) = throttle_for(request.uri().path()) {
            let caller = rate_limit::caller(request.headers());
            if let Err(error) =
                rate_limit::hit(policy.bucket, &caller, policy.limit, policy.window_secs)
            {
                return error.into_response();
            }
        }
    }
    next.run(request).await
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
            "/api/push/key",
            "/api/push/subscribe",
            "/api/push/unsubscribe",
            "/api/push/test",
            "/api/cron/allowance-nudge"
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
    let discount_code = request.discount_code.clone();
    let mut payment = payments::create_payment(request)?;
    // The invoice is recorded here, with its name, its facts and the account
    // that made it: the feed lists these rows and asks the cluster only whether
    // each one settled.
    payments::remember_invoice(&state.db, &payment, Some(&user.id)).await;

    // The buyer's scan-to-pay screen asks the API for this seller's codes.
    payment.seller_user = Some(user.id.clone());
    // A code attached to the invoice is priced here, before the payload is
    // built, so the buyer's screen can apply it without anyone typing it.
    if let Some(code) = discount_code {
        payment.discount = Some(discounts::resolve_for_seller(&state.db, &user.id, &code).await?);
    }
    let qr_payload = payment.qr_payload();
    Ok(Json(
        serde_json::json!({ "payment": payment, "qrPayload": qr_payload }),
    ))
}

/// Live status of a payment, read straight from the cluster.
async fn get_payment(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<PaymentQuery>,
) -> Result<Json<payments::PaymentRecord>, ApiError> {
    Ok(Json(payments::fetch_payment(&id, query, &state.db).await?))
}

/// Batch status read — the seller's screen watches the invoice address and
/// every discounted variant in a single round trip.
async fn watch_payments(
    State(state): State<AppState>,
    Json(request): Json<payments::WatchRequest>,
) -> Result<Json<Vec<payments::PaymentRecord>>, ApiError> {
    Ok(Json(payments::fetch_many(request, &state.db).await?))
}
