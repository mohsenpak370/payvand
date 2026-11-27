//! In-place upgrades for databases created before a schema change.
//!
//! Each step runs after `SCHEMA` and checks before it changes anything, so a
//! fresh database does nothing here and every cold start can run them all.

use super::error::DbError;

/// Shops began as one per account, with products hanging off the account. They
/// are a list now: several shops per seller, and each product belongs to one of
/// them. Both changes are applied here, in place, so a database created before
/// that keeps the shop and catalogue it already has.
///
/// Runs after `SCHEMA`, so on a fresh database every step is a no-op.
pub(super) async fn migrate_shops_to_a_list(
    connection: &libsql::Connection,
) -> Result<(), DbError> {
    // `products.shop_id` is additive, so the table does not need rebuilding.
    if !has_column(connection, "products", "shop_id").await? {
        connection
            .execute("ALTER TABLE products ADD COLUMN shop_id TEXT", ())
            .await?;
        eprintln!("[db] migrate: added products.shop_id");
    }

    // Categories are additive too, and an older catalogue simply has none.
    if !has_column(connection, "products", "category").await? {
        connection
            .execute(
                "ALTER TABLE products ADD COLUMN category TEXT NOT NULL DEFAULT ''",
                (),
            )
            .await?;
        eprintln!("[db] migrate: added products.category");
    }

    // One shop per account was expressed as a UNIQUE column. SQLite cannot drop
    // a constraint, so the table is rebuilt with the same rows.
    // The cursor is finished and dropped before any DDL runs: an open read on
    // sqlite_master keeps the schema locked on this connection.
    let definition: Option<String> = {
        let mut rows = connection
            .query(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'shops'",
                (),
            )
            .await?;
        let mut found = None;
        if let Some(row) = rows.next().await? {
            found = row.get(0).ok();
        }
        found
    };
    if let Some(definition) = definition {
        if definition.to_uppercase().contains("UNIQUE") {
            // One transaction, so a failure halfway cannot leave the shop table
            // missing: either the list-shaped table exists or the old one does.
            connection
                .execute_batch(
                    "BEGIN IMMEDIATE;\n                     DROP TABLE IF EXISTS shops_new;\n                     CREATE TABLE shops_new (\n                        id          TEXT PRIMARY KEY,\n                        user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,\n                        name        TEXT NOT NULL,\n                        description TEXT NOT NULL DEFAULT '',\n                        created_at  INTEGER NOT NULL\n                    );\n                    INSERT INTO shops_new (id, user_id, name, description, created_at)\n                        SELECT id, user_id, name, description, created_at FROM shops;\n                    DROP TABLE shops;\n                    ALTER TABLE shops_new RENAME TO shops;\n                    COMMIT;",
                )
                .await?;
            eprintln!("[db] migrate: shops rebuilt as a list");
        }
    }
    connection
        .execute(
            "CREATE INDEX IF NOT EXISTS shops_user_id_idx ON shops(user_id)",
            (),
        )
        .await?;

    // A product needs a shop. Any seller who added products before shops were a
    // list gets one shop of their own to keep them.
    connection
        .execute(
            "INSERT INTO shops (id, user_id, name, description, created_at)\n             SELECT lower(hex(randomblob(16))), p.user_id, 'My shop', '', MIN(p.created_at)\n             FROM products p\n             WHERE NOT EXISTS (SELECT 1 FROM shops s WHERE s.user_id = p.user_id)\n             GROUP BY p.user_id",
            (),
        )
        .await?;
    connection
        .execute(
            "UPDATE products SET shop_id = (\n                 SELECT s.id FROM shops s\n                 WHERE s.user_id = products.user_id\n                 ORDER BY s.created_at ASC, s.rowid ASC LIMIT 1\n             )\n             WHERE shop_id IS NULL",
            (),
        )
        .await?;

    // Created here rather than in `SCHEMA`: an older database only gets the
    // column above, so an index on it cannot be part of the first pass.
    connection
        .execute(
            "CREATE INDEX IF NOT EXISTS products_shop_id_idx ON products(shop_id)",
            (),
        )
        .await?;

    Ok(())
}

/// A shop prices its menu in one currency, chosen with the shop rather than on
/// every product. The column is additive: a shop created before it charges in
/// SOL, which is exactly what its stored prices already mean.
pub(super) async fn migrate_shop_currency(connection: &libsql::Connection) -> Result<(), DbError> {
    if !has_column(connection, "shops", "currency").await? {
        connection
            .execute(
                "ALTER TABLE shops ADD COLUMN currency TEXT NOT NULL DEFAULT 'sol'",
                (),
            )
            .await?;
        eprintln!("[db] migrate: added shops.currency");
    }
    Ok(())
}

/// Saved codes began on the account alone. They now belong to a shop, which is
/// what the seller files them under, so the shop page can list what it keeps.
/// The column is additive: codes saved before this keep their place on the
/// account and simply belong to no shop yet.
pub(super) async fn migrate_saved_codes_to_shops(
    connection: &libsql::Connection,
) -> Result<(), DbError> {
    if !has_column(connection, "saved_payments", "shop_id").await? {
        connection
            .execute("ALTER TABLE saved_payments ADD COLUMN shop_id TEXT", ())
            .await?;
        eprintln!("[db] migrate: added saved_payments.shop_id");
    }

    // Created here rather than in `SCHEMA`, because an older database only gets
    // the column above, and an index on a missing column cannot be built.
    connection
        .execute(
            "CREATE INDEX IF NOT EXISTS saved_payments_shop_id_idx ON saved_payments(shop_id)",
            (),
        )
        .await?;

    Ok(())
}

