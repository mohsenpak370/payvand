//! The `users` table: the account shape and the queries that read it.

use serde::{Deserialize, Serialize};

use super::support::{internal, nullable_text};
use crate::payments::ApiResult;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub wallet: Option<String>,
    pub created_at: i64,
}

pub(crate) async fn read_user(
    connection: &libsql::Connection,
    id: &str,
) -> ApiResult<Option<User>> {
    let mut rows = connection
        .query(
            "SELECT id, email, wallet, created_at FROM users WHERE id = ?",
            [id],
        )
        .await
        .map_err(internal)?;
    read_user_row(&mut rows).await
}

pub(super) async fn read_user_with_hash(
    connection: &libsql::Connection,
    email: &str,
) -> ApiResult<Option<(User, String)>> {
    let mut rows = connection
        .query(
            "SELECT id, email, wallet, created_at, password_hash FROM users WHERE email = ?",
            [email],
        )
        .await
        .map_err(internal)?;

    match rows.next().await.map_err(internal)? {
        Some(row) => {
            let user = User {
                id: row.get(0).map_err(internal)?,
                email: row.get(1).map_err(internal)?,
                wallet: nullable_text(&row, 2)?,
                created_at: row.get(3).map_err(internal)?,
            };
            let hash: String = row.get(4).map_err(internal)?;
            Ok(Some((user, hash)))
        }
        None => Ok(None),
    }
}

/// Decodes the next row of an `id, email, wallet, created_at` selection.
pub(super) async fn read_user_row(rows: &mut libsql::Rows) -> ApiResult<Option<User>> {
    match rows.next().await.map_err(internal)? {
        Some(row) => Ok(Some(User {
            id: row.get(0).map_err(internal)?,
            email: row.get(1).map_err(internal)?,
            wallet: nullable_text(&row, 2)?,
            created_at: row.get(3).map_err(internal)?,
        })),
        None => Ok(None),
    }
}

/// The account a seller's wallet address belongs to, if one is linked to it.
///
/// Invoices name a wallet, and the API looks the account up from it, because a
/// payment code is data a stranger can edit: trusting an account id from one
/// would let the payer choose whose discount list prices an invoice.
pub(crate) async fn seller_account(
    db: &libsql::Database,
    wallet: &str,
) -> ApiResult<Option<String>> {
    let wallet = wallet.trim();
    if wallet.is_empty() {
        return Ok(None);
    }
    let connection = db.connect().map_err(internal)?;
    let mut rows = connection
        .query("SELECT id FROM users WHERE wallet = ?1", [wallet])
        .await
        .map_err(internal)?;
    match rows.next().await.map_err(internal)? {
        Some(row) => Ok(Some(row.get(0).map_err(internal)?)),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests;
