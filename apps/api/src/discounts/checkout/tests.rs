use super::*;

async fn seeded_db() -> libsql::Database {
    let path = std::env::temp_dir().join(format!("payvand-discount-test-{}.db", auth::random_id()));
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
