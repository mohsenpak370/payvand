//! Shared fixtures for the auth handler tests.

use super::random_id;

/// Handler tests run against a temp-file database: unlike `:memory:`,
/// every `db.connect()` sees the same schema and data.
pub(super) async fn test_state() -> (crate::AppState, std::path::PathBuf) {
    use std::sync::Arc;
    let path = std::env::temp_dir().join(format!("payvand-auth-test-{}.db", random_id()));
    let db = libsql::Builder::new_local(&path)
        .build()
        .await
        .expect("temp db");
    crate::db::migrate(&db).await.expect("migrate");
    (crate::AppState { db: Arc::new(db) }, path)
}

pub(super) async fn count(state: &crate::AppState, sql: &str) -> i64 {
    let connection = state.db.connect().expect("connect");
    let mut rows = connection.query(sql, ()).await.expect("query");
    match rows.next().await.expect("row") {
        Some(row) => row.get(0).expect("count"),
        None => 0,
    }
}
