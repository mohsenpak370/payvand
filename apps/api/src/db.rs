//! Database access.
//!
//! Local development uses an embedded SQLite file so nothing has to be
//! installed; production points `DATABASE_URL` at a libSQL server (Turso
//! compatible) which is HTTP based and therefore safe inside a serverless
//! function.

use libsql::{Builder, Database};
use std::fmt;
use std::sync::Arc;

pub type SharedDb = Arc<Database>;

/// Used when `DATABASE_URL` is unset. Relative to the working directory, so
/// run the API from the repository root.
const DEFAULT_LOCAL_PATH: &str = "data/payvand.db";

const SCHEMA: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS users (
        id            TEXT PRIMARY KEY,
        email         TEXT NOT NULL UNIQUE,
        password_hash TEXT NOT NULL,
        wallet        TEXT,
        created_at    INTEGER NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS sessions (
        token_hash TEXT PRIMARY KEY,
        user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        created_at INTEGER NOT NULL,
        expires_at INTEGER NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS sessions_user_id_idx ON sessions(user_id)",
    "CREATE INDEX IF NOT EXISTS sessions_expires_at_idx ON sessions(expires_at)",
    "CREATE TABLE IF NOT EXISTS discount_codes (
        id          TEXT PRIMARY KEY,
        user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        code        TEXT NOT NULL,
        percent_off INTEGER NOT NULL CHECK (percent_off BETWEEN 1 AND 99),
        created_at  INTEGER NOT NULL,
        UNIQUE (user_id, code)
    )",
    "CREATE INDEX IF NOT EXISTS discount_codes_user_id_idx ON discount_codes(user_id)",
    // A saved payment keeps a copy of the code that already *is* the invoice:
    // the API does not know invoices, only what the account wants to see again.
    "CREATE TABLE IF NOT EXISTS saved_payments (
        id         TEXT PRIMARY KEY,
        user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        payment_id TEXT NOT NULL,
        payload    TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        UNIQUE (user_id, payment_id)
    )",
    "CREATE INDEX IF NOT EXISTS saved_payments_user_id_idx ON saved_payments(user_id)",
    // A seller may keep several shops — a café and a bakery — so a shop is a
    // row of its own with an id, listed on the profile and opened on its own
    // page.
    "CREATE TABLE IF NOT EXISTS shops (
        id          TEXT PRIMARY KEY,
        user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        name        TEXT NOT NULL,
        description TEXT NOT NULL DEFAULT '',
        created_at  INTEGER NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS shops_user_id_idx ON shops(user_id)",
    // Products belong to a shop, and keep the account id as well: every read
    // is scoped to the signed-in account, so one shop's id in a request can
    // never reach another account's catalogue. `price_lamports` is the
    // canonical price — the receipt in every invoice is built from it.
    "CREATE TABLE IF NOT EXISTS products (
        id             TEXT PRIMARY KEY,
        user_id        TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        shop_id        TEXT NOT NULL,
        name           TEXT NOT NULL,
        details        TEXT NOT NULL DEFAULT '',
        price_lamports INTEGER NOT NULL,
        created_at     INTEGER NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS products_user_id_idx ON products(user_id)",
    "CREATE TABLE IF NOT EXISTS password_resets (
        token_hash TEXT PRIMARY KEY,
        user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        created_at INTEGER NOT NULL,
        expires_at INTEGER NOT NULL
    )",
    // One live sign-in/link challenge per address, single use, short lived.
    "CREATE TABLE IF NOT EXISTS wallet_challenges (
        address    TEXT PRIMARY KEY,
        purpose    TEXT NOT NULL,
        nonce      TEXT NOT NULL,
        message    TEXT NOT NULL,
        issued_at  INTEGER NOT NULL,
        expires_at INTEGER NOT NULL
    )",
    // A wallet belongs to at most one account.
    "CREATE UNIQUE INDEX IF NOT EXISTS users_wallet_unique_idx \
     ON users(wallet) WHERE wallet IS NOT NULL",
    "CREATE INDEX IF NOT EXISTS password_resets_user_id_idx ON password_resets(user_id)",
];

#[derive(Debug)]
pub struct DbError(String);

impl DbError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DbError {}

impl From<libsql::Error> for DbError {
    fn from(error: libsql::Error) -> Self {
        Self(error.to_string())
    }
}

pub async fn open() -> Result<Database, DbError> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_LOCAL_PATH.to_string());
    let target = url.strip_prefix("file:").unwrap_or(&url).to_string();

    if target.starts_with("libsql://")
        || target.starts_with("https://")
        || target.starts_with("http://")
    {
        let token = std::env::var("LIBSQL_AUTH_TOKEN").map_err(|_| {
            DbError::new("LIBSQL_AUTH_TOKEN is required when DATABASE_URL is remote")
        })?;
        let host = target
            .split("://")
            .nth(1)
            .unwrap_or("?")
            .split('/')
            .next()
            .unwrap_or("?");
        eprintln!("[db] remote open host={host} token_len={}", token.len());
        let built = Builder::new_remote(target, token).build().await;
        eprintln!("[db] builder.build() -> {}", if built.is_ok() { "ok" } else { "err" });
        return Ok(built?);
    }

    // Embedded SQLite: make sure the parent directory exists.
    if let Some(parent) = std::path::Path::new(&target).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    Ok(Builder::new_local(target).build().await?)
}

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
    eprintln!("[db] migrate done");
    Ok(())
}

