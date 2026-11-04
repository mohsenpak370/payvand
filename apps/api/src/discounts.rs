//! Discount codes.
//!
//! A seller defines percentage-off codes on their account; a buyer types one
//! into the scan-to-pay screen and the API prices the invoice. The discount
//! never changes the amount already committed by the QR code — it derives a
//! *second* payment address from the same seller and reference at the lower
//! amount, and the budget/silent path pays that address exactly like the full
//! price. Codes are stored per account, not per invoice, so one list works
//! for every QR the seller creates.

use axum::extract::{Json, Path, State};
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};

use crate::auth;
use crate::payments::{bad, ApiError, ApiResult, PaymentDiscount};

/// Codes per account — small on purpose so the list stays a list, not a system.
const MAX_CODES_PER_USER: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountCode {
    pub id: String,
    pub code: String,
    pub percent_off: u32,
}

/// What a seller posts to create a code.
#[derive(Debug, Deserialize)]
pub struct CreateDiscountRequest {
    pub code: String,
    pub percent_off: u32,
}

/// What a buyer posts to check a code against an invoice price. `seller_user`
/// comes from the QR payload, so the lookup is always scoped to the seller on
/// the code the buyer actually scanned.
#[derive(Debug, Deserialize)]
pub struct ValidateDiscountRequest {
    pub code: String,
    pub seller_user: String,
    pub price_lamports: u64,
}

/// The priced result the buyer's screen shows above the pay button.
#[derive(Debug, Serialize)]
pub struct DiscountPreview {
    pub code: String,
    pub percent_off: u32,
    pub discounted_lamports: u64,
    pub saved_lamports: u64,
}

/// Uppercases and range-checks a seller- or buyer-supplied code.
pub fn normalize_code(raw: &str) -> ApiResult<String> {
    let code = raw.trim().to_uppercase();
    let ok_len = (3..=32).contains(&code.len());
    let ok_chars = code
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !ok_len || !ok_chars {
        return Err(bad(
            "A discount code is 3 to 32 letters, numbers, dashes or underscores.",
        ));
    }
    Ok(code)
}

fn validate_percent(percent_off: u32) -> ApiResult<u32> {
    if !(1..=99).contains(&percent_off) {
        return Err(bad("A discount must be between 1% and 99%."));
    }
    Ok(percent_off)
}

/// `price - percent`, rounded down to the nearest lamport so the seller
/// never under-collects by rounding up.
pub fn discounted_lamports(price_lamports: u64, percent_off: u32) -> u64 {
    let percent_off = percent_off.min(99);
    (u128::from(price_lamports) * u128::from(100 - percent_off) / 100) as u64
}

/// Resolves a code a seller attached while creating an invoice. Scoped to the
/// seller's own list: the request body can name a code, never pick a discount.
pub async fn resolve_for_seller(
    db: &libsql::Database,
    user_id: &str,
    raw: &str,
) -> ApiResult<PaymentDiscount> {
    let code = normalize_code(raw)?;
    let connection = db.connect().map_err(auth::internal)?;

    let mut rows = connection
        .query(
            "SELECT percent_off FROM discount_codes WHERE user_id = ?1 AND code = ?2",
            (user_id, code.clone()),
        )
        .await
        .map_err(auth::internal)?;

    let percent_off = match rows.next().await.map_err(auth::internal)? {
        Some(row) => read_percent(&row, 0)?,
        None => return Err(bad(format!("You have no discount code called {code}."))),
    };
    Ok(PaymentDiscount { code, percent_off })
}

/// The signed-in account's codes, newest first.
pub async fn list(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<DiscountCode>>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;

    let mut rows = connection
        .query(
            "SELECT id, code, percent_off FROM discount_codes \
             WHERE user_id = ?1 ORDER BY created_at DESC",
            [user.id.clone()],
        )
        .await
        .map_err(auth::internal)?;

    let mut codes = Vec::new();
    while let Some(row) = rows.next().await.map_err(auth::internal)? {
        codes.push(DiscountCode {
            id: row.get(0).map_err(auth::internal)?,
            code: row.get(1).map_err(auth::internal)?,
            percent_off: read_percent(&row, 2)?,
        });
    }
    Ok(Json(codes))
}

