use crate::db::migrate_connection;
use libsql::{Builder, Database};

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
        .query("SELECT shop_id FROM products WHERE id = 'product-1'", ())
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
    migrate_connection(&connection)
        .await
        .expect("migrated again");
    let mut rows = connection
        .query("SELECT COUNT(*) FROM shops", ())
        .await
        .expect("count");
    let count: i64 = rows
        .next()
        .await
        .expect("row")
        .expect("count")
        .get(0)
        .expect("value");
    assert_eq!(count, 2);

    let _ = std::fs::remove_file(path);
}

/// Sponsored settlements used to require an account. A row written under
/// that rule keeps its place, and a buyer with no account can now be
/// recorded at all — which is what sponsorship for a printed code needs.
#[tokio::test]
async fn upgrades_sponsored_settlements_to_wallet_keys() {
    let path = std::env::temp_dir().join(format!(
        "payvand-settle-migrate-test-{}.db",
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
            CREATE TABLE sponsored_settlements (
                id         TEXT PRIMARY KEY,
                user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                wallet     TEXT NOT NULL,
                cluster    TEXT NOT NULL,
                amount     INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            );
            INSERT INTO users (id, email, password_hash, wallet, created_at)
                VALUES ('user-1', 'seller@example.com', 'x', NULL, 1);
            INSERT INTO sponsored_settlements (id, user_id, wallet, cluster, amount, created_at)
                VALUES ('sig-1', 'user-1', 'wallet-1', 'devnet', 42, 2);",
        )
        .await
        .expect("old schema");

    migrate_connection(&connection).await.expect("migrated");

    let mut rows = connection
        .query(
            "SELECT wallet, user_id FROM sponsored_settlements WHERE id = 'sig-1'",
            (),
        )
        .await
        .expect("read row");
    let row = rows.next().await.expect("row").expect("settlement");
    assert_eq!(row.get::<String>(0).expect("wallet"), "wallet-1");
    assert_eq!(row.get::<String>(1).expect("account"), "user-1");

    // No account, and the row fits: this is the anonymous buyer.
    connection
        .execute(
            "INSERT INTO sponsored_settlements (id, wallet, user_id, cluster, amount, created_at) \
             VALUES ('sig-2', 'wallet-2', NULL, 'devnet', 7, 3)",
            (),
        )
        .await
        .expect("a wallet with no account can be recorded");

    // Re-running is a no-op, as every cold start does.
    migrate_connection(&connection)
        .await
        .expect("migrated again");
    let mut rows = connection
        .query("SELECT COUNT(*) FROM sponsored_settlements", ())
        .await
        .expect("count");
    let count: i64 = rows
        .next()
        .await
        .expect("row")
        .expect("count")
        .get(0)
        .expect("value");
    assert_eq!(count, 2);

    let _ = std::fs::remove_file(path);
}
