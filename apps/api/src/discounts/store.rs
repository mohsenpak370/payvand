//! Reading a seller's codes and a buyer's past uses from the database.

use crate::auth;
use crate::payments::{bad, ApiError, ApiResult, PaymentDiscount};

use super::rules::normalize_code;

/// One account's own percentage for a code, if that account has it. The only
/// place a discount percentage ever comes from: a request, an invoice or a
/// payment code can name a code, never a percentage.
pub async fn percent_for(
    db: &libsql::Database,
    user_id: &str,
    code: &str,
) -> ApiResult<Option<u32>> {
    let connection = db.connect().map_err(auth::internal)?;
    let mut rows = connection
        .query(
            "SELECT percent_off FROM discount_codes WHERE user_id = ?1 AND code = ?2",
            (user_id, code),
        )
        .await
        .map_err(auth::internal)?;
    match rows.next().await.map_err(auth::internal)? {
        Some(row) => Ok(Some(read_percent(&row, 0)?)),
        None => Ok(None),
    }
}

/// Resolves a code a seller attached while creating an invoice. Scoped to the
/// seller's own list: the request body can name a code, never pick a discount.
pub async fn resolve_for_seller(
    db: &libsql::Database,
    user_id: &str,
    raw: &str,
) -> ApiResult<PaymentDiscount> {
    let code = normalize_code(raw)?;
    let Some(percent_off) = percent_for(db, user_id, &code).await? else {
        return Err(bad(format!("You have no discount code called {code}.")));
    };
    Ok(PaymentDiscount { code, percent_off })
}

/// Whether this wallet has already spent this code. The seller is named by its
/// account id here: every caller has already resolved it from a wallet address.
pub(super) async fn used_by(
    db: &libsql::Database,
    user_id: &str,
    code: &str,
    buyer: &str,
) -> ApiResult<bool> {
    let connection = db.connect().map_err(auth::internal)?;
    let mut rows = connection
        .query(
            "SELECT COUNT(*) FROM discount_redemptions \
             WHERE user_id = ?1 AND code = ?2 AND buyer = ?3",
            (user_id, code, buyer),
        )
        .await
        .map_err(auth::internal)?;
    let row = rows.next().await.map_err(auth::internal)?;
    let count: i64 = match row {
        Some(row) => row.get(0).map_err(auth::internal)?,
        None => 0,
    };
    Ok(count > 0)
}

/// Reads the `percent_off` column, which the table's CHECK keeps in 1..=99.
pub(super) fn read_percent(row: &libsql::Row, index: i32) -> ApiResult<u32> {
    let raw: i64 = row.get(index).map_err(auth::internal)?;
    u32::try_from(raw).map_err(|_| ApiError::Internal("stored percent_off out of range".into()))
}
