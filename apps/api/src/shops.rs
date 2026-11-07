//! Shops and products.
//!
//! A seller keeps a list of shops on their account — a café and a bakery — and
//! each shop has its own catalogue. The payment page prices an invoice from a
//! cart of those products, and the receipt travels inside the QR code: the API
//! still stores no invoices, so a shop is only ever the menu the seller picks
//! from.
//!
//! Nothing here is on chain. A price is a number the seller edits, not state
//! anyone has to trust: the customer pays the amount the code says, at an
//! address the program derives from it.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::auth;
use crate::payments::{bad, ApiResult};

/// Enough for a real café, small enough that the list stays a list.
pub const MAX_PRODUCTS: usize = 60;
/// Several shops are useful; a hundred is a shop-keeping system.
pub const MAX_SHOPS: usize = 20;
pub const MAX_SHOP_NAME_LEN: usize = 48;
pub const MAX_SHOP_DESCRIPTION_LEN: usize = 160;
/// Product names ride in the receipt inside the code, so they are kept short:
/// a cart's worth of them still has to fit a scannable QR.
pub const MAX_PRODUCT_NAME_LEN: usize = 32;
pub const MAX_PRODUCT_DETAILS_LEN: usize = 160;
/// 10,000 SOL — a product priced above that is a typo, not a price.
pub const MAX_PRICE_LAMPORTS: u64 = 10_000_000_000_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shop {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: String,
    /// The shop this product is sold from.
    pub shop_id: String,
    pub name: String,
    /// Free text — what is in it, the size, the allergen. Never in the code.
    pub details: String,
    pub price_lamports: u64,
    pub created_at_unix: i64,
}

/// A shop with its catalogue. The profile lists several of these to count
/// products, and the payment page needs the products to price a cart.
#[derive(Debug, Serialize)]
pub struct ShopWithProducts {
    pub id: String,
    pub name: String,
    pub description: String,
    pub products: Vec<Product>,
}

#[derive(Debug, Deserialize)]
pub struct SaveShopRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct SaveProductRequest {
    pub name: String,
    #[serde(default)]
    pub details: String,
    pub price_lamports: u64,
}

/// Trimmed, required text with a length the storage and the QR can afford.
fn required_text(raw: &str, max: usize, field: &str) -> ApiResult<String> {
    let value = raw.trim().to_string();
    if value.is_empty() {
        return Err(bad(format!("{field} cannot be empty.")));
    }
    if value.chars().count() > max {
        return Err(bad(format!("{field} must be at most {max} characters.")));
    }
    Ok(value)
}

/// Trimmed, optional text with the same limit.
fn optional_text(raw: &str, max: usize, field: &str) -> ApiResult<String> {
    let value = raw.trim().to_string();
    if value.chars().count() > max {
        return Err(bad(format!("{field} must be at most {max} characters.")));
    }
    Ok(value)
}

/// A price of zero would create an invoice nobody can derive an address for,
/// so it is refused here rather than on the payment page.
fn valid_price(price_lamports: u64) -> ApiResult<u64> {
    if price_lamports == 0 {
        return Err(bad("A price must be greater than 0."));
    }
    if price_lamports > MAX_PRICE_LAMPORTS {
        return Err(bad(format!(
            "A price must be at most {} SOL.",
            MAX_PRICE_LAMPORTS / 1_000_000_000
        )));
    }
    Ok(price_lamports)
}

fn read_product(row: &libsql::Row) -> ApiResult<Product> {
    let price: i64 = row.get(4).map_err(auth::internal)?;
    Ok(Product {
        id: row.get(0).map_err(auth::internal)?,
        shop_id: row.get(1).map_err(auth::internal)?,
        name: row.get(2).map_err(auth::internal)?,
        details: row.get(3).map_err(auth::internal)?,
        price_lamports: u64::try_from(price)
            .map_err(|_| crate::payments::ApiError::Internal("stored price out of range".into()))?,
        created_at_unix: row.get(5).map_err(auth::internal)?,
    })
}

