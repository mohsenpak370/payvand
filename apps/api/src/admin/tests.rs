use super::airdrops::{DEFAULT_AIRDROP_PAGE, MAX_AIRDROP_PAGE};
use super::records::{
    list_codes, list_products, list_shops, update_product, update_shop, ProductEdit, RecordQuery,
    ShopEdit,
};
use super::{
    airdrops, delete_user,
    feed::shape_feed,
    is_admin, list_payments, list_users, require_admin, scalar, stats,
    users::{DEFAULT_PAGE, MAX_PAGE},
    AirdropQuery, PaymentListQuery, UserListQuery,
};
use crate::auth::{random_id, ForgotPasswordRequest, SignupRequest};
use crate::payments::{ApiError, PaymentRecord, PaymentStatus};
use axum::extract::{
    Json as JsonExtractor, Path, Query as QueryExtractor, State as StateExtractor,
};
use axum::http::HeaderMap;

async fn test_state() -> (crate::AppState, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!("payvand-admin-test-{}.db", random_id()));
    let db = libsql::Builder::new_local(&path)
        .build()
        .await
        .expect("temp db");
    crate::db::migrate(&db).await.expect("migrate");
    (
        crate::AppState {
            db: std::sync::Arc::new(db),
        },
        path,
    )
}

fn bearer(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {token}").parse().expect("header value"),
    );
    headers
}

async fn signup(state: &crate::AppState, email: &str) -> String {
    crate::auth::signup(
        StateExtractor(state.clone()),
        JsonExtractor(SignupRequest {
            email: email.to_string(),
            password: "correct horse battery staple".into(),
        }),
    )
    .await
    .expect("signup")
    .0
    .token
}

