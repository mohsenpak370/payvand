//! Reads shared by every route: the ownership check and the catalogue.

use crate::auth;
use crate::payments::{bad, ApiResult};

use super::model::Product;

pub(super) fn read_product(row: &libsql::Row) -> ApiResult<Product> {
    let price: i64 = row.get(5).map_err(auth::internal)?;
    Ok(Product {
        id: row.get(0).map_err(auth::internal)?,
        shop_id: row.get(1).map_err(auth::internal)?,
        name: row.get(2).map_err(auth::internal)?,
        details: row.get(3).map_err(auth::internal)?,
        category: row.get(4).map_err(auth::internal)?,
        price_lamports: u64::try_from(price)
            .map_err(|_| crate::payments::ApiError::Internal("stored price out of range".into()))?,
        created_at_unix: row.get(6).map_err(auth::internal)?,
    })
}

/// Refuses a shop id that is not this account's. Every product route goes
/// through it, so a shop id in a request can never reach another account.
pub(super) async fn require_shop(
    connection: &libsql::Connection,
    user_id: &str,
    shop_id: &str,
) -> ApiResult<(String, String, String)> {
    let mut rows = connection
        .query(
            "SELECT name, description, currency FROM shops WHERE id = ?1 AND user_id = ?2",
            (shop_id, user_id),
        )
        .await
        .map_err(auth::internal)?;
    match rows.next().await.map_err(auth::internal)? {
        Some(row) => Ok((
            row.get(0).map_err(auth::internal)?,
            row.get(1).map_err(auth::internal)?,
            row.get(2).map_err(auth::internal)?,
        )),
        None => Err(bad("That shop no longer exists.")),
    }
}

pub(super) async fn products_for_shop(
    connection: &libsql::Connection,
    user_id: &str,
    shop_id: &str,
) -> ApiResult<Vec<Product>> {
    // Oldest first: a catalogue that keeps its order reads like a menu.
    let mut rows = connection
        .query(
            "SELECT id, shop_id, name, details, category, price_lamports, created_at FROM products \
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