/// Refuses a shop id that is not this account's. Every product route goes
/// through it, so a shop id in a request can never reach another account.
async fn require_shop(
    connection: &libsql::Connection,
    user_id: &str,
    shop_id: &str,
) -> ApiResult<(String, String)> {
    let mut rows = connection
        .query(
            "SELECT name, description FROM shops WHERE id = ?1 AND user_id = ?2",
            (shop_id, user_id),
        )
        .await
        .map_err(auth::internal)?;
    match rows.next().await.map_err(auth::internal)? {
        Some(row) => Ok((
            row.get(0).map_err(auth::internal)?,
            row.get(1).map_err(auth::internal)?,
        )),
        None => Err(bad("That shop no longer exists.")),
    }
}

async fn products_for_shop(
    connection: &libsql::Connection,
    user_id: &str,
    shop_id: &str,
) -> ApiResult<Vec<Product>> {
    // Oldest first: a catalogue that keeps its order reads like a menu.
    let mut rows = connection
        .query(
            "SELECT id, shop_id, name, details, price_lamports, created_at FROM products \
             WHERE user_id = ?1 AND shop_id = ?2 ORDER BY created_at ASC, rowid ASC",
            (user_id, shop_id),
        )
        .await
        .map_err(auth::internal)?;

    let mut products = Vec::new();
    while let Some(row) = rows.next().await.map_err(auth::internal)? {
        products.push(read_product(&row)?);
    }
    Ok(products)
}

/// Every shop the signed-in account keeps, each with its catalogue. One read
/// serves both screens: the profile lists them, the payment page prices from
/// them.
pub async fn list(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<ShopWithProducts>>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;

    let mut rows = connection
        .query(
            "SELECT id, name, description FROM shops WHERE user_id = ?1 \
             ORDER BY created_at ASC, rowid ASC",
            [user.id.clone()],
        )
        .await
        .map_err(auth::internal)?;

    let mut shops: Vec<ShopWithProducts> = Vec::new();
    while let Some(row) = rows.next().await.map_err(auth::internal)? {
        shops.push(ShopWithProducts {
            id: row.get(0).map_err(auth::internal)?,
            name: row.get(1).map_err(auth::internal)?,
            description: row.get(2).map_err(auth::internal)?,
            products: Vec::new(),
        });
    }

    // One query for every product, grouped in memory: a seller has a handful
    // of shops, and this keeps the round trips to two.
    if !shops.is_empty() {
        let mut rows = connection
            .query(
                "SELECT id, shop_id, name, details, price_lamports, created_at FROM products \
                 WHERE user_id = ?1 ORDER BY created_at ASC, rowid ASC",
                [user.id],
            )
            .await
            .map_err(auth::internal)?;
        let mut by_shop: HashMap<String, Vec<Product>> = HashMap::new();
        while let Some(row) = rows.next().await.map_err(auth::internal)? {
            let product = read_product(&row)?;
            by_shop.entry(product.shop_id.clone()).or_default().push(product);
        }
        for shop in &mut shops {
            shop.products = by_shop.remove(&shop.id).unwrap_or_default();
        }
    }

    Ok(Json(shops))
}

/// Opens one shop with its catalogue — what the shop's own page renders.
pub async fn get(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<ShopWithProducts>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;
    let (name, description) = require_shop(&connection, &user.id, &id).await?;
    let products = products_for_shop(&connection, &user.id, &id).await?;
    Ok(Json(ShopWithProducts {
        id,
        name,
        description,
        products,
    }))
}

/// Creates a shop. Several per account are allowed; the profile lists them.
pub async fn create(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Json(request): Json<SaveShopRequest>,
) -> ApiResult<Json<Shop>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let name = required_text(&request.name, MAX_SHOP_NAME_LEN, "The shop name")?;
    let description = optional_text(
        &request.description,
        MAX_SHOP_DESCRIPTION_LEN,
        "The shop description",
    )?;
    let connection = state.db.connect().map_err(auth::internal)?;

    let mut rows = connection
        .query("SELECT COUNT(*) FROM shops WHERE user_id = ?1", [user.id.clone()])
        .await
        .map_err(auth::internal)?;
    let count: i64 = match rows.next().await.map_err(auth::internal)? {
        Some(row) => row.get(0).map_err(auth::internal)?,
        None => 0,
    };
    if count >= MAX_SHOPS as i64 {
        return Err(bad(format!(
            "You already keep the maximum of {MAX_SHOPS} shops."
        )));
    }

    let id = auth::random_id();
    connection
        .execute(
            "INSERT INTO shops (id, user_id, name, description, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (
                id.clone(),
                user.id,
                name.clone(),
                description.clone(),
                auth::unix_now(),
            ),
        )
        .await
        .map_err(auth::internal)?;

    Ok(Json(Shop {
        id,
        name,
        description,
    }))
}