/// One test owns `ADMIN_EMAILS` end to end: `set_var` is process-global,
/// so no other test may read it concurrently.
#[tokio::test]
async fn admin_gate_stats_search_and_delete() {
    std::env::set_var("ADMIN_EMAILS", " Boss@Example.com, other@example.com ");

    let (state, path) = test_state().await;

    // Parsing: normalization, typos, empty and missing lists.
    assert!(is_admin("boss@example.com"), "normalized list matches");
    assert!(!is_admin("anyone@example.com"));
    assert!(!is_admin("boss@"));
    assert!(!is_admin("boss@example.com "));
    std::env::remove_var("ADMIN_EMAILS");
    assert!(!is_admin("boss@example.com"), "unset list grants nothing");
    assert!(!is_admin(""));
    std::env::set_var("ADMIN_EMAILS", "boss@example.com");

    let boss = signup(&state, "boss@example.com").await;
    let pleb = signup(&state, "pleb@example.com").await;

    let admin_headers = bearer(&boss);
    let user_headers = bearer(&pleb);

    // Signed in but not listed: 403. No session at all: 401.
    let denied = require_admin(&state.db, &user_headers).await;
    assert!(matches!(denied, Err(ApiError::Forbidden(_))));
    let anonymous = require_admin(&state.db, &HeaderMap::new()).await;
    assert!(matches!(anonymous, Err(ApiError::Unauthorized)));

    // Stats see both accounts.
    let stats = stats(StateExtractor(state.clone()), admin_headers.clone())
        .await
        .expect("stats");
    assert_eq!(stats.0.users, 2);
    assert_eq!(stats.0.active_sessions, 2);
    assert_eq!(stats.0.pending_resets, 0);

    // Search narrows the list.
    let all = list_users(
        StateExtractor(state.clone()),
        admin_headers.clone(),
        QueryExtractor(UserListQuery {
            search: None,
            limit: None,
            offset: None,
        }),
    )
    .await
    .expect("list");
    assert_eq!(all.0.items.len(), 2);
    assert_eq!(all.0.total, 2);
    assert_eq!(all.0.limit, DEFAULT_PAGE);
    let found = list_users(
        StateExtractor(state.clone()),
        admin_headers.clone(),
        QueryExtractor(UserListQuery {
            search: Some("pleb".into()),
            limit: None,
            offset: None,
        }),
    )
    .await
    .expect("search");
    assert_eq!(found.0.items.len(), 1);
    assert_eq!(found.0.total, 1);
    assert_eq!(found.0.items[0].email, "pleb@example.com");
    assert_eq!(found.0.items[0].sessions, 1);

    // Records: an admin can read and correct them, but a normal account cannot.
    let seller_id = found.0.items[0].id.clone();
    let records_db = state.db.connect().expect("connect");
    records_db
        .execute(
            "INSERT INTO shops (id, user_id, name, description, currency, created_at) \
             VALUES ('shop-1', ?1, 'Corner Shop', 'Flat whites', 'sol', 1)",
            [seller_id.as_str()],
        )
        .await
        .expect("shop");
    records_db
        .execute(
            "INSERT INTO products (id, user_id, shop_id, name, details, category, \
             price_lamports, created_at) \
             VALUES ('product-1', ?1, 'shop-1', 'Flat white', 'Warm', 'coffee', 12000000, 2)",
            [seller_id.as_str()],
        )
        .await
        .expect("product");
    records_db
        .execute(
            "INSERT INTO discount_codes (id, user_id, code, percent_off, created_at) \
             VALUES ('code-1', ?1, 'SAVE', 20, 3)",
            [seller_id.as_str()],
        )
        .await
        .expect("code");

    let denied_records = list_shops(
        StateExtractor(state.clone()),
        user_headers.clone(),
        QueryExtractor(RecordQuery {
            search: None,
            limit: None,
            offset: None,
        }),
    )
    .await;
    assert!(matches!(denied_records, Err(ApiError::Forbidden(_))));

    // Orphans are not rows the admin can act on, so they must not be counted.
    // This connection enforces foreign keys; turn them off only to create the
    // corrupt state the count must tolerate.
    records_db
        .execute("PRAGMA foreign_keys = OFF", ())
        .await
        .expect("disable foreign keys for fixture");
    for sql in [
        "INSERT INTO shops (id, user_id, name, description, currency, created_at) \
         VALUES ('orphan-shop', 'ghost', 'Ghost', '', 'sol', 4)",
        "INSERT INTO products (id, user_id, shop_id, name, details, category, \
         price_lamports, created_at) \
         VALUES ('orphan-product', 'ghost', 'shop-1', 'Ghost', '', '', 100, 5)",
        "INSERT INTO discount_codes (id, user_id, code, percent_off, created_at) \
         VALUES ('orphan-code', 'ghost', 'GHOST', 10, 6)",
    ] {
        records_db.execute(sql, ()).await.expect("orphan");
    }
    records_db
        .execute("PRAGMA foreign_keys = ON", ())
        .await
        .expect("restore foreign keys");

    let shops = list_shops(
        StateExtractor(state.clone()),
        admin_headers.clone(),
        QueryExtractor(RecordQuery {
            search: None,
            limit: None,
            offset: None,
        }),
    )
    .await
    .expect("list shops");
    assert_eq!(shops.0.total, 1, "orphaned shop must not inflate total");
    let products = list_products(
        StateExtractor(state.clone()),
        admin_headers.clone(),
        QueryExtractor(RecordQuery {
            search: None,
            limit: None,
            offset: None,
        }),
    )
    .await
    .expect("list products");
    assert_eq!(
        products.0.total, 1,
        "orphaned product must not inflate total"
    );
    // The price is in the shop's currency, so the row says which one.
    assert_eq!(products.0.items[0].currency, "sol");
    assert_eq!(products.0.items[0].price_lamports, 12_000_000);
    let codes = list_codes(
        StateExtractor(state.clone()),
        admin_headers.clone(),
        QueryExtractor(RecordQuery {
            search: None,
            limit: None,
            offset: None,
        }),
    )
    .await
    .expect("list codes");
    assert_eq!(codes.0.total, 1, "orphaned code must not inflate total");

    // A PATCH that omits optional fields keeps what is already stored.
    let shop_patch: ShopEdit =
        serde_json::from_value(serde_json::json!({ "name": "Corner Shop Renamed" }))
            .expect("shop patch");
    let _ = update_shop(
        StateExtractor(state.clone()),
        admin_headers.clone(),
        Path("shop-1".into()),
        JsonExtractor(shop_patch),
    )
    .await
    .expect("update shop");
    assert_eq!(
        text_field(&state, "SELECT description FROM shops WHERE id = 'shop-1'").await,
        "Flat whites",
        "omitted description must survive a partial update"
    );

    let product_patch: ProductEdit = serde_json::from_value(serde_json::json!({
        "name": "Flat white large",
        "price_lamports": 13_000_000
    }))
    .expect("product patch");
    let _ = update_product(
        StateExtractor(state.clone()),
        admin_headers.clone(),
        Path("product-1".into()),
        JsonExtractor(product_patch),
    )
    .await
    .expect("update product");
    assert_eq!(
        text_field(
            &state,
            "SELECT details FROM products WHERE id = 'product-1'"
        )
        .await,
        "Warm",
        "omitted details must survive a partial update"
    );
    assert_eq!(
        text_field(
            &state,
            "SELECT category FROM products WHERE id = 'product-1'"
        )
        .await,
        "coffee",
        "omitted category must survive a partial update"
    );

    // Pagination: pages walk the list without losing the total, and the
    // bounds are clamped rather than trusted.
    let page0 = list_users(
        StateExtractor(state.clone()),
        admin_headers.clone(),
        QueryExtractor(UserListQuery {
            search: None,
            limit: Some(1),
            offset: Some(0),
        }),
    )
    .await
    .expect("page 0");
    assert_eq!(page0.0.items.len(), 1);
    assert_eq!(page0.0.total, 2);
    let page1 = list_users(
        StateExtractor(state.clone()),
        admin_headers.clone(),
        QueryExtractor(UserListQuery {
            search: None,
            limit: Some(1),
            offset: Some(1),
        }),
    )
    .await
    .expect("page 1");
    assert_eq!(page1.0.items.len(), 1);
    assert_eq!(page1.0.total, 2);
    let clamped = list_users(
        StateExtractor(state.clone()),
        admin_headers.clone(),
        QueryExtractor(UserListQuery {
            search: None,
            limit: Some(9_999),
            offset: Some(-4),
        }),
    )
    .await
    .expect("clamped");
    assert_eq!(clamped.0.limit, MAX_PAGE);
    assert_eq!(clamped.0.offset, 0);

    // The feed: the chain check is refused before any RPC is attempted,
    // and its pure part orders and caps the records correctly.
    let anonymous_feed = list_payments(
        StateExtractor(state.clone()),
        HeaderMap::new(),
        QueryExtractor(PaymentListQuery { limit: None }),
    )
    .await;
    assert!(matches!(anonymous_feed, Err(ApiError::Unauthorized)));
    let denied_feed = list_payments(
        StateExtractor(state.clone()),
        user_headers.clone(),
        QueryExtractor(PaymentListQuery { limit: None }),
    )
    .await;
    assert!(matches!(denied_feed, Err(ApiError::Forbidden(_))));
    let (items, total) = shape_feed(
        vec![
            payment_fixture("aaa", 100, 0),
            payment_fixture("bbb", 300, 400),
            payment_fixture("ccc", 200, 0),
        ],
        2,
    );
    assert_eq!(total, 3, "total counts every invoice, not just the page");
    assert_eq!(items[0].id, "bbb", "newest first");
    assert_eq!(items[1].id, "ccc");
    assert_eq!(items[0].status, PaymentStatus::Settled);
    assert_eq!(items[0].settled_by.as_deref(), Some("Payer111111"));
    assert!(items[0].memo == "memo for bbb");

    // A non-admin cannot list anything.
    let denied_list = list_users(
        StateExtractor(state.clone()),
        user_headers.clone(),
        QueryExtractor(UserListQuery {
            search: None,
            limit: None,
            offset: None,
        }),
    )
    .await;
    assert!(matches!(denied_list, Err(ApiError::Forbidden(_))));

    // Everything scoped to the account is removed too, even though SQLite's
    // foreign keys are not guaranteed to be on for every connection.
    for sql in [
        "INSERT INTO discount_redemptions (id, user_id, code, buyer, invoice, created_at) \
         VALUES ('redemption-1', ?1, 'SAVE', 'buyer-1', 'invoice-1', 7)",
        "INSERT INTO shop_categories (user_id, shop_id, name, created_at) \
         VALUES (?1, 'shop-1', 'coffee', 7)",
        "INSERT INTO subscriptions (id, user_id, currency, amount, payment_id, paid_at, expires_at) \
         VALUES ('subscription-1', ?1, 'SOL', 100, 'invoice-2', 7, 8)",
        "INSERT INTO subscription_invoices (payment_id, user_id, currency, amount, created_at) \
         VALUES ('invoice-2', ?1, 'SOL', 100, 7)",
        "INSERT INTO push_subscriptions (endpoint, user_id, p256dh, auth, locale, created_at) \
         VALUES ('endpoint-1', ?1, 'p256dh', 'auth', 'en', 7)",
        "INSERT INTO push_nudges (user_id, month_index, sent_at) VALUES (?1, 7, 7)",
        "INSERT INTO faucet_claims (user_id, wallet, cluster, sol_lamports, token_base_units, \
         signature, claimed_at) VALUES (?1, 'wallet-1', 'devnet', 1, 1, 'sig-1', 7)",
        "INSERT INTO sponsored_settlements (id, wallet, user_id, cluster, amount, created_at) \
         VALUES ('settle-1', 'wallet-1', ?1, 'devnet', 1, 7)",
        "INSERT INTO invoice_meta (id, seller, reference, cluster, endpoint, currency, mint, \
         amount, memo, seller_user, name, description, items, created_at) \
         VALUES ('meta-1', 'seller-1', '', 'devnet', '', 'SOL', '', 1, '', ?1, 'n', 'd', '', 7)",
    ] {
        records_db.execute(sql, [seller_id.as_str()]).await.expect("child row");
    }

    // Deleting the pleb removes the account and its session.
    let pleb_id = found.0.items[0].id.clone();
    let _ = delete_user(
        StateExtractor(state.clone()),
        admin_headers.clone(),
        Path(pleb_id.clone()),
    )
    .await
    .expect("delete");
    let cleanup_db = state.db.connect().expect("connect");
    for table in [
        "sessions",
        "discount_codes",
        "password_resets",
        "saved_payments",
        "products",
        "shops",
        "shop_categories",
        "discount_redemptions",
        "subscriptions",
        "subscription_invoices",
        "push_subscriptions",
        "push_nudges",
        "faucet_claims",
        "sponsored_settlements",
    ] {
        let sql = format!("SELECT COUNT(*) FROM {table} WHERE user_id = ?1");
        let count = scalar(&cleanup_db, &sql).await.expect("count");
        assert_eq!(count, 0, "{table} still has rows for the deleted user");
    }
    let invoice_rows = scalar(
        &cleanup_db,
        "SELECT COUNT(*) FROM invoice_meta WHERE seller_user = ?1",
    )
    .await
    .expect("count");
    assert_eq!(invoice_rows, 0, "invoice_meta still names the deleted user");

    let after = list_users(
        StateExtractor(state.clone()),
        admin_headers.clone(),
        QueryExtractor(UserListQuery {
            search: None,
            limit: None,
            offset: None,
        }),
    )
    .await
    .expect("list");
    assert_eq!(after.0.items.len(), 1);
    assert_eq!(after.0.total, 1);
    assert_eq!(stats_after_delete(&state).await, 1);

    // Unknown id: 404. Own account: refused.
    let missing = delete_user(
        StateExtractor(state.clone()),
        admin_headers.clone(),
        Path("no-such-user".into()),
    )
    .await;
    assert!(matches!(missing, Err(ApiError::NotFound(_))));
    let boss_id = after.0.items[0].id.clone();
    let self_delete = delete_user(
        StateExtractor(state.clone()),
        admin_headers.clone(),
        Path(boss_id),
    )
    .await;
    assert!(matches!(self_delete, Err(ApiError::BadRequest(_))));

    // Forgot-password bookkeeping is removed together with the account.
    let _ = crate::auth::signup(
        StateExtractor(state.clone()),
        JsonExtractor(SignupRequest {
            email: "temp@example.com".into(),
            password: "correct horse battery staple".into(),
        }),
    )
    .await;
    let _ = crate::auth::forgot_password(
        StateExtractor(state.clone()),
        JsonExtractor(ForgotPasswordRequest {
            email: "temp@example.com".into(),
        }),
    )
    .await;
    assert!(
        scalar(
            &state.db.connect().expect("connect"),
            "SELECT COUNT(*) FROM password_resets"
        )
        .await
        .expect("count")
            >= 1
    );

    // The faucet's claims: newest first, with what reached the cluster summed
    // and a claim still in flight listed but not counted.
    let connection = state.db.connect().expect("connect");
    let boss_row = text_field(
        &state,
        "SELECT id FROM users WHERE email = 'boss@example.com'",
    )
    .await;
    let temp_row = text_field(
        &state,
        "SELECT id FROM users WHERE email = 'temp@example.com'",
    )
    .await;
    connection
        .execute(
            "INSERT INTO faucet_claims VALUES (?1, 'WalletA', 'devnet', 1000000000, 10000000, 'SigA', 100)",
            [boss_row.as_str()],
        )
        .await
        .expect("claim");
    connection
        .execute(
            "INSERT INTO faucet_claims VALUES (?1, 'WalletB', 'devnet', 1000000000, 10000000, '', 200)",
            [temp_row.as_str()],
        )
        .await
        .expect("pending claim");
    let drops = airdrops(
        StateExtractor(state.clone()),
        admin_headers.clone(),
        QueryExtractor(AirdropQuery {
            limit: None,
            offset: None,
        }),
    )
    .await
    .expect("airdrops")
    .0;
    assert_eq!(drops.total, 2);
    assert_eq!(drops.limit, DEFAULT_AIRDROP_PAGE);
    assert_eq!(drops.items[0].email, "temp@example.com");
    assert_eq!(drops.items[0].signature, "");
    assert_eq!(drops.items[1].wallet, "WalletA");
    assert_eq!(drops.total_sol_lamports, 1_000_000_000);
    assert_eq!(drops.total_token_base_units, 10_000_000);
    let paged = airdrops(
        StateExtractor(state.clone()),
        admin_headers.clone(),
        QueryExtractor(AirdropQuery {
            limit: Some(1_000),
            offset: Some(1),
        }),
    )
    .await
    .expect("paged airdrops")
    .0;
    assert_eq!(paged.limit, MAX_AIRDROP_PAGE);
    assert_eq!(paged.items.len(), 1);
    assert_eq!(paged.items[0].signature, "SigA");
    // The earlier non-admin account is gone by now, so a fresh one asks.
    let outsider = bearer(&signup(&state, "outsider@example.com").await);
    let refused = airdrops(
        StateExtractor(state.clone()),
        outsider,
        QueryExtractor(AirdropQuery {
            limit: None,
            offset: None,
        }),
    )
    .await;
    assert!(matches!(refused, Err(ApiError::Forbidden(_))));

    std::env::remove_var("ADMIN_EMAILS");
    let _ = std::fs::remove_file(path);
}

