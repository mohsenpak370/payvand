//! The `/api/shops/{id}/products` routes: one shop's catalogue.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;

use crate::auth;
use crate::payments::{bad, ApiResult};

use super::category::remember_category;
use super::model::{
    Product, SaveProductRequest, MAX_PRODUCTS, MAX_PRODUCT_CATEGORY_LEN, MAX_PRODUCT_DETAILS_LEN,
    MAX_PRODUCT_NAME_LEN,
};
use super::store::{read_product, require_shop};
use super::validate::{optional_text, required_text, valid_price};

/// Adds a product to one shop, up to the catalogue limit.
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
    let category = optional_text(
        &request.category,
        MAX_PRODUCT_CATEGORY_LEN,
        "The product category",
    )?;
    let price_lamports = valid_price(request.price_lamports)?;
    let connection = state.db.connect().map_err(auth::internal)?;
    require_shop(&connection, &user.id, &shop_id).await?;
    crate::subscription::require_active(&state, &user.id).await?;
    remember_category(&state.db, &user.id, &shop_id, &category).await?;

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
            "INSERT INTO products \
                (id, user_id, shop_id, name, details, category, price_lamports, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            (
                id.clone(),
                user.id,
                shop_id.clone(),
                name.clone(),
                details.clone(),
                category.clone(),
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
        category,
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
    let category = optional_text(
        &request.category,
        MAX_PRODUCT_CATEGORY_LEN,
        "The product category",
    )?;
    let price_lamports = valid_price(request.price_lamports)?;
    let connection = state.db.connect().map_err(auth::internal)?;
    require_shop(&connection, &user.id, &shop_id).await?;
    crate::subscription::require_active(&state, &user.id).await?;
    remember_category(&state.db, &user.id, &shop_id, &category).await?;

    let updated = connection
        .execute(
            "UPDATE products SET name = ?1, details = ?2, category = ?3, price_lamports = ?4 \
             WHERE id = ?5 AND shop_id = ?6 AND user_id = ?7",
            (
                name.clone(),
                details.clone(),
                category.clone(),
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
            "SELECT id, shop_id, name, details, category, price_lamports, created_at FROM products \
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
    crate::subscription::require_active(&state, &user.id).await?;
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