/// A sponsored settlement used to belong to an account: the row carried a
/// `NOT NULL REFERENCES users` column, so anything paid by a buyer without one
/// could not be recorded at all. The cap is keyed by the wallet now, and the
/// account is only what we happen to know.
///
/// SQLite cannot drop a NOT NULL or a foreign key in place, so the table is
/// rebuilt with the rows it already has — the same shape of upgrade the shops
/// table got. Runs after `SCHEMA`, so a fresh database has nothing to do.
pub(super) async fn migrate_settlements_to_wallets(
    connection: &libsql::Connection,
) -> Result<(), DbError> {
    let definition: Option<String> = {
        let mut rows = connection
            .query(
                "SELECT sql FROM sqlite_master WHERE type = 'table' \
                 AND name = 'sponsored_settlements'",
                (),
            )
            .await?;
        // The cursor is finished and dropped before any DDL runs: an open read
        // on sqlite_master keeps the schema locked on this connection.
        match rows.next().await? {
            Some(row) => row.get(0).ok(),
            None => None,
        }
    };
    let Some(definition) = definition else {
        return Ok(());
    };

    if definition.contains("REFERENCES users") {
        connection
            .execute_batch(
                "ALTER TABLE sponsored_settlements RENAME TO sponsored_settlements_legacy;
                 CREATE TABLE sponsored_settlements (
                    id         TEXT PRIMARY KEY,
                    wallet     TEXT NOT NULL,
                    user_id    TEXT,
                    cluster    TEXT NOT NULL,
                    amount     INTEGER NOT NULL,
                    created_at INTEGER NOT NULL
                 );
                 INSERT INTO sponsored_settlements (id, wallet, user_id, cluster, amount, created_at)
                    SELECT id, wallet, user_id, cluster, amount, created_at
                    FROM sponsored_settlements_legacy;
                 DROP TABLE sponsored_settlements_legacy;",
            )
            .await?;
        eprintln!("[db] migrate: sponsored_settlements keyed by wallet");
    }

    // The caps count one wallet's last day and the whole deployment's, which are
    // the two reads this table serves.
    connection
        .execute_batch(
            "CREATE INDEX IF NOT EXISTS sponsored_settlements_wallet_idx \
                ON sponsored_settlements(wallet, created_at);
             CREATE INDEX IF NOT EXISTS sponsored_settlements_created_idx \
                ON sponsored_settlements(created_at);",
        )
        .await?;

    Ok(())
}

/// An invoice row began as a name and a description: the seller's own copy of
/// what the chain cannot hold. It is the invoice list now — who made it, at
/// which address, for which currency and how much — which is what lets the feed
/// read our rows instead of the program's whole account table.
///
/// Additive on purpose: a row written before this keeps its name and gains
/// empty facts, and `recorded_invoices` skips those (`amount = 0`) so the scan
/// still answers for them and fills them in.
pub(super) async fn migrate_invoice_columns(
    connection: &libsql::Connection,
) -> Result<(), DbError> {
    let additions = [
        ("reference", "TEXT NOT NULL DEFAULT ''"),
        ("cluster", "TEXT NOT NULL DEFAULT ''"),
        ("endpoint", "TEXT NOT NULL DEFAULT ''"),
        ("currency", "TEXT NOT NULL DEFAULT ''"),
        ("mint", "TEXT NOT NULL DEFAULT ''"),
        ("amount", "INTEGER NOT NULL DEFAULT 0"),
        ("memo", "TEXT NOT NULL DEFAULT ''"),
        ("seller_user", "TEXT"),
        // Which reusable code minted this invoice, when one did. A code is
        // opened far more often than it is paid, so this is what lets history
        // leave a lookup out until it settles.
        ("reusable_id", "TEXT NOT NULL DEFAULT ''"),
        // When a feed first saw that invoice settled, for a reusable one. The
        // settlement row needs a history read that can fail; this does not, so
        // a paid visit stays history even after newer visits push it out of
        // the handful that are still watched.
        ("paid_at", "INTEGER NOT NULL DEFAULT 0"),
    ];
    for (column, definition) in additions {
        if !has_column(connection, "invoice_meta", column).await? {
            connection
                .execute(
                    &format!("ALTER TABLE invoice_meta ADD COLUMN {column} {definition}"),
                    (),
                )
                .await?;
            eprintln!("[db] migrate: added invoice_meta.{column}");
        }
    }

    // The two reads the feed makes: one wallet's invoices, newest first, and one
    // account's.
    connection
        .execute_batch(
            "CREATE INDEX IF NOT EXISTS invoice_meta_seller_idx \
                ON invoice_meta(seller, created_at);
             CREATE INDEX IF NOT EXISTS invoice_meta_user_idx \
                ON invoice_meta(seller_user, created_at);
             CREATE INDEX IF NOT EXISTS invoice_meta_reusable_idx \
                ON invoice_meta(reusable_id, created_at) WHERE reusable_id <> '';",
        )
        .await?;

    Ok(())
}

pub(super) async fn has_column(
    connection: &libsql::Connection,
    table: &str,
    column: &str,
) -> Result<bool, DbError> {
    // Table names here are constants in the db module, never user input.
    let mut rows = connection
        .query(&format!("PRAGMA table_info({table})"), ())
        .await?;
    let mut columns = Vec::new();
    while let Some(row) = rows.next().await? {
        columns.push(row.get::<String>(1)?);
    }
    Ok(columns.iter().any(|name| name == column))
}

#[cfg(test)]
mod tests;
