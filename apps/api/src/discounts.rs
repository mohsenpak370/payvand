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

/// What a buyer posts to check a code against an invoice price: the code, the
/// seller's **wallet address** and the price.
///
/// The seller is named by its address, never by an account id: a payment code
/// is data a buyer can edit, and an account id in it would let them point the
/// lookup at their own list. The address is the one the invoice's own PDA was
/// derived from, so it cannot be swapped for someone else's.
#[derive(Debug, Deserialize)]
pub struct ValidateDiscountRequest {
    pub code: String,
    /// Optional only so a page cached before this field existed answers with a
    /// sentence instead of a deserialization error; it is required in practice.
    #[serde(default)]
    pub seller: Option<String>,
    pub price_lamports: u64,
    /// The wallet asking. A code is once per buyer, and the buyer is the wallet
    /// that pays — an account is not needed to open a payment link. Absent on
    /// older pages, which are priced but not counted as a use.
    #[serde(default)]
    pub buyer: Option<String>,
}

/// The priced result the buyer's screen shows above the pay button.
#[derive(Debug, Serialize)]
pub struct DiscountPreview {
    pub code: String,
    pub percent_off: u32,
    pub discounted_lamports: u64,
    pub saved_lamports: u64,
}

/// What records a use of a code: the code, the seller whose list it belongs to
/// and the settled invoice the buyer paid. The buyer is read off that invoice's
/// stored settlement — the request cannot name someone else as the user.
#[derive(Debug, Clone, Deserialize)]
pub struct UseDiscountRequest {
    pub code: String,
    pub seller: String,
    pub invoice: String,
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
async fn used_by(db: &libsql::Database, user_id: &str, code: &str, buyer: &str) -> ApiResult<bool> {
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

/// Reads the `percent_off` column, which the table's CHECK keeps in 1..=99.
fn read_percent(row: &libsql::Row, index: i32) -> ApiResult<u32> {
    let raw: i64 = row.get(index).map_err(auth::internal)?;
    u32::try_from(raw).map_err(|_| ApiError::Internal("stored percent_off out of range".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn seeded_db() -> libsql::Database {
        let path =
            std::env::temp_dir().join(format!("payvand-discount-test-{}.db", auth::random_id()));
        let db = libsql::Builder::new_local(&path)
            .build()
            .await
            .expect("temp db");
        crate::db::migrate(&db).await.expect("migrate");
        db
    }

    async fn add_account(db: &libsql::Database, id: &str, email: &str, wallet: &str) {
        db.connect()
            .expect("connection")
            .execute(
                "INSERT INTO users (id, email, password_hash, wallet, created_at) \
                 VALUES (?1, ?2, 'unused-in-tests', ?3, 0)",
                (id, email, wallet),
            )
            .await
            .expect("account");
    }

    async fn add_code(db: &libsql::Database, user_id: &str, code: &str, percent_off: i64) {
        db.connect()
            .expect("connection")
            .execute(
                "INSERT INTO discount_codes (id, user_id, code, percent_off, created_at) \
                 VALUES (?1, ?2, ?3, ?4, 0)",
                (format!("{user_id}-{code}"), user_id, code, percent_off),
            )
            .await
            .expect("code");
    }

    /// The attack the seller-address lookup closes: a buyer posts their own
    /// account id next to a 99% code of theirs and asks for a price on someone
    /// else's invoice.
    #[tokio::test]
    async fn prices_only_with_the_invoices_sellers_codes() {
        let db = seeded_db().await;
        add_account(&db, "seller-a", "a@example.com", "SellerWalletA").await;
        add_code(&db, "seller-a", "SAVE10", 10).await;
        add_account(&db, "buyer-b", "b@example.com", "BuyerWalletB").await;
        add_code(&db, "buyer-b", "MINE99", 99).await;
        let state = crate::AppState {
            db: std::sync::Arc::new(db),
        };

        let own = validate(
            State(state.clone()),
            Json(ValidateDiscountRequest {
                code: "save10".into(),
                seller: Some("SellerWalletA".into()),
                price_lamports: 1_000_000,
                buyer: None,
            }),
        )
        .await
        .expect("the seller's own code prices the invoice")
        .0;
        assert_eq!(own.percent_off, 10);
        assert_eq!(own.discounted_lamports, 900_000);

        let attack = validate(
            State(state),
            Json(ValidateDiscountRequest {
                code: "MINE99".into(),
                seller: Some("SellerWalletA".into()),
                price_lamports: 1_000_000,
                buyer: None,
            }),
        )
        .await;
        assert!(
            attack.is_err(),
            "a stranger's 99% code must not price this invoice"
        );
    }

    #[tokio::test]
    async fn refuses_a_seller_without_an_account() {
        let db = seeded_db().await;
        add_account(&db, "buyer-b", "b@example.com", "BuyerWalletB").await;
        add_code(&db, "buyer-b", "MINE99", 99).await;
        let state = crate::AppState {
            db: std::sync::Arc::new(db),
        };

        let refused = validate(
            State(state),
            Json(ValidateDiscountRequest {
                code: "MINE99".into(),
                seller: Some("NobodyWallet".into()),
                price_lamports: 1_000_000,
                buyer: None,
            }),
        )
        .await;
        assert!(refused.is_err());
    }

    async fn token_for(db: &libsql::Database, user_id: &str) -> String {
        let connection = db.connect().expect("connection");
        auth::issue_session(&connection, user_id)
            .await
            .expect("session")
    }

    fn bearer(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            format!("Bearer {token}").parse().expect("header value"),
        );
        headers
    }

    async fn add_settlement(db: &libsql::Database, invoice: &str, seller: &str, payer: &str) {
        db.connect()
            .expect("connection")
            .execute(
                "INSERT INTO settled_payments \
                    (invoice, endpoint, seller, reference, mint, amount, memo, payer, created_at, settled_at) \
                 VALUES (?1, 'https://api.devnet.solana.com', ?2, 'ref', '', 1000000, '', ?3, 0, 0)",
                (invoice, seller, payer),
            )
            .await
            .expect("settlement");
    }

    /// The code is once per buyer wallet: recorded from a settled invoice's
    /// payer, refused at validation for that wallet, and still free for another.
    #[tokio::test]
    async fn records_a_code_once_per_buyer_wallet() {
        let db = seeded_db().await;
        add_account(&db, "seller-a", "a@example.com", "SellerWalletA").await;
        add_code(&db, "seller-a", "SAVE10", 10).await;
        add_settlement(&db, "InvoiceOne", "SellerWalletA", "BuyerWallet").await;
        add_settlement(&db, "InvoiceTwo", "SellerWalletA", "BuyerWallet").await;
        let seller_token = token_for(&db, "seller-a").await;
        let state = crate::AppState {
            db: std::sync::Arc::new(db),
        };

        let preview = |buyer: &'static str| ValidateDiscountRequest {
            code: "SAVE10".into(),
            seller: Some("SellerWalletA".into()),
            price_lamports: 1_000_000,
            buyer: Some(buyer.into()),
        };
        let use_request = |invoice: &'static str| UseDiscountRequest {
            code: "SAVE10".into(),
            seller: "SellerWalletA".into(),
            invoice: invoice.into(),
        };

        // Before the payment, the wallet is free to price the invoice.
        assert!(validate(State(state.clone()), Json(preview("BuyerWallet")))
            .await
            .is_ok());

        // Once the invoice has settled, its seller's session records the use
        // against the invoice's payer.
        let _ = use_code(
            State(state.clone()),
            bearer(&seller_token),
            Json(use_request("InvoiceOne")),
        )
        .await
        .expect("recorded");

        let refused = validate(State(state.clone()), Json(preview("BuyerWallet"))).await;
        assert!(
            matches!(&refused, Err(ApiError::BadRequest(message)) if message.contains("already been used")),
            "the same wallet must not price with the code twice"
        );
        // Another wallet still can.
        assert!(validate(State(state.clone()), Json(preview("OtherWallet")))
            .await
            .is_ok());

        // Reporting the same invoice again is the same use...
        assert!(use_code(
            State(state.clone()),
            bearer(&seller_token),
            Json(use_request("InvoiceOne"))
        )
        .await
        .is_ok());
        // ...while a second invoice from the same wallet is a second use.
        assert!(use_code(
            State(state.clone()),
            bearer(&seller_token),
            Json(use_request("InvoiceTwo"))
        )
        .await
        .is_err());

        // An invoice nobody has paid proves nothing, so no use is recorded.
        assert!(use_code(
            State(state),
            bearer(&seller_token),
            Json(use_request("Unsettled"))
        )
        .await
        .is_err());
    }

    /// The abuse the session requirement closes: with a settled invoice and a
    /// code name, a stranger could otherwise spend that buyer's copy of the
    /// code. Only the seller who owns the invoice may record the use.
    #[tokio::test]
    async fn a_stranger_cannot_burn_a_buyers_code() {
        let db = seeded_db().await;
        add_account(&db, "seller-a", "a@example.com", "SellerWalletA").await;
        add_code(&db, "seller-a", "SAVE10", 10).await;
        // A second seller with a code of the same name, so the check under test
        // is the invoice's seller, not merely whether the code resolves.
        add_account(&db, "seller-b", "b@example.com", "SellerWalletB").await;
        add_code(&db, "seller-b", "SAVE10", 10).await;
        add_settlement(&db, "InvoiceOne", "SellerWalletA", "BuyerWallet").await;
        let stranger_token = token_for(&db, "seller-b").await;
        let state = crate::AppState {
            db: std::sync::Arc::new(db),
        };

        let request = UseDiscountRequest {
            code: "SAVE10".into(),
            seller: "SellerWalletA".into(),
            invoice: "InvoiceOne".into(),
        };

        // No session: nothing is recorded.
        assert!(matches!(
            use_code(
                State(state.clone()),
                HeaderMap::new(),
                Json(request.clone())
            )
            .await,
            Err(ApiError::Unauthorized)
        ));

        // Another seller cannot record a use against this seller's code.
        assert!(matches!(
            use_code(
                State(state.clone()),
                bearer(&stranger_token),
                Json(request.clone())
            )
            .await,
            Err(ApiError::Forbidden(_))
        ));

        // Nor against their own code on someone else's settled invoice.
        let theirs = UseDiscountRequest {
            code: "SAVE10".into(),
            seller: "SellerWalletB".into(),
            invoice: "InvoiceOne".into(),
        };
        assert!(
            use_code(State(state.clone()), bearer(&stranger_token), Json(theirs))
                .await
                .is_err()
        );

        // The buyer's code is untouched: the preview still prices it.
        assert!(validate(
            State(state),
            Json(ValidateDiscountRequest {
                code: "SAVE10".into(),
                seller: Some("SellerWalletA".into()),
                price_lamports: 1_000_000,
                buyer: Some("BuyerWallet".into()),
            })
        )
        .await
        .is_ok());
    }

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
