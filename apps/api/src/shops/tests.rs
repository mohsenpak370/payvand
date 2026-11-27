//! The routes end to end, against a migrated temporary database.

use super::*;
use crate::auth::{random_id, SignupRequest};
use axum::extract::{Json as JsonExtractor, Path, State as StateExtractor};
use axum::http::HeaderMap;

fn bearer(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {token}").parse().expect("header value"),
    );
    headers
}

async fn test_state() -> (crate::AppState, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!("payvand-shop-test-{}.db", random_id()));
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

async fn signup(state: &crate::AppState, email: &str) -> String {
    signup_with_id(state, email).await.0
}

/// The token and the account id: a shop mutation needs a subscription, and
/// that is keyed by the account.
async fn signup_with_id(state: &crate::AppState, email: &str) -> (String, String) {
    let token = crate::auth::signup(
        StateExtractor(state.clone()),
        JsonExtractor(SignupRequest {
            email: email.to_string(),
            password: "correct horse battery staple".into(),
        }),
    )
    .await
    .expect("signup")
    .0
    .token;
    let connection = state.db.connect().expect("connection");
    let mut rows = connection
        .query("SELECT id FROM users WHERE email = ?1", [email.to_string()])
        .await
        .expect("user read");
    let id: String = rows
        .next()
        .await
        .expect("row")
        .expect("user")
        .get(0)
        .expect("id");
    (token, id)
}

fn product(name: &str, price_lamports: u64) -> SaveProductRequest {
    SaveProductRequest {
        name: name.to_string(),
        details: String::new(),
        category: String::new(),
        price_lamports,
    }
}

#[tokio::test]
async fn keeps_several_shops_per_account_apart() {
    let (state, path) = test_state().await;
    let (mine_token, mine_id) = signup_with_id(&state, "seller@example.com").await;
    crate::subscription::activate_for_tests(&state.db, &mine_id).await;
    let mine = bearer(&mine_token);
    let theirs = bearer(&signup(&state, "other@example.com").await);

    let anonymous = list(StateExtractor(state.clone()), HeaderMap::new()).await;
    assert!(matches!(
        anonymous,
        Err(crate::payments::ApiError::Unauthorized)
    ));

    // No shops yet: an empty list, not an error.
    let empty = list(StateExtractor(state.clone()), mine.clone())
        .await
        .expect("read")
        .0;
    assert!(empty.is_empty());

    // Without a paid period the other account cannot open one, while the
    // subscribed seller can.
    let unsubscribed = create(
        StateExtractor(state.clone()),
        theirs.clone(),
        JsonExtractor(SaveShopRequest {
            name: "Corner Shop".into(),
            description: String::new(),
            currency: None,
        }),
    )
    .await;

    assert!(matches!(
        unsubscribed,
        Err(crate::payments::ApiError::Forbidden(_))
    ));

    let cafe = create(
        StateExtractor(state.clone()),
        mine.clone(),
        JsonExtractor(SaveShopRequest {
            name: "Corner Shop".into(),
            description: "Flat whites".into(),
            currency: None,
        }),
    )
    .await
    .expect("created")
    .0;
    assert_eq!(cafe.name, "Corner Shop");
    assert_eq!(cafe.currency, "sol");

    let bakery = create(
        StateExtractor(state.clone()),
        mine.clone(),
        JsonExtractor(SaveShopRequest {
            name: "Corner Bakery".into(),
            description: String::new(),
            currency: Some("eurc".into()),
        }),
    )
    .await
    .expect("second shop")
    .0;

    // Each shop keeps its own catalogue.
    let coffee = create_product(
        StateExtractor(state.clone()),
        mine.clone(),
        Path(cafe.id.clone()),
        JsonExtractor(product("Flat white", 12_000_000)),
    )
    .await
    .expect("product")
    .0;
    assert_eq!(coffee.shop_id, cafe.id);

    let croissant = create_product(
        StateExtractor(state.clone()),
        mine.clone(),
        Path(bakery.id.clone()),
        JsonExtractor(SaveProductRequest {
            category: "Pastry".into(),
            ..product("Croissant", 8_000_000)
        }),
    )
    .await
    .expect("bakery product")
    .0;
    assert_eq!(croissant.category, "Pastry");

    // The category is a heading, not a label: editing a product moves it.
    let moved = update_product(
        StateExtractor(state.clone()),
        mine.clone(),
        Path((bakery.id.clone(), croissant.id.clone())),
        JsonExtractor(SaveProductRequest {
            category: "Bread".into(),
            ..product("Croissant", 8_000_000)
        }),
    )
    .await
    .expect("re-categorised")
    .0;
    assert_eq!(moved.category, "Bread");

    let shops = list(StateExtractor(state.clone()), mine.clone())
        .await
        .expect("read")
        .0;
    assert_eq!(shops.len(), 2);
    let cafe_row = shops.iter().find(|shop| shop.id == cafe.id).expect("cafe");
    assert_eq!(cafe_row.products.len(), 1);
    assert_eq!(cafe_row.products[0].name, "Flat white");

    // Renaming one shop leaves the other alone.
    let _ = update(
        StateExtractor(state.clone()),
        mine.clone(),
        Path(cafe.id.clone()),
        JsonExtractor(SaveShopRequest {
            name: "Corner Shop & Café".into(),
            description: String::new(),
            currency: None,
        }),
    )
    .await
    .expect("renamed");
    let cafe_row = get(
        StateExtractor(state.clone()),
        mine.clone(),
        Path(cafe.id.clone()),
    )
    .await
    .expect("open")
    .0;
    assert_eq!(cafe_row.name, "Corner Shop & Café");
    assert_eq!(cafe_row.products.len(), 1);
    // The rename kept the currency, and the bakery kept the euro it was
    // opened with.
    assert_eq!(cafe_row.currency, "sol");
    let bakery_row = shops
        .iter()
        .find(|shop| shop.id == bakery.id)
        .expect("bakery");
    assert_eq!(bakery_row.currency, "eurc");

    // Another account sees no shops and cannot touch this one.
    let their_shops = list(StateExtractor(state.clone()), theirs.clone())
        .await
        .expect("read")
        .0;
    assert!(their_shops.is_empty());
    assert!(get(
        StateExtractor(state.clone()),
        theirs.clone(),
        Path(cafe.id.clone())
    )
    .await
    .is_err());
    assert!(create_product(
        StateExtractor(state.clone()),
        theirs.clone(),
        Path(cafe.id.clone()),
        JsonExtractor(product("Free coffee", 1)),
    )
    .await
    .is_err());

    // A product of one shop cannot be edited through another.
    assert!(update_product(
        StateExtractor(state.clone()),
        mine.clone(),
        Path((bakery.id.clone(), coffee.id.clone())),
        JsonExtractor(product("Flat white", 15_000_000)),
    )
    .await
    .is_err());

    let edited = update_product(
        StateExtractor(state.clone()),
        mine.clone(),
        Path((cafe.id.clone(), coffee.id.clone())),
        JsonExtractor(product("Flat white (large)", 15_000_000)),
    )
    .await
    .expect("edited")
    .0;
    assert_eq!(edited.price_lamports, 15_000_000);
    assert_eq!(edited.created_at_unix, coffee.created_at_unix);

    let _ = delete_product(
        StateExtractor(state.clone()),
        mine.clone(),
        Path((cafe.id.clone(), coffee.id.clone())),
    )
    .await
    .expect("deleted");
    let after = get(StateExtractor(state.clone()), mine, Path(cafe.id))
        .await
        .expect("read")
        .0;
    assert!(after.products.is_empty());

    let _ = std::fs::remove_file(path);
}
