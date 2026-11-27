//! The `/api/solana-pay` routes a wallet calls after scanning a code.

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap};
use axum::response::IntoResponse;
use axum::Json;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;

use crate::payments::{parse_pubkey, resolve_endpoint, ApiError, ApiResult};
use crate::AppState;

use super::invoice::{decode_invoice, format_lamports};
use super::model::{AccountQuery, AccountRequest, RequestInfo, TransactionRequest};
use super::wire::{settle_payment_transaction, SettlePaymentRequest};

/// `GET /api/solana-pay/{token}` — wallet metadata for the payment.
pub async fn info(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(token): Path<String>,
) -> ApiResult<Json<RequestInfo>> {
    let invoice = decode_invoice(&token)?;
    let payable = invoice.payable_lamports(&state.db).await?;
    Ok(Json(RequestInfo {
        label: invoice.label(),
        icon: icon_url(&headers),
        message: invoice.description(payable),
    }))
}

/// The icon the wallet shows next to the label. The API serves it itself, so
/// the URL is absolute and points at a file the wallet can fetch.
pub async fn icon() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "image/svg+xml"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        ICON_SVG,
    )
}

/// A mark, not the full web logo: wallets render it at a few dozen pixels.
const ICON_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect width="64" height="64" rx="14" fill="#0b1220"/><path d="M23 47V17h11a10.5 10.5 0 0 1 0 21h-5" fill="none" stroke="#fff" stroke-width="6" stroke-linecap="round" stroke-linejoin="round"/><circle cx="29" cy="27" r="2.6" fill="#fff"/></svg>"##;

/// `https://<api-host>/api/solana-pay/icon.svg`, from the request the wallet
/// just made. Behind Vercel the scheme comes from the forwarded header.
fn icon_url(headers: &HeaderMap) -> Option<String> {
    let host = headers.get(header::HOST)?.to_str().ok()?;
    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_else(|| {
            if host.starts_with("localhost") || host.starts_with("127.0.0.1") {
                "http"
            } else {
                "https"
            }
        });
    Some(format!("{scheme}://{host}/api/solana-pay/icon.svg"))
}

/// `POST /api/solana-pay/{token}` — builds the transaction the wallet asked
/// for. Nothing is written anywhere: the wallet signs and sends it itself,
/// which is what creates the invoice account on chain.
pub async fn transaction(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Query(query): Query<AccountQuery>,
    Json(request): Json<AccountRequest>,
) -> ApiResult<Json<TransactionRequest>> {
    let invoice = decode_invoice(&token)?;
    let account = query
        .account
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or(&request.account);
    let payer = parse_pubkey(account, "account")?;

    // A discount code rides in the invoice, but the amount it prices is
    // resolved against the seller's own list. The address follows the amount:
    // the program derives a different invoice per price, so this pays exactly
    // what the buyer's screen shows.
    let payable = invoice.payable_lamports(&state.db).await?;
    let payment = invoice.address_for(payable)?;

    // Only known clusters, never the endpoint carried in the token: a code is
    // just data handed to us by a stranger, and this fetch must not become a
    // request to an arbitrary host.
    let endpoint = resolve_endpoint(None, &invoice.cluster)?;
    let rpc = RpcClient::new(endpoint);
    let blockhash = rpc.get_latest_blockhash().await.map_err(|error| {
        tracing::info!(%error, "latest blockhash unavailable for a solana pay request");
        ApiError::Internal("The network did not return a blockhash. Try again.".into())
    })?;

    let wire = settle_payment_transaction(SettlePaymentRequest {
        payer,
        payment,
        seller: invoice.seller()?,
        reference: invoice.reference()?,
        amount: payable,
        memo: &invoice.memo,
        blockhash: blockhash.to_bytes(),
    });

    Ok(Json(TransactionRequest {
        transaction: STANDARD.encode(wire),
        message: format!("Pay {} SOL to Payvand", format_lamports(payable)),
    }))
}
