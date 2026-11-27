//! The `/api/shops` and `/api/shops/{id}` routes: the shops themselves.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use std::collections::HashMap;

use crate::auth;
use crate::payments::{bad, ApiResult};

use super::model::{
    Product, SaveShopRequest, Shop, ShopWithProducts, MAX_SHOPS, MAX_SHOP_DESCRIPTION_LEN,
    MAX_SHOP_NAME_LEN,
};
use super::store::{products_for_shop, read_product, require_shop};
use super::validate::{optional_text, required_text, valid_shop_currency};

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
            "SELECT id, name, description, currency FROM shops WHERE user_id = ?1 \
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
            currency: row.get(3).map_err(auth::internal)?,
            products: Vec::new(),
        });
    }

    // One query for every product, grouped in memory: a seller has a handful
    // of shops, and this keeps the round trips to two.
    if !shops.is_empty() {
        let mut rows = connection
            .query(
                "SELECT id, shop_id, name, details, category, price_lamports, created_at FROM products \
                 WHERE user_id = ?1 ORDER BY created_at ASC, rowid ASC",
                [user.id],
            )
            .await
            .map_err(auth::internal)?;
        let mut by_shop: HashMap<String, Vec<Product>> = HashMap::new();
        while let Some(row) = rows.next().await.map_err(auth::internal)? {
            let product = read_product(&row)?;
            by_shop
                .entry(product.shop_id.clone())
                .or_default()
                .push(product);
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
    let (name, description, currency) = require_shop(&connection, &user.id, &id).await?;
    let products = products_for_shop(&connection, &user.id, &id).await?;
    Ok(Json(ShopWithProducts {
        id,
        name,
        description,
        currency,
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
    let currency = valid_shop_currency(request.currency.as_deref())?;
    crate::subscription::require_active(&state, &user.id).await?;
    let connection = state.db.connect().map_err(auth::internal)?;

    let mut rows = connection
        .query(
            "SELECT COUNT(*) FROM shops WHERE user_id = ?1",
            [user.id.clone()],
        )
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
            "INSERT INTO shops (id, user_id, name, description, currency, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                id.clone(),
                user.id,
                name.clone(),
                description.clone(),
                currency.clone(),
                auth::unix_now(),
            ),
        )
        .await
        .map_err(auth::internal)?;

    Ok(Json(Shop {
        id,
        name,
        description,
        currency,
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
    let (_, _, current_currency) = require_shop(&connection, &user.id, &id).await?;
    crate::subscription::require_active(&state, &user.id).await?;
    // A rename leaves the currency alone; a client that sends one is changing
    // what the menu charges, and its prices are converted before it gets here.
    let currency = match request.currency.as_deref() {
        Some(raw) => valid_shop_currency(Some(raw))?,
        None => current_currency,
    };

    connection
        .execute(
            "UPDATE shops SET name = ?1, description = ?2, currency = ?3 \
             WHERE id = ?4 AND user_id = ?5",
            (
                name.clone(),
                description.clone(),
                currency.clone(),
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
        currency,
    }))
}
