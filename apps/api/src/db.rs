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
    eprintln!("[db] migrate done");
    Ok(())
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
