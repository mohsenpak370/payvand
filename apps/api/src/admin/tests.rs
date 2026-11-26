use super::{
    delete_user,
    feed::shape_feed,
    is_admin, list_payments, list_users, require_admin, scalar, stats,
    users::{DEFAULT_PAGE, MAX_PAGE},
    PaymentListQuery, UserListQuery,
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

    // Deleting the pleb removes the account and its session.
    let pleb_id = found.0.items[0].id.clone();
    let _ = delete_user(
        StateExtractor(state.clone()),
        admin_headers.clone(),
        Path(pleb_id.clone()),
    )
    .await
    .expect("delete");
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
