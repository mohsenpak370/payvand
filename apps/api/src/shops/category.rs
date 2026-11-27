//! The `/api/shops/{id}/categories` routes: the headings a menu is listed under.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;

use crate::auth;
use crate::payments::ApiResult;

use super::model::{CategoryNames, CategoryRequest, MAX_PRODUCT_CATEGORY_LEN};
use super::store::require_shop;
use super::validate::required_text;

/// Remembers a heading for the shop, so the next product can pick it instead of
/// retyping it. An empty category is not a heading and is not remembered.
pub(super) async fn remember_category(
    db: &libsql::Database,
    user_id: &str,
    shop_id: &str,
    category: &str,
) -> ApiResult<()> {
    let name = category.trim();
    if name.is_empty() {
        return Ok(());
    }
    let connection = db.connect().map_err(auth::internal)?;
    connection
        .execute(
            "INSERT INTO shop_categories (user_id, shop_id, name, created_at) \
             VALUES (?1, ?2, ?3, ?4) ON CONFLICT(shop_id, name) DO NOTHING",
            (
                user_id.to_string(),
                shop_id.to_string(),
                name.to_string(),
                auth::unix_now(),
            ),
        )
        .await
        .map_err(auth::internal)?;
    Ok(())
}

/// The shop's category list, alphabetically.
pub async fn categories(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Path(shop_id): Path<String>,
) -> ApiResult<Json<CategoryNames>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;
    require_shop(&connection, &user.id, &shop_id).await?;

    let mut rows = connection
        .query(
            "SELECT name FROM shop_categories WHERE user_id = ?1 AND shop_id = ?2 \
             ORDER BY name COLLATE NOCASE",
            (user.id.clone(), shop_id.clone()),
        )
        .await
        .map_err(auth::internal)?;
    let mut found = Vec::new();
    while let Some(row) = rows.next().await.map_err(auth::internal)? {
        found.push(row.get::<String>(0).map_err(auth::internal)?);
    }
    Ok(Json(CategoryNames { categories: found }))
}

/// Adds a heading the seller typed but has not saved a product under yet.
pub async fn add_category(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Path(shop_id): Path<String>,
    Json(request): Json<CategoryRequest>,
) -> ApiResult<Json<CategoryNames>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let name = required_text(&request.name, MAX_PRODUCT_CATEGORY_LEN, "The category name")?;
    let connection = state.db.connect().map_err(auth::internal)?;
    require_shop(&connection, &user.id, &shop_id).await?;
    remember_category(&state.db, &user.id, &shop_id, &name).await?;
    drop(connection);
    categories(State(state), headers, Path(shop_id)).await
}

/// Forgets a heading. Products keep the text they were saved with, so nothing a
/// customer already sees disappears; the list simply stops suggesting it.
pub async fn delete_category(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Path((shop_id, name)): Path<(String, String)>,
) -> ApiResult<Json<CategoryNames>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;
    require_shop(&connection, &user.id, &shop_id).await?;
    connection
        .execute(
            "DELETE FROM shop_categories WHERE user_id = ?1 AND shop_id = ?2 AND name = ?3",
            (user.id.clone(), shop_id.clone(), name),
        )
        .await
        .map_err(auth::internal)?;
    drop(connection);
    categories(State(state), headers, Path(shop_id)).await
}