/// Shops began as one per account, with products hanging off the account. They
/// are a list now: several shops per seller, and each product belongs to one of
/// them. Both changes are applied here, in place, so a database created before
/// that keeps the shop and catalogue it already has.
///
/// Runs after `SCHEMA`, so on a fresh database every step is a no-op.
async fn migrate_shops_to_a_list(connection: &libsql::Connection) -> Result<(), DbError> {
    // `products.shop_id` is additive, so the table does not need rebuilding.
    if !has_column(connection, "products", "shop_id").await? {
        connection
            .execute("ALTER TABLE products ADD COLUMN shop_id TEXT", ())
            .await?;
        eprintln!("[db] migrate: added products.shop_id");
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
        while let Some(row) = rows.next().await? {
            found = row.get(0).ok();
            break;
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

async fn has_column(
    connection: &libsql::Connection,
    table: &str,
    column: &str,
) -> Result<bool, DbError> {
    // Table names here are constants in this file, never user input.
    let mut rows = connection
        .query(&format!("PRAGMA table_info({table})"), ())
        .await?;
    let mut columns = Vec::new();
    while let Some(row) = rows.next().await? {
        columns.push(row.get::<String>(1)?);
    }
    Ok(columns.iter().any(|name| name == column))
}

pub async fn open_and_migrate() -> Result<Database, DbError> {
    let db = open().await?;
    migrate(&db).await?;
    Ok(db)
}

/// Empty in-memory database with the schema applied — used by tests.
///
/// An in-memory SQLite database is private to its connection, so both the
/// handle and the connection are returned to keep the schema alive.
#[cfg(test)]
pub async fn in_memory() -> Result<(Database, libsql::Connection), DbError> {
    let db = Builder::new_local(":memory:").build().await?;
    let connection = db.connect()?;
    migrate_connection(&connection).await?;
    Ok((db, connection))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A database created when a seller could keep only one shop.
    async fn single_shop_database() -> (Database, libsql::Connection, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "payvand-migrate-test-{}.db",
            crate::auth::random_id()
        ));
        let db = Builder::new_local(&path).build().await.expect("temp db");
        let connection = db.connect().expect("connect");
        connection
            .execute_batch(
                "CREATE TABLE users (
                    id            TEXT PRIMARY KEY,
                    email         TEXT NOT NULL UNIQUE,
                    password_hash TEXT NOT NULL,
                    wallet        TEXT,
                    created_at    INTEGER NOT NULL
                );
                CREATE TABLE shops (
                    id          TEXT PRIMARY KEY,
                    user_id     TEXT NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
                    name        TEXT NOT NULL,
                    description TEXT NOT NULL DEFAULT '',
                    created_at  INTEGER NOT NULL
                );
                CREATE TABLE products (
                    id             TEXT PRIMARY KEY,
                    user_id        TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                    name           TEXT NOT NULL,
                    details        TEXT NOT NULL DEFAULT '',
                    price_lamports INTEGER NOT NULL,
                    created_at     INTEGER NOT NULL
                );
                INSERT INTO users (id, email, password_hash, wallet, created_at)
                    VALUES ('user-1', 'seller@example.com', 'x', NULL, 1);
                INSERT INTO shops (id, user_id, name, description, created_at)
                    VALUES ('shop-1', 'user-1', 'Corner Shop', 'Flat whites', 2);
                INSERT INTO products (id, user_id, name, details, price_lamports, created_at)
                    VALUES ('product-1', 'user-1', 'Flat white', '', 12000000, 3);",
            )
            .await
            .expect("old schema");
        (db, connection, path)
    }

    #[tokio::test]
    async fn upgrades_a_single_shop_database_to_a_list() {
        let (_db, connection, path) = single_shop_database().await;

        migrate_connection(&connection).await.expect("migrated");

        // The shop survives, keeps its name, and owns the product it had.
        let mut rows = connection
            .query("SELECT name FROM shops WHERE id = 'shop-1'", ())
            .await
            .expect("read shop");
        let name: String = rows
            .next()
            .await
            .expect("row")
            .expect("shop")
            .get(0)
            .expect("name");
        assert_eq!(name, "Corner Shop");

        let mut rows = connection
            .query(
                "SELECT shop_id FROM products WHERE id = 'product-1'",
                (),
            )
            .await
            .expect("read product");
        let shop_id: String = rows
            .next()
            .await
            .expect("row")
            .expect("product")
            .get(0)
            .expect("shop id");
        assert_eq!(shop_id, "shop-1");

        // The unique account column is gone: a second shop fits.
        connection
            .execute(
                "INSERT INTO shops (id, user_id, name, description, created_at) \
                 VALUES ('shop-2', 'user-1', 'Corner Bakery', '', 4)",
                (),
            )
            .await
            .expect("a seller can keep two shops");

        // Re-running is a no-op, as every cold start does.
        migrate_connection(&connection).await.expect("migrated again");
        let mut rows = connection
            .query("SELECT COUNT(*) FROM shops", ())
            .await
            .expect("count");
        let count: i64 = rows.next().await.expect("row").expect("count").get(0).expect("value");
        assert_eq!(count, 2);

        let _ = std::fs::remove_file(path);
    }
}
