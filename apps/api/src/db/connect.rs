//! Opening a database: an embedded SQLite file locally, a remote libSQL server
//! when `DATABASE_URL` names one.

use libsql::{Builder, Database};
use std::sync::Arc;

use super::error::DbError;
use super::migrate::migrate;
#[cfg(test)]
use super::migrate::migrate_connection;

pub type SharedDb = Arc<Database>;

/// Used when `DATABASE_URL` is unset. Relative to the working directory, so
/// run the API from the repository root.
const DEFAULT_LOCAL_PATH: &str = "data/payvand.db";

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
