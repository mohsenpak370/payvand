//! A code at checkout: pricing an invoice with it, then recording its use
//! once the payment has settled.

use axum::extract::{Json, State};
use axum::http::HeaderMap;

use crate::auth;
use crate::payments::{bad, ApiError, ApiResult};

use super::model::{DiscountPreview, UseDiscountRequest, ValidateDiscountRequest};
use super::rules::{discounted_lamports, normalize_code};
use super::store::{percent_for, used_by};

/// Records that a buyer spent a code, once the invoice they paid has settled.
///
/// Only the seller who owns the code may record a use, with their own session:
/// anyone else could name a stranger's settled invoice and burn that buyer's
/// copy of a code they never used. The buyer is then read from the settlement
/// the status endpoint stored, never from the request, so the payer on chain is
/// the wallet the code is counted against and a seller cannot spend it for
/// someone else.
pub async fn use_code(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Json(request): Json<UseDiscountRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let code = normalize_code(&request.code)?;
    let seller_wallet = request.seller.trim();
    let seller = auth::seller_account(&state.db, seller_wallet)
        .await?
        .ok_or_else(|| bad("That seller does not offer discount codes."))?;
    if seller != user.id {
        return Err(ApiError::Forbidden(
            "Only the seller whose code this is can record a use.".into(),
        ));
    }
    percent_for(&state.db, &seller, &code)
        .await?
        .ok_or_else(|| bad("That discount code is not valid."))?;

    let connection = state.db.connect().map_err(auth::internal)?;
    let mut rows = connection
        .query(
            "SELECT payer, seller FROM settled_payments WHERE invoice = ?1 \
             ORDER BY settled_at DESC LIMIT 1",
            [request.invoice.trim().to_string()],
        )
        .await
        .map_err(auth::internal)?;
    let (buyer, paid_seller): (String, String) = match rows.next().await.map_err(auth::internal)? {
        Some(row) => (
            row.get(0).map_err(auth::internal)?,
            row.get(1).map_err(auth::internal)?,
        ),
        None => return Err(bad("This payment has not settled yet.")),
    };
    // The invoice has to be this seller's own, or a seller could name another
    // seller's settled invoice and spend that buyer's copy of a shared code name.
    if paid_seller.trim() != seller_wallet {
        return Err(bad("That payment did not settle to this seller."));
    }

    connection
        .execute(
            "INSERT INTO discount_redemptions (id, user_id, code, buyer, invoice, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
             ON CONFLICT(user_id, code, buyer) DO NOTHING",
            (
                auth::random_id(),
                seller.clone(),
                code.clone(),
                buyer.clone(),
                request.invoice.trim().to_string(),
                auth::unix_now(),
            ),
        )
        .await
        .map_err(auth::internal)?;

    // The same invoice reported twice is the same use, not a second one; a
    // different invoice is the same wallet trying the code again.
    let mut rows = connection
        .query(
            "SELECT invoice FROM discount_redemptions \
             WHERE user_id = ?1 AND code = ?2 AND buyer = ?3",
            (seller, code, buyer),
        )
        .await
        .map_err(auth::internal)?;
    if let Some(row) = rows.next().await.map_err(auth::internal)? {
        let stored: String = row.get(0).map_err(auth::internal)?;
        if stored != request.invoice.trim() {
            return Err(bad("This code has already been used by this buyer."));
        }
    }

    Ok(Json(serde_json::json!({ "recorded": true })))
}

/// Prices an invoice with a code. Public, like the status read: the buyer
/// may have opened the payment link without an account.
///
/// The account is found from the seller's wallet address, so the percentage
/// always comes from the list of the seller whose address the buyer is paying
/// — a buyer cannot price someone else's invoice with a code of their own.
pub async fn validate(
    State(state): State<crate::AppState>,
    Json(request): Json<ValidateDiscountRequest>,
) -> ApiResult<Json<DiscountPreview>> {
    if request.price_lamports == 0 {
        return Err(bad("price_lamports must be greater than 0"));
    }
    let code = normalize_code(&request.code)?;

    let seller_wallet = request
        .seller
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| bad("Reload this payment page to apply a discount code."))?;
    let seller = auth::seller_account(&state.db, seller_wallet)
        .await?
        .ok_or_else(|| bad("That seller does not offer discount codes."))?;
    let percent_off = percent_for(&state.db, &seller, &code)
        .await?
        .ok_or_else(|| bad("That discount code is not valid."))?;

    // A code is once per buyer, so a wallet that has already spent it is told
    // here rather than at the pay button.
    if let Some(buyer) = request
        .buyer
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        if used_by(&state.db, &seller, &code, buyer).await? {
            return Err(bad(
                "This discount code has already been used by this wallet.",
            ));
        }
    }

    let discounted = discounted_lamports(request.price_lamports, percent_off);
    if discounted == 0 || discounted >= request.price_lamports {
        return Err(bad("That discount does not apply to this price."));
    }

    Ok(Json(DiscountPreview {
        code,
        percent_off,
        discounted_lamports: discounted,
        saved_lamports: request.price_lamports - discounted,
    }))
}

#[cfg(test)]
mod tests;