/// Renames a shop or rewrites its description. Ownership is checked first.
pub async fn update(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<SaveShopRequest>,
) -> ApiResult<Json<Shop>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let name = required_text(&request.name, MAX_SHOP_NAME_LEN, "The shop name")?;
    let description = optional_text(
        &request.description,
        MAX_SHOP_DESCRIPTION_LEN,
        "The shop description",
    )?;
    let connection = state.db.connect().map_err(auth::internal)?;
    require_shop(&connection, &user.id, &id).await?;

    connection
        .execute(
            "UPDATE shops SET name = ?1, description = ?2 WHERE id = ?3 AND user_id = ?4",
            (
                name.clone(),
                description.clone(),
                id.clone(),
                user.id,
            ),
        )
        .await
        .map_err(auth::internal)?;

    Ok(Json(Shop {
        id,
        name,
        description,
    }))
}

/// Adds a product to one of the account's shops.
pub async fn create_product(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Path(shop_id): Path<String>,
    Json(request): Json<SaveProductRequest>,
) -> ApiResult<Json<Product>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let name = required_text(&request.name, MAX_PRODUCT_NAME_LEN, "The product name")?;
    let details = optional_text(
        &request.details,
        MAX_PRODUCT_DETAILS_LEN,
        "The product details",
    )?;
    let price_lamports = valid_price(request.price_lamports)?;
    let connection = state.db.connect().map_err(auth::internal)?;
    require_shop(&connection, &user.id, &shop_id).await?;

    let mut rows = connection
        .query(
            "SELECT COUNT(*) FROM products WHERE user_id = ?1 AND shop_id = ?2",
            (user.id.clone(), shop_id.clone()),
        )
        .await
        .map_err(auth::internal)?;
    let count: i64 = match rows.next().await.map_err(auth::internal)? {
        Some(row) => row.get(0).map_err(auth::internal)?,
        None => 0,
    };
    if count >= MAX_PRODUCTS as i64 {
        return Err(bad(format!(
            "This shop already has the maximum of {MAX_PRODUCTS} products."
        )));
    }

    let id = auth::random_id();
    let created_at = auth::unix_now();
    connection
        .execute(
            "INSERT INTO products (id, user_id, shop_id, name, details, price_lamports, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                id.clone(),
                user.id,
                shop_id.clone(),
                name.clone(),
                details.clone(),
                i64::try_from(price_lamports).map_err(|_| bad("That price is too large."))?,
                created_at,
            ),
        )
        .await
        .map_err(auth::internal)?;

    Ok(Json(Product {
        id,
        shop_id,
        name,
        details,
        price_lamports,
        created_at_unix: created_at,
    }))
}

/// Edits one product of one shop. The price is part of the edit: a shop can
/// change what something costs without recreating it.
pub async fn update_product(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Path((shop_id, product_id)): Path<(String, String)>,
    Json(request): Json<SaveProductRequest>,
) -> ApiResult<Json<Product>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let name = required_text(&request.name, MAX_PRODUCT_NAME_LEN, "The product name")?;
    let details = optional_text(
        &request.details,
        MAX_PRODUCT_DETAILS_LEN,
        "The product details",
    )?;
    let price_lamports = valid_price(request.price_lamports)?;
    let connection = state.db.connect().map_err(auth::internal)?;
    require_shop(&connection, &user.id, &shop_id).await?;

    let updated = connection
        .execute(
            "UPDATE products SET name = ?1, details = ?2, price_lamports = ?3 \
             WHERE id = ?4 AND shop_id = ?5 AND user_id = ?6",
            (
                name.clone(),
                details.clone(),
                i64::try_from(price_lamports).map_err(|_| bad("That price is too large."))?,
                product_id.clone(),
                shop_id.clone(),
                user.id.clone(),
            ),
        )
        .await
        .map_err(auth::internal)?;
    if updated == 0 {
        return Err(bad("That product no longer exists."));
    }

    let mut rows = connection
        .query(
            "SELECT id, shop_id, name, details, price_lamports, created_at FROM products \
             WHERE id = ?1 AND shop_id = ?2 AND user_id = ?3",
            (product_id, shop_id, user.id),
        )
        .await
        .map_err(auth::internal)?;
    match rows.next().await.map_err(auth::internal)? {
        Some(row) => Ok(Json(read_product(&row)?)),
        None => Err(bad("That product no longer exists.")),
    }
}

