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
    // A code is once per buyer, and the buyer is a wallet: a payment link can be
    // opened without an account, so an account id would leave most redemptions
    // uncounted. The invoice is kept because it is the evidence — `use_code`
    // reads the payer off the settled invoice rather than trusting a request.
    "CREATE TABLE IF NOT EXISTS discount_redemptions (
        id         TEXT PRIMARY KEY,
        user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        code       TEXT NOT NULL,
        buyer      TEXT NOT NULL,
        invoice    TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        UNIQUE (user_id, code, buyer)
    )",
    "CREATE INDEX IF NOT EXISTS discount_redemptions_user_id_idx ON discount_redemptions(user_id)",
    // A saved payment keeps a copy of the code that already *is* the invoice:
    // the API does not know invoices, only what the account wants to see again.
    // `shop_id` is the shop the seller filed it under, when they picked one.
    "CREATE TABLE IF NOT EXISTS saved_payments (
        id         TEXT PRIMARY KEY,
        user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        payment_id TEXT NOT NULL,
        payload    TEXT NOT NULL,
        shop_id    TEXT,
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
        currency    TEXT NOT NULL DEFAULT 'sol',
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
        category       TEXT NOT NULL DEFAULT '',
        price_lamports INTEGER NOT NULL,
        created_at     INTEGER NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS products_user_id_idx ON products(user_id)",
    // The headings a shop has used, kept per shop so the editor can suggest them
    // the next time rather than asking the seller to remember their own menu.
    "CREATE TABLE IF NOT EXISTS shop_categories (
        user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        shop_id    TEXT NOT NULL,
        name       TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        PRIMARY KEY (shop_id, name)
    )",
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
    // A settled invoice's facts, copied off its settle transaction once. The
    // chain keeps the only authoritative record — the marker — and this is a
    // cache: losing every row costs one history read, never correctness, which
    // is why nothing here is a foreign key or a source of truth.
    "CREATE TABLE IF NOT EXISTS settled_payments (\n        invoice    TEXT NOT NULL,\n        endpoint   TEXT NOT NULL,\n        seller     TEXT NOT NULL,\n        reference  TEXT NOT NULL,\n        mint       TEXT NOT NULL,\n        amount     INTEGER NOT NULL,\n        memo       TEXT NOT NULL,\n        payer      TEXT NOT NULL,\n        created_at INTEGER NOT NULL,\n        settled_at INTEGER NOT NULL,\n        PRIMARY KEY (invoice, endpoint)\n    )",
    // A wallet belongs to at most one account.
    "CREATE UNIQUE INDEX IF NOT EXISTS users_wallet_unique_idx \
     ON users(wallet) WHERE wallet IS NOT NULL",
    // Sellers who want to take real money, waiting for a live cluster. The
    // email is the key, so one address joins once; nothing here is linked to
    // an account, because someone on the list may not have one.
    "CREATE TABLE IF NOT EXISTS waitlist (\n        email      TEXT PRIMARY KEY,\n        created_at INTEGER NOT NULL\n    )",
    "CREATE INDEX IF NOT EXISTS password_resets_user_id_idx ON password_resets(user_id)",
    // The shops subscription: one row per paid period. `payment_id` is the
    // settled invoice address, so the same payment cannot buy two periods.
    "CREATE TABLE IF NOT EXISTS subscriptions (\n        id         TEXT PRIMARY KEY,\n        user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,\n        currency   TEXT NOT NULL,\n        amount     INTEGER NOT NULL,\n        payment_id TEXT NOT NULL,\n        paid_at    INTEGER NOT NULL,\n        expires_at INTEGER NOT NULL,\n        UNIQUE (user_id, payment_id)\n    )",
    "CREATE INDEX IF NOT EXISTS subscriptions_user_id_idx ON subscriptions(user_id)",
    // The invoice a subscription was quoted at, kept until it settles: the
    // amount the buyer agreed to is the amount `confirm` requires on chain,
    // whatever the rate has done since.
    "CREATE TABLE IF NOT EXISTS subscription_invoices (\n        payment_id TEXT PRIMARY KEY,\n        user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,\n        currency   TEXT NOT NULL,\n        amount     INTEGER NOT NULL,\n        created_at INTEGER NOT NULL\n    )",
    // Every settlement the relayer paid the network fee for. The caps in
    // `sponsor.rs` read this table, and a row is written only after the cluster
    // accepted the transaction, so the count is spend rather than attempts. The
    // signature is the id: one sponsored settle cannot be counted twice.
    //
    // The wallet is the key the caps count, not the account: a buyer who scanned
    // a printed code has no account here, and the cap has to hold for them too.
    // `user_id` is what we happen to know about them, never a requirement.
    "CREATE TABLE IF NOT EXISTS sponsored_settlements (\n        id         TEXT PRIMARY KEY,\n        wallet     TEXT NOT NULL,\n        user_id    TEXT,\n        cluster    TEXT NOT NULL,\n        amount     INTEGER NOT NULL,\n        created_at INTEGER NOT NULL\n    )",
    // A browser's push subscription: one row per device, keyed by the endpoint
    // the push service gave it. `locale` is the copy the reminder is written
    // in, because a notification arrives outside the app where no i18n runs.
    "CREATE TABLE IF NOT EXISTS push_subscriptions (\n        endpoint   TEXT PRIMARY KEY,\n        user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,\n        p256dh     TEXT NOT NULL,\n        auth       TEXT NOT NULL,\n        locale     TEXT NOT NULL DEFAULT 'en',\n        created_at INTEGER NOT NULL\n    )",
    "CREATE INDEX IF NOT EXISTS push_subscriptions_user_id_idx ON push_subscriptions(user_id)",
    // One reminder per account per month: the cron runs daily, and this is what
    // stops a month of mornings from becoming a month of notifications.
    // One starter-token claim per account. The row is reserved before the
    // transfer is sent, so a double tap cannot send twice; `signature` stays
    // empty until the cluster accepted it, and a failed send deletes the row.
    "CREATE TABLE IF NOT EXISTS faucet_claims (\n        user_id         TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,\n        wallet          TEXT NOT NULL,\n        cluster         TEXT NOT NULL,\n        sol_lamports    INTEGER NOT NULL,\n        token_base_units INTEGER NOT NULL,\n        signature       TEXT NOT NULL,\n        claimed_at      INTEGER NOT NULL\n    )",
    "CREATE TABLE IF NOT EXISTS push_nudges (\n        user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,\n        month_index INTEGER NOT NULL,\n        sent_at     INTEGER NOT NULL,\n        PRIMARY KEY (user_id, month_index)\n    )",
    // What an invoice is called and what it is for: the chain stores neither,
    // so the seller's own words are kept here when the invoice is made, and the
    // history can show a subject instead of an amount with no story.
    // The invoices this API made, one row per address the invoice can be paid
    // at. It began as the seller's own copy of a name and description — the
    // chain keeps neither — and the feed lists from it now: `amount` and
    // `reference` are what a status read needs, so the history asks the cluster
    // whether an invoice settled rather than for every payment ever made.
    "CREATE TABLE IF NOT EXISTS invoice_meta (\n        id          TEXT PRIMARY KEY,\n        seller      TEXT NOT NULL,\n        reference   TEXT NOT NULL DEFAULT '',\n        cluster     TEXT NOT NULL DEFAULT '',\n        endpoint    TEXT NOT NULL DEFAULT '',\n        currency    TEXT NOT NULL DEFAULT '',\n        mint        TEXT NOT NULL DEFAULT '',\n        amount      INTEGER NOT NULL DEFAULT 0,\n        memo        TEXT NOT NULL DEFAULT '',\n        seller_user TEXT,\n        name        TEXT NOT NULL,\n        description TEXT NOT NULL,\n        items       TEXT NOT NULL DEFAULT '',\n        created_at  INTEGER NOT NULL\n    )",
    // A reusable code: one offer — price, currency, title, words — that any
    // visitor pays by minting a fresh one-time invoice from it. The chain can
    // settle an invoice once, so the repeatable thing is the row, not an
    // address. The seller is the account that made it; `seller_user` is what
    // ties every payment it mints to that account's history.
    "CREATE TABLE IF NOT EXISTS reusable_links (\n        id          TEXT PRIMARY KEY,\n        seller      TEXT NOT NULL,\n        seller_user TEXT NOT NULL,\n        shop_id     TEXT,\n        cluster     TEXT NOT NULL,\n        endpoint    TEXT NOT NULL DEFAULT '',\n        currency    TEXT NOT NULL,\n        amount      INTEGER NOT NULL,\n        memo        TEXT NOT NULL DEFAULT '',\n        name        TEXT NOT NULL DEFAULT '',\n        description TEXT NOT NULL DEFAULT '',\n        created_at  INTEGER NOT NULL\n    )",
    "CREATE INDEX IF NOT EXISTS reusable_links_seller_idx ON reusable_links(seller_user, created_at)",
    // Which wallet a session key spends for. A payment made from an allowance is
    // signed by that key, not by the buyer's wallet, so the settle leaves a payer
    // the feed would not otherwise recognise as this buyer. The chain keeps the
    // pair — a budget account holds the buyer and its session key — and this is
    // what one look at it learned, so a later load matches the payment to the
    // wallet with a row read instead of another scan.
    "CREATE TABLE IF NOT EXISTS session_keys (\n        session_key TEXT NOT NULL,\n        wallet      TEXT NOT NULL,\n        endpoint    TEXT NOT NULL,\n        cluster     TEXT NOT NULL,\n        first_seen  INTEGER NOT NULL,\n        PRIMARY KEY (session_key, endpoint, cluster)\n    )",
    "CREATE INDEX IF NOT EXISTS session_keys_wallet_idx ON session_keys(wallet, endpoint, cluster)",
    // When this wallet's allowances were last looked up on chain. A wallet with
    // none would otherwise ask the cluster to confirm that on every feed load.
    "CREATE TABLE IF NOT EXISTS allowance_checks (\n        wallet     TEXT NOT NULL,\n        endpoint   TEXT NOT NULL,\n        cluster    TEXT NOT NULL,\n        checked_at INTEGER NOT NULL,\n        PRIMARY KEY (wallet, endpoint, cluster)\n    )",
    // An invoice row written before rows carried an amount is filled from the
    // chain once, the first time its seller opens the feed. This records the ids
    // the chain had nothing to say about, so a dead testnet invoice is not asked
    // about on every load afterwards.
    "CREATE TABLE IF NOT EXISTS invoice_backfill (\n        id       TEXT PRIMARY KEY,\n        endpoint TEXT NOT NULL,\n        tried_at INTEGER NOT NULL\n    )",
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
        eprintln!(
            "[db] builder.build() -> {}",
            if built.is_ok() { "ok" } else { "err" }
        );
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
async fn migrate_shop_currency(connection: &libsql::Connection) -> Result<(), DbError> {
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
async fn migrate_saved_codes_to_shops(connection: &libsql::Connection) -> Result<(), DbError> {
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
async fn migrate_settlements_to_wallets(connection: &libsql::Connection) -> Result<(), DbError> {
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
async fn migrate_invoice_columns(connection: &libsql::Connection) -> Result<(), DbError> {
    let additions = [
        ("reference", "TEXT NOT NULL DEFAULT ''"),
        ("cluster", "TEXT NOT NULL DEFAULT ''"),
        ("endpoint", "TEXT NOT NULL DEFAULT ''"),
        ("currency", "TEXT NOT NULL DEFAULT ''"),
        ("mint", "TEXT NOT NULL DEFAULT ''"),
        ("amount", "INTEGER NOT NULL DEFAULT 0"),
        ("memo", "TEXT NOT NULL DEFAULT ''"),
        ("seller_user", "TEXT"),
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
                ON invoice_meta(seller_user, created_at);",
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
}
