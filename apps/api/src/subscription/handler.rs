//! The `/api/subscription` routes.

use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;

use crate::auth;
use crate::payments::{self, bad, ApiResult, CreateOption, CreatePaymentRequest};

use super::credit::{credit_invoice, credit_unconfirmed};
use super::feed::rates;
use super::model::{ConfirmRequest, InvoiceRequest, InvoiceResponse, SubscriptionStatus};
use super::period::latest;
use super::platform::{cluster, new_reference, platform_seller};
use super::pricing::{quoted, quotes_for, PERIOD_SECS, PLAN_PRICE_EUR_CENTS};

/// `GET /api/subscription` — the account's period and the plan's three prices.
pub async fn status(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<SubscriptionStatus>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let (mut active, mut expires_at_unix, mut currency, mut amount) =
        latest(&state, &user.id).await?;
    if !active && credit_unconfirmed(&state, &user.id).await > 0 {
        (active, expires_at_unix, currency, amount) = latest(&state, &user.id).await?;
    }
    let cluster = cluster();
    // Every feed being down costs the prices, not the answer: `rates()` has
    // already logged why, and an active period must still be visible.
    let rates = rates().await.ok();
    let (quotes, rated_at_unix, rates_available) = quoted(rates.as_ref(), &cluster);
    Ok(Json(SubscriptionStatus {
        active,
        expires_at_unix,
        currency,
        amount,
        price_eur_cents: PLAN_PRICE_EUR_CENTS,
        period_days: PERIOD_SECS / (24 * 60 * 60),
        quotes,
        rated_at_unix,
        rates_available,
    }))
}

/// `POST /api/subscription/invoice` — one invoice for the plan, in the currency
/// the seller picked, priced at the quote of the moment. The row it writes is
/// what `confirm` checks the payment against, so a settled payment for another
/// amount cannot buy a period.
pub async fn create_invoice(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Json(request): Json<InvoiceRequest>,
) -> ApiResult<Json<InvoiceResponse>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let cluster = cluster();
    let seller = platform_seller()?;
    let rates = rates().await?;
    let quote = quotes_for(&rates, &cluster)
        .into_iter()
        .find(|quote| quote.currency == request.currency.to_ascii_lowercase())
        .ok_or_else(|| bad("Pick one of sol, usdc or eurc."))?;
    if quote.amount == 0 {
        return Err(bad("That currency has no price right now. Try another."));
    }

    let record = payments::create_payment(CreatePaymentRequest {
        payment_pda: String::new(),
        seller: seller.to_string(),
        reference: new_reference(&user.id).to_string(),
        amount_lamports: 0,
        options: vec![CreateOption {
            currency: quote.currency.clone(),
            amount: quote.amount,
        }],
        memo: "Shops subscription, 30 days".to_string(),
        name: Some("Shops subscription".to_string()),
        description: Some("€1.70 for 30 days of shops and products.".to_string()),
        items: Vec::new(),
        discount_code: None,
        allow_discounts: false,
        cluster: cluster.clone(),
        endpoint: None,
    })?;

    let connection = state.db.connect().map_err(auth::internal)?;
    connection
        .execute(
            "INSERT INTO subscription_invoices (payment_id, user_id, currency, amount, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (
                record.id.clone(),
                user.id.clone(),
                quote.currency.clone(),
                i64::try_from(quote.amount).map_err(|_| bad("That price is too large."))?,
                auth::unix_now(),
            ),
        )
        .await
        .map_err(auth::internal)?;

    // Built before `id` moves out of the record.
    let payload = record.qr_payload();
    Ok(Json(InvoiceResponse {
        payment_id: record.id,
        payload,
        currency: quote.currency,
        amount: quote.amount,
        expires_at_unix: auth::unix_now() + PERIOD_SECS,
    }))
}

/// `POST /api/subscription/confirm` — records the period once the invoice has
/// settled. The cluster is the source of truth: a payment id that never settled
/// buys nothing, and confirming twice is a no-op.
pub async fn confirm(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Json(request): Json<ConfirmRequest>,
) -> ApiResult<Json<SubscriptionStatus>> {
    let user = auth::require_user(&state.db, &headers).await?;
    credit_invoice(&state, &user.id, &request.payment_id).await?;
    status(State(state), headers).await
}
