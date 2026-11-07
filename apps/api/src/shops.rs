//! Shops and products.
//!
//! A seller describes their shop once and keeps a short catalogue on their
//! account. The payment page then prices an invoice from a cart of those
//! products, and the receipt travels inside the QR code — the API still stores
//! no invoices, so a shop is only ever the menu the seller picks from.
//!
//! Nothing here is on chain. A price is a number the seller edits, not state
//! anyone has to trust: the customer pays the amount the code says, at an
//! address the program derives from it.

use axum::extract::{Json, Path, State};
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};

use crate::auth;
use crate::payments::{bad, ApiResult};

/// Enough for a real café, small enough that the list stays a list.
pub const MAX_PRODUCTS: usize = 60;
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
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: String,
    pub name: String,
    /// Free text — what is in it, the size, the allergen. Never in the code.
    pub details: String,
    pub price_lamports: u64,
    pub created_at_unix: i64,
}

/// The shop and its catalogue, in one read: the profile edits them together
/// and the payment page needs both to price a cart.
#[derive(Debug, Serialize)]
pub struct ShopResponse {
    pub shop: Option<Shop>,
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
    let price: i64 = row.get(3).map_err(auth::internal)?;
    Ok(Product {
        id: row.get(0).map_err(auth::internal)?,
        name: row.get(1).map_err(auth::internal)?,
        details: row.get(2).map_err(auth::internal)?,
        price_lamports: u64::try_from(price)
            .map_err(|_| crate::payments::ApiError::Internal("stored price out of range".into()))?,
        created_at_unix: row.get(4).map_err(auth::internal)?,
    })
}

/// The signed-in account's shop and catalogue. A shop is one row per account,
/// so there is no id in the path and no way to read someone else's.
pub async fn get(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<ShopResponse>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;

    let mut shop_rows = connection
        .query(
            "SELECT name, description FROM shops WHERE user_id = ?1",
            [user.id.clone()],
        )
        .await
        .map_err(auth::internal)?;
    let shop = match shop_rows.next().await.map_err(auth::internal)? {
        Some(row) => Some(Shop {
            name: row.get(0).map_err(auth::internal)?,
            description: row.get(1).map_err(auth::internal)?,
        }),
        None => None,
    };

    // Oldest first: a catalogue that keeps its order reads like a menu.
    let mut rows = connection
        .query(
            "SELECT id, name, details, price_lamports, created_at FROM products \
             WHERE user_id = ?1 ORDER BY created_at ASC, rowid ASC",
            [user.id],
        )
        .await
        .map_err(auth::internal)?;

    let mut products = Vec::new();
    while let Some(row) = rows.next().await.map_err(auth::internal)? {
        products.push(read_product(&row)?);
    }

    Ok(Json(ShopResponse { shop, products }))
}

/// Creates the shop, or renames the one this account already has. Idempotent
/// by design: the profile form saves the same row every time.
pub async fn save(
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

    connection
        .execute(
            "INSERT INTO shops (id, user_id, name, description, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT(user_id) DO UPDATE SET \
             name = excluded.name, description = excluded.description",
            (
                auth::random_id(),
                user.id,
                name.clone(),
                description.clone(),
                auth::unix_now(),
            ),
        )
        .await
        .map_err(auth::internal)?;

    Ok(Json(Shop { name, description }))
}

/// Adds a product to the signed-in account's catalogue.
pub async fn create_product(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
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

    let mut rows = connection
        .query(
            "SELECT COUNT(*) FROM products WHERE user_id = ?1",
            [user.id.clone()],
        )
        .await
        .map_err(auth::internal)?;
    let count: i64 = match rows.next().await.map_err(auth::internal)? {
        Some(row) => row.get(0).map_err(auth::internal)?,
        None => 0,
    };
    if count >= MAX_PRODUCTS as i64 {
        return Err(bad(format!(
            "Your shop already has the maximum of {MAX_PRODUCTS} products."
        )));
    }

    let id = auth::random_id();
    let created_at = auth::unix_now();
    connection
        .execute(
            "INSERT INTO products (id, user_id, name, details, price_lamports, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                id.clone(),
                user.id,
                name.clone(),
                details.clone(),
                i64::try_from(price_lamports)
                    .map_err(|_| bad("That price is too large."))?,
                created_at,
            ),
        )
        .await
        .map_err(auth::internal)?;

    Ok(Json(Product {
        id,
        name,
        details,
        price_lamports,
        created_at_unix: created_at,
    }))
}

