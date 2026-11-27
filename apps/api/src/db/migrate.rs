//! Brings a database up to the current schema: the `SCHEMA` statements, then
//! every in-place upgrade in order.

use libsql::Database;

use super::error::DbError;
use super::schema::SCHEMA;
use super::upgrades::{
    has_column, migrate_invoice_columns, migrate_saved_codes_to_shops,
    migrate_settlements_to_wallets, migrate_shop_currency, migrate_shops_to_a_list,
};

/// Idempotent, so every serverless cold start is safe.
pub async fn migrate(db: &Database) -> Result<(), DbError> {
    let connection = db.connect()?;
    migrate_connection(&connection).await
}

pub async fn migrate_connection(connection: &libsql::Connection) -> Result<(), DbError> {
    eprintln!("[db] migrate: {} statements", SCHEMA.len());
    for (i, statement) in SCHEMA.iter().enumerate() {
        connection.execute(statement, ()).await?;
        eprintln!("[db] migrate statement {} ok", i + 1);
    }
    migrate_shops_to_a_list(connection).await?;
    migrate_shop_currency(connection).await?;
    migrate_saved_codes_to_shops(connection).await?;
    migrate_settlements_to_wallets(connection).await?;
    migrate_invoice_columns(connection).await?;
    // The receipt lines an invoice was priced from. Additive: an invoice made
    // before this keeps an empty list, and a database created after it has the
    // column from the statement above.
    if !has_column(connection, "invoice_meta", "items").await? {
        connection
            .execute(
                "ALTER TABLE invoice_meta ADD COLUMN items TEXT NOT NULL DEFAULT ''",
                (),
            )
            .await?;
        eprintln!("[db] migrate: added invoice_meta.items");
    }
    eprintln!("[db] migrate done");
    Ok(())
}
