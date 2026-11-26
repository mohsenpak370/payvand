//! Admin view and correction of products.

use axum::extract::{Json, Path, Query, State};
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};

use crate::auth;
use crate::payments::{ApiError, ApiResult};

use super::super::{first_count, require_admin, scalar};
use super::{optional, record_page, record_search, required, RecordQuery};

#[derive(Debug, Serialize)]
pub struct AdminProduct {
    pub id: String,
    pub shop: String,
    pub owner: String,
    pub name: String,
    pub details: String,
    pub category: String,
    /// In the shop's currency's base units.
    pub price_lamports: u64,
    pub created_at: i64,
}

#[derive(Debug, Serialize)]
pub struct AdminProductPage {
    pub items: Vec<AdminProduct>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

pub async fn list_products(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Query(query): Query<RecordQuery>,
) -> ApiResult<Json<AdminProductPage>> {
    require_admin(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;
    let (limit, offset) = record_page(&query);
    let pattern = record_search(&query);

    let total = match &pattern {
        Some(pattern) => {
            let mut rows = connection
                .query(
                    "SELECT COUNT(*) FROM products p JOIN shops s ON s.id = p.shop_id \
                     JOIN users u ON u.id = p.user_id \
                     WHERE p.name LIKE ?1 OR s.name LIKE ?1 OR u.email LIKE ?1",
                    [pattern.as_str()],
                )
                .await
                .map_err(auth::internal)?;
            first_count(&mut rows).await?
        }
        None => scalar(&connection, "SELECT COUNT(*) FROM products").await?,
    };

    let select = "SELECT p.id, s.name, u.email, p.name, p.details, p.category, \
        p.price_lamports, p.created_at \
        FROM products p JOIN shops s ON s.id = p.shop_id JOIN users u ON u.id = p.user_id";
    let mut rows = match &pattern {
        Some(pattern) => {
            let sql = format!(
                "{select} WHERE p.name LIKE ?1 OR s.name LIKE ?1 OR u.email LIKE ?1 \
                 ORDER BY p.created_at DESC, p.id ASC LIMIT ?2 OFFSET ?3"
            );
            connection
                .query(&sql, (pattern.as_str(), limit, offset))
                .await
                .map_err(auth::internal)?
        }
        None => {
            let sql = format!("{select} ORDER BY p.created_at DESC, p.id ASC LIMIT ?1 OFFSET ?2");
            connection
                .query(&sql, (limit, offset))
                .await
                .map_err(auth::internal)?
        }
    };

    let mut items = Vec::new();
    while let Some(row) = rows.next().await.map_err(auth::internal)? {
        let price: i64 = row.get(6).map_err(auth::internal)?;
        items.push(AdminProduct {
            id: row.get(0).map_err(auth::internal)?,
            shop: row.get(1).map_err(auth::internal)?,
            owner: row.get(2).map_err(auth::internal)?,
            name: row.get(3).map_err(auth::internal)?,
            details: row.get(4).map_err(auth::internal)?,
            category: row.get(5).map_err(auth::internal)?,
            price_lamports: u64::try_from(price)
                .map_err(|_| ApiError::Internal("stored price out of range".into()))?,
            created_at: row.get(7).map_err(auth::internal)?,
        });
    }

    Ok(Json(AdminProductPage {
        items,
        total,
        limit,
        offset,
    }))
}

#[derive(Debug, Deserialize)]
pub struct ProductEdit {
    pub name: String,
    #[serde(default)]
    pub details: String,
    #[serde(default)]
    pub category: String,
    pub price_lamports: u64,
}

pub async fn update_product(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<ProductEdit>,
) -> ApiResult<Json<serde_json::Value>> {
    require_admin(&state.db, &headers).await?;
    let name = required(
        &request.name,
        crate::shops::MAX_PRODUCT_NAME_LEN,
        "The product name",
    )?;
    let details = optional(
        &request.details,
        crate::shops::MAX_PRODUCT_DETAILS_LEN,
        "The product details",
    )?;
    let category = optional(
        &request.category,
        crate::shops::MAX_PRODUCT_CATEGORY_LEN,
        "The product category",
    )?;
    if request.price_lamports == 0 {
        return Err(ApiError::BadRequest(
            "A price must be greater than 0.".into(),
        ));
    }
    let price = i64::try_from(request.price_lamports)
        .map_err(|_| ApiError::BadRequest("That price is too large.".into()))?;
    let connection = state.db.connect().map_err(auth::internal)?;
    let updated = connection
        .execute(
            "UPDATE products SET name = ?1, details = ?2, category = ?3, price_lamports = ?4 \
             WHERE id = ?5",
            (name, details, category, price, id),
        )
        .await
        .map_err(auth::internal)?;
    if updated == 0 {
        return Err(ApiError::NotFound("No product with that id.".into()));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn delete_product(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    require_admin(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;
    let removed = connection
        .execute("DELETE FROM products WHERE id = ?1", [id.as_str()])
        .await
        .map_err(auth::internal)?;
    if removed == 0 {
        return Err(ApiError::NotFound("No product with that id.".into()));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}