/// Edits one of the signed-in account's products. The price is part of the
/// edit: a shop can change what something costs without recreating it.
pub async fn update_product(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
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

    let updated = connection
        .execute(
            "UPDATE products SET name = ?1, details = ?2, price_lamports = ?3 \
             WHERE id = ?4 AND user_id = ?5",
            (
                name.clone(),
                details.clone(),
                i64::try_from(price_lamports).map_err(|_| bad("That price is too large."))?,
                id.clone(),
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
            "SELECT id, name, details, price_lamports, created_at FROM products \
             WHERE id = ?1 AND user_id = ?2",
            (id, user.id),
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
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;
    let deleted = connection
        .execute(
            "DELETE FROM products WHERE id = ?1 AND user_id = ?2",
            (id, user.id),
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
    async fn keeps_a_shop_and_its_products_per_account() {
        let (state, path) = test_state().await;
        let mine = bearer(&signup(&state, "seller@example.com").await);
        let theirs = bearer(&signup(&state, "other@example.com").await);

        let anonymous = get(StateExtractor(state.clone()), HeaderMap::new()).await;
        assert!(matches!(
            anonymous,
            Err(crate::payments::ApiError::Unauthorized)
        ));

        // No shop yet: an empty menu, not an error.
        let empty = get(StateExtractor(state.clone()), mine.clone())
            .await
            .expect("read")
            .0;
        assert!(empty.shop.is_none());
        assert!(empty.products.is_empty());

        let saved = save(
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
        assert_eq!(saved.name, "Corner Shop");

        // Saving again renames the same shop instead of creating a second one.
        save(
            StateExtractor(state.clone()),
            mine.clone(),
            JsonExtractor(SaveShopRequest {
                name: "Corner Shop & Bakery".into(),
                description: String::new(),
            }),
        )
        .await
        .expect("renamed");

        let added = create_product(
            StateExtractor(state.clone()),
            mine.clone(),
            JsonExtractor(product("Flat white", 12_000_000)),
        )
        .await
        .expect("product")
        .0;
        assert_eq!(added.name, "Flat white");

        let edited = update_product(
            StateExtractor(state.clone()),
            mine.clone(),
            axum::extract::Path(added.id.clone()),
            JsonExtractor(product("Flat white (large)", 15_000_000)),
        )
        .await
        .expect("edited")
        .0;
        assert_eq!(edited.price_lamports, 15_000_000);
        assert_eq!(edited.created_at_unix, added.created_at_unix);

        let mine_shop = get(StateExtractor(state.clone()), mine.clone())
            .await
            .expect("read")
            .0;
        assert_eq!(
            mine_shop.shop.as_ref().map(|shop| shop.name.as_str()),
            Some("Corner Shop & Bakery")
        );
        assert_eq!(mine_shop.products.len(), 1);

        // Another account sees no shop and cannot edit this product.
        let theirs_shop = get(StateExtractor(state.clone()), theirs.clone())
            .await
            .expect("read")
            .0;
        assert!(theirs_shop.shop.is_none());
        assert!(theirs_shop.products.is_empty());

        let stolen = update_product(
            StateExtractor(state.clone()),
            theirs.clone(),
            axum::extract::Path(added.id.clone()),
            JsonExtractor(product("Free coffee", 1)),
        )
        .await;
        assert!(stolen.is_err());

        delete_product(
            StateExtractor(state.clone()),
            mine.clone(),
            axum::extract::Path(added.id),
        )
        .await
        .expect("deleted");
        let after = get(StateExtractor(state.clone()), mine)
            .await
            .expect("read")
            .0;
        assert!(after.products.is_empty());

        let _ = std::fs::remove_file(path);
    }
}