/// Removes a product. Invoices already created keep their receipt: it lives in
/// the buyer's code, not in this table.
pub async fn delete_product(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Path((shop_id, product_id)): Path<(String, String)>,
) -> ApiResult<Json<serde_json::Value>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;
    require_shop(&connection, &user.id, &shop_id).await?;
    let deleted = connection
        .execute(
            "DELETE FROM products WHERE id = ?1 AND shop_id = ?2 AND user_id = ?3",
            (product_id, shop_id, user.id),
        )
        .await
        .map_err(auth::internal)?;
    if deleted == 0 {
        return Err(bad("That product no longer exists."));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{random_id, SignupRequest};
    use axum::extract::{Json as JsonExtractor, State as StateExtractor};

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

    fn product(name: &str, price_lamports: u64) -> SaveProductRequest {
        SaveProductRequest {
            name: name.to_string(),
            details: String::new(),
            price_lamports,
        }
    }

    #[test]
    fn keeps_names_short_and_prices_positive() {
        assert_eq!(required_text("  Flat white ", 32, "Name").unwrap(), "Flat white");
        assert!(required_text("   ", 32, "Name").is_err());
        assert!(required_text(&"x".repeat(33), 32, "Name").is_err());
        // Counted in characters, so a short non-ASCII name is not rejected.
        assert!(required_text("Crème brûlée", 12, "Name").is_ok());
        assert!(valid_price(0).is_err());
        assert!(valid_price(1).is_ok());
        assert!(valid_price(MAX_PRICE_LAMPORTS + 1).is_err());
    }

    #[tokio::test]
    async fn keeps_several_shops_per_account_apart() {
        let (state, path) = test_state().await;
        let mine = bearer(&signup(&state, "seller@example.com").await);
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

        let cafe = create(
            StateExtractor(state.clone()),
            mine.clone(),
            JsonExtractor(SaveShopRequest {
                name: "Corner Shop".into(),
                description: "Flat whites".into(),
            }),
        )
        .await
        .expect("created")
        .0;
        assert_eq!(cafe.name, "Corner Shop");

        let bakery = create(
            StateExtractor(state.clone()),
            mine.clone(),
            JsonExtractor(SaveShopRequest {
                name: "Corner Bakery".into(),
                description: String::new(),
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

        create_product(
            StateExtractor(state.clone()),
            mine.clone(),
            Path(bakery.id.clone()),
            JsonExtractor(product("Croissant", 8_000_000)),
        )
        .await
        .expect("bakery product");

        let shops = list(StateExtractor(state.clone()), mine.clone())
            .await
            .expect("read")
            .0;
        assert_eq!(shops.len(), 2);
        let cafe_row = shops.iter().find(|shop| shop.id == cafe.id).expect("cafe");
        assert_eq!(cafe_row.products.len(), 1);
        assert_eq!(cafe_row.products[0].name, "Flat white");

        // Renaming one shop leaves the other alone.
        update(
            StateExtractor(state.clone()),
            mine.clone(),
            Path(cafe.id.clone()),
            JsonExtractor(SaveShopRequest {
                name: "Corner Shop & Café".into(),
                description: String::new(),
            }),
        )
        .await
        .expect("renamed");
        let cafe_row = get(StateExtractor(state.clone()), mine.clone(), Path(cafe.id.clone()))
            .await
            .expect("open")
            .0;
        assert_eq!(cafe_row.name, "Corner Shop & Café");
        assert_eq!(cafe_row.products.len(), 1);

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

        delete_product(
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
}