/// A payment account as the chain would hand it over.
fn payment_fixture(id: &str, created: i64, settled: i64) -> PaymentRecord {
    PaymentRecord {
        id: id.to_string(),
        status: if settled > 0 {
            PaymentStatus::Settled
        } else {
            PaymentStatus::Pending
        },
        payment_pda: id.to_string(),
        seller: "Seller111111".into(),
        seller_user: None,
        reference: "Reference111".into(),
        amount_lamports: 1_000_000,
        options: Vec::new(),
        memo: format!("memo for {id}"),
        name: String::new(),
        description: String::new(),
        items: Vec::new(),
        discount: None,
        allow_discounts: false,
        cluster: "devnet".into(),
        endpoint: "https://api.devnet.solana.com".into(),
        settled_by: (settled > 0).then(|| "Payer111111".into()),
        created_at_unix: created,
        settled_at_unix: settled,
        fee: None,
    }
}

async fn stats_after_delete(state: &crate::AppState) -> i64 {
    scalar(
        &state.db.connect().expect("connect"),
        "SELECT COUNT(*) FROM users",
    )
    .await
    .expect("count")
}

async fn text_field(state: &crate::AppState, sql: &str) -> String {
    let connection = state.db.connect().expect("connect");
    let mut rows = connection.query(sql, ()).await.expect("query");
    rows.next()
        .await
        .expect("row")
        .expect("value")
        .get(0)
        .expect("text")
}
