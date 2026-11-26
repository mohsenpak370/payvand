//! Route table: grouped by area, with the shared middleware applied once.

use axum::http::{header, Method};
use axum::routing::{delete, get, patch, post};
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::AppState;
use crate::{
    admin, auth, config, discounts, faucet, handlers, push, saved, shops, solana_pay, sponsor,
    subscription, waitlist, wallet,
};

pub fn build_router(state: AppState) -> Router {
    Router::<AppState>::new()
        .merge(meta_routes())
        .merge(auth_routes())
        .merge(quote_routes())
        .merge(integration_routes())
        .merge(admin_routes())
        .merge(discount_routes())
        .merge(waitlist_routes())
        .merge(push_routes())
        .merge(shop_routes())
        // Inside the CORS layer on purpose: a 429 still has to carry the
        // headers that let the browser read it.
        .layer(axum::middleware::from_fn(crate::throttle::throttle))
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

fn meta_routes() -> Router<AppState> {
    Router::<AppState>::new()
        .route("/", get(handlers::index))
        .route("/health", get(handlers::health))
        .route("/api/health", get(handlers::health))
        // The interactive API reference and the document it renders. Static,
        // embedded at compile time, so they cost a handler and no storage.
        .route("/api/openapi.json", get(handlers::openapi))
        .route("/api/docs", get(handlers::docs))
}

fn auth_routes() -> Router<AppState> {
    Router::<AppState>::new()
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
}

fn quote_routes() -> Router<AppState> {
    Router::<AppState>::new()
        // The numbers a quote needs — the fee policy, the mint addresses and the
        // cluster's rent minimums — read from the cluster once and cached for a
        // minute, so a browser tab does not read devnet for its own pricing.
        .route("/api/config", get(config::config))
        .route("/api/payments", post(handlers::create_payment))
        .route("/api/payments/watch", post(handlers::watch_payments))
        .route("/api/payments/{id}", get(handlers::get_payment))
}

fn integration_routes() -> Router<AppState> {
    Router::<AppState>::new()
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
}

fn admin_routes() -> Router<AppState> {
    Router::<AppState>::new()
        .route("/api/admin/stats", get(admin::stats))
        .route("/api/admin/users", get(admin::list_users))
        .route("/api/admin/users/{id}", delete(admin::delete_user))
        .route("/api/admin/payments", get(admin::list_payments))
        .route("/api/admin/waitlist", get(admin::waitlist))
        .route(
            "/api/admin/waitlist/{email}",
            delete(admin::remove_waitlist),
        )
        // Records: the seller-owned rows, read and corrected the way a
        // database console would.
        .route("/api/admin/shops", get(admin::list_shops))
        .route(
            "/api/admin/shops/{id}",
            patch(admin::update_shop).delete(admin::delete_shop),
        )
        .route("/api/admin/products", get(admin::list_products))
        .route(
            "/api/admin/products/{id}",
            patch(admin::update_product).delete(admin::delete_product),
        )
        .route("/api/admin/discounts", get(admin::list_codes))
        .route("/api/admin/discounts/{id}", delete(admin::delete_code))
}

fn discount_routes() -> Router<AppState> {
    Router::<AppState>::new()
        .route(
            "/api/discounts",
            get(discounts::list).post(discounts::create),
        )
        .route("/api/discounts/validate", post(discounts::validate))
        .route("/api/discounts/use", post(discounts::use_code))
        .route("/api/discounts/{id}", delete(discounts::delete))
}

fn waitlist_routes() -> Router<AppState> {
    Router::<AppState>::new()
        // Sellers who want to take real payments, before they have an account.
        .route("/api/waitlist", post(waitlist::join))
}

fn push_routes() -> Router<AppState> {
    Router::<AppState>::new()
        // Push notifications: the VAPID public key the browser subscribes
        // with, this device's subscription, a test send, and the daily cron
        // that reminds people their allowance reset unrefilled.
        .route("/api/push/key", get(push::key))
        .route("/api/push/subscribe", post(push::subscribe))
        .route("/api/push/unsubscribe", post(push::unsubscribe))
        .route("/api/push/test", post(push::test))
        .route("/api/cron/allowance-nudge", get(push::nudge))
}

fn shop_routes() -> Router<AppState> {
    Router::<AppState>::new()
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    /// The grouped routers still merge into one working app: a duplicate route
    /// would panic here rather than in production.
    #[tokio::test]
    async fn merged_routes_serve_public_endpoints() {
        let path = std::env::temp_dir().join(format!(
            "payvand-router-test-{}.db",
            crate::auth::random_id()
        ));
        let db = libsql::Builder::new_local(&path)
            .build()
            .await
            .expect("temp db");
        crate::db::migrate(&db).await.expect("migrate");
        let app = build_router(AppState {
            db: std::sync::Arc::new(db),
        });

        for uri in [
            "/health",
            "/api/health",
            "/",
            "/api/docs",
            "/api/openapi.json",
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(uri)
                        .body(Body::empty())
                        .expect("request"),
                )
                .await
                .expect("response");
            assert_eq!(response.status(), StatusCode::OK, "{uri}");
        }

        std::fs::remove_file(path).expect("cleanup");
    }
}