/// Creates a code for the signed-in account. Only signed-in sellers can.
pub async fn create(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateDiscountRequest>,
) -> ApiResult<Json<DiscountCode>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let code = normalize_code(&request.code)?;
    let percent_off = validate_percent(request.percent_off)?;
    let connection = state.db.connect().map_err(auth::internal)?;

    let mut rows = connection
        .query(
            "SELECT COUNT(*) FROM discount_codes WHERE user_id = ?1",
            [user.id.clone()],
        )
        .await
        .map_err(auth::internal)?;
    let count: i64 = match rows.next().await.map_err(auth::internal)? {
        Some(row) => row.get(0).map_err(auth::internal)?,
        None => 0,
    };
    if count >= MAX_CODES_PER_USER as i64 {
        return Err(bad(format!(
            "You already have the maximum of {MAX_CODES_PER_USER} discount codes."
        )));
    }

    let id = auth::random_id();
    connection
        .execute(
            "INSERT INTO discount_codes (id, user_id, code, percent_off, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (
                id.clone(),
                user.id,
                code.clone(),
                i64::from(percent_off),
                auth::unix_now(),
            ),
        )
        .await
        .map_err(|error| {
            if auth::is_unique_violation(&error) {
                bad(format!("You already have a code called {code}."))
            } else {
                auth::internal(error)
            }
        })?;

    Ok(Json(DiscountCode {
        id,
        code,
        percent_off,
    }))
}

/// Deletes one of the signed-in account's codes.
pub async fn delete(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;
    let deleted = connection
        .execute(
            "DELETE FROM discount_codes WHERE id = ?1 AND user_id = ?2",
            (id, user.id),
        )
        .await
        .map_err(auth::internal)?;
    if deleted == 0 {
        return Err(bad("That discount code no longer exists."));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// Prices an invoice with a code. Public, like the status read: the buyer
/// may have opened the payment link without an account. The lookup is scoped
/// to the `seller_user` carried by the code they scanned, and the response
/// reveals only that code's percentage for that seller's invoice.
pub async fn validate(
    State(state): State<crate::AppState>,
    Json(request): Json<ValidateDiscountRequest>,
) -> ApiResult<Json<DiscountPreview>> {
    if request.price_lamports == 0 {
        return Err(bad("price_lamports must be greater than 0"));
    }
    let code = normalize_code(&request.code)?;

    let connection = state.db.connect().map_err(auth::internal)?;
    let mut rows = connection
        .query(
            "SELECT percent_off FROM discount_codes WHERE user_id = ?1 AND code = ?2",
            (request.seller_user, code.clone()),
        )
        .await
        .map_err(auth::internal)?;

    let percent_off = match rows.next().await.map_err(auth::internal)? {
        Some(row) => read_percent(&row, 0)?,
        None => return Err(bad("That discount code is not valid.")),
    };

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

/// Reads the `percent_off` column, which the table's CHECK keeps in 1..=99.
fn read_percent(row: &libsql::Row, index: i32) -> ApiResult<u32> {
    let raw: i64 = row.get(index).map_err(auth::internal)?;
    u32::try_from(raw).map_err(|_| ApiError::Internal("stored percent_off out of range".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_codes() {
        assert_eq!(normalize_code(" save10 ").unwrap(), "SAVE10");
        assert!(normalize_code("ab").is_err());
        assert!(normalize_code("no spaces").is_err());
        assert!(normalize_code(&"x".repeat(33)).is_err());
    }

    #[test]
    fn prices_the_discount() {
        assert_eq!(discounted_lamports(1_000_000_000, 10), 900_000_000);
        // Rounds down: the seller never under-collects by a fraction.
        assert_eq!(discounted_lamports(333, 50), 166);
        assert_eq!(discounted_lamports(999, 99), 9);
        // Even an out-of-range percent can never zero an invoice.
        assert!(discounted_lamports(1_000, 200) > 0);
    }

    #[test]
    fn validates_percent() {
        assert!(validate_percent(0).is_err());
        assert!(validate_percent(100).is_err());
        assert_eq!(validate_percent(50).unwrap(), 50);
    }
}
