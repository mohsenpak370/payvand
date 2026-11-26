//! Admin view and correction of shops.

use axum::extract::{Json, Path, Query, State};
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};

use crate::auth;
use crate::payments::{ApiError, ApiResult};

use super::super::{first_count, require_admin, scalar};
use super::{optional, record_page, record_search, required, RecordQuery};

#[derive(Debug, Serialize)]
pub struct AdminShop {
    pub id: String,
    pub owner: String,
    pub name: String,
    pub description: String,
    /// What its menu charges in — shown, not editable here: changing it means
    /// converting every price, which the seller's own editor does.
    pub currency: String,
    pub products: i64,
    pub created_at: i64,
}

#[derive(Debug, Serialize)]
pub struct AdminShopPage {
    pub items: Vec<AdminShop>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

pub async fn list_shops(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Query(query): Query<RecordQuery>,
) -> ApiResult<Json<AdminShopPage>> {
    require_admin(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;
    let (limit, offset) = record_page(&query);
    let pattern = record_search(&query);

    let total = match &pattern {
        Some(pattern) => {
            let mut rows = connection
                .query(
                    "SELECT COUNT(*) FROM shops s JOIN users u ON u.id = s.user_id \
                     WHERE s.name LIKE ?1 OR u.email LIKE ?1",
                    [pattern.as_str()],
                )
                .await
                .map_err(auth::internal)?;
            first_count(&mut rows).await?
        }
        // Count the same rows the page can show, not every row in the base
        // table: a shop whose account is gone is not an admin record.
        None => {
            scalar(
                &connection,
                "SELECT COUNT(*) FROM shops s JOIN users u ON u.id = s.user_id",
            )
            .await?
        }
    };

    let select = "SELECT s.id, u.email, s.name, s.description, s.currency, \
        (SELECT COUNT(*) FROM products p WHERE p.shop_id = s.id), s.created_at \
        FROM shops s JOIN users u ON u.id = s.user_id";
    let mut rows = match &pattern {
        Some(pattern) => {
            let sql = format!(
                "{select} WHERE s.name LIKE ?1 OR u.email LIKE ?1 \
                 ORDER BY s.created_at DESC, s.id ASC LIMIT ?2 OFFSET ?3"
            );
            connection
                .query(&sql, (pattern.as_str(), limit, offset))
                .await
                .map_err(auth::internal)?
        }
        None => {
            let sql = format!("{select} ORDER BY s.created_at DESC, s.id ASC LIMIT ?1 OFFSET ?2");
            connection
                .query(&sql, (limit, offset))
                .await
                .map_err(auth::internal)?
        }
    };

    let mut items = Vec::new();
    while let Some(row) = rows.next().await.map_err(auth::internal)? {
        items.push(AdminShop {
            id: row.get(0).map_err(auth::internal)?,
            owner: row.get(1).map_err(auth::internal)?,
            name: row.get(2).map_err(auth::internal)?,
            description: row.get(3).map_err(auth::internal)?,
            currency: row.get(4).map_err(auth::internal)?,
            products: row.get(5).map_err(auth::internal)?,
            created_at: row.get(6).map_err(auth::internal)?,
        });
    }

    Ok(Json(AdminShopPage {
        items,
        total,
        limit,
        offset,
    }))
}

#[derive(Debug, Deserialize)]
pub struct ShopEdit {
    pub name: String,
    /// `None` means the field was omitted (or explicitly null), so the
    /// stored description is left alone.
    #[serde(default)]
    pub description: Option<String>,
}

/// Corrects a shop's name or description. Its currency is deliberately not
/// editable here: the prices already stored are in it, and converting them is
/// the seller's editor's job, not a console field.
pub async fn update_shop(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<ShopEdit>,
) -> ApiResult<Json<serde_json::Value>> {
    require_admin(&state.db, &headers).await?;
    let name = required(
        &request.name,
        crate::shops::MAX_SHOP_NAME_LEN,
        "The shop name",
    )?;
    let description = request
        .description
        .as_deref()
        .map(|value| {
            optional(
                value,
                crate::shops::MAX_SHOP_DESCRIPTION_LEN,
                "The shop description",
            )
        })
        .transpose()?;
    let connection = state.db.connect().map_err(auth::internal)?;
    let updated = match description {
        Some(description) => {
            connection
                .execute(
                    "UPDATE shops SET name = ?1, description = ?2 WHERE id = ?3",
                    (name, description, id),
                )
                .await
        }
        None => {
            connection
                .execute("UPDATE shops SET name = ?1 WHERE id = ?2", (name, id))
                .await
        }
    }
    .map_err(auth::internal)?;
    if updated == 0 {
        return Err(ApiError::NotFound("No shop with that id.".into()));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// Removes a shop and its catalogue. Invoices already made keep their receipt:
/// it lives in the buyer's code, not in this table.
pub async fn delete_shop(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    require_admin(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;
    for sql in [
        "DELETE FROM products WHERE shop_id = ?1",
        "DELETE FROM shop_categories WHERE shop_id = ?1",
    ] {
        connection
            .execute(sql, [id.as_str()])
            .await
            .map_err(auth::internal)?;
    }
    let removed = connection
        .execute("DELETE FROM shops WHERE id = ?1", [id.as_str()])
        .await
        .map_err(auth::internal)?;
    if removed == 0 {
        return Err(ApiError::NotFound("No shop with that id.".into()));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}
