//! Database access.
//!
//! Local development uses an embedded SQLite file so nothing has to be
//! installed; production points `DATABASE_URL` at a libSQL server (Turso
//! compatible) which is HTTP based and therefore safe inside a serverless
//! function.
//!
//! This file is the public surface. Implementation lives in the `db/`
//! directory:
//!
//! - `connect`: opening the local or remote database
//! - `migrate`: bringing a database up to the current schema
//! - `schema`: the tables and indexes a fresh database is created with
//! - `upgrades`: in-place upgrades for databases created before a change
//! - `error`: `DbError`

mod connect;
mod error;
mod migrate;
mod schema;
mod upgrades;

#[cfg(test)]
pub use connect::in_memory;
pub use connect::{open, open_and_migrate, SharedDb};
pub use error::DbError;
pub use migrate::{migrate, migrate_connection};
