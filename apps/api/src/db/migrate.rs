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
    // A sponsored settle is recorded when the relayer signs it, and
    // `submitted_at` says it was also sent from here. Every row older than the
    // column was written after the cluster accepted it, so it was submitted.
    // What became of a signed settle: landed, failed or expired. Every row
    // older than the column was written after the cluster accepted it.
    if !has_column(connection, "sponsored_settlements", "outcome").await? {
        connection
            .execute_batch(
                "ALTER TABLE sponsored_settlements ADD COLUMN outcome TEXT;
                 UPDATE sponsored_settlements SET outcome = 'landed';",
            )
            .await?;
        eprintln!("[db] migrate: added sponsored_settlements.outcome");
    }
    if !has_column(connection, "sponsored_settlements", "submitted_at").await? {
        connection
            .execute_batch(
                "ALTER TABLE sponsored_settlements ADD COLUMN submitted_at INTEGER;
                 UPDATE sponsored_settlements SET submitted_at = created_at;",
            )
            .await?;
        eprintln!("[db] migrate: added sponsored_settlements.submitted_at");
    }
    // The fee a settlement charged its seller, from its `PaymentSettled` event.
    // Rows stored before this keep NULL — unknown — and the profile shows an
    // estimate for those; a later history read of the same settlement fills it.
    if !has_column(connection, "settled_payments", "fee").await? {
        connection
            .execute("ALTER TABLE settled_payments ADD COLUMN fee INTEGER", ())
            .await?;
        eprintln!("[db] migrate: added settled_payments.fee");
    }
    eprintln!("[db] migrate done");
    Ok(())
}
