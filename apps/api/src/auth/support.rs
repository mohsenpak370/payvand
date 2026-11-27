//! Small helpers the rest of the crate leans on: random ids, the clock,
//! error mapping and row decoding.

use crate::payments::{ApiError, ApiResult};

/// 16 random bytes, hex encoded — used for user ids and session tokens.
pub(crate) fn random_id() -> String {
    hex(&random_bytes::<16>())
}

pub(super) fn random_bytes<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    // `getrandom` is already in the tree through argon2; it reads the OS CSPRNG.
    getrandom::fill(&mut bytes).expect("the operating system RNG failed");
    bytes
}

pub(super) fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

pub(crate) fn internal(error: impl std::fmt::Display) -> ApiError {
    ApiError::Internal(error.to_string())
}

pub(crate) fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or_default()
}

pub(crate) fn is_unique_violation(error: &libsql::Error) -> bool {
    error.to_string().contains("UNIQUE constraint failed")
}

pub(crate) fn nullable_text(row: &libsql::Row, index: i32) -> ApiResult<Option<String>> {
    match row.get::<libsql::Value>(index).map_err(internal)? {
        libsql::Value::Null => Ok(None),
        libsql::Value::Text(text) => Ok(Some(text)),
        other => Err(ApiError::Internal(format!(
            "unexpected value in column {index}: {other:?}"
        ))),
    }
}

/// Where emailed links point. `FRONTEND_BASE_URL` in production (Beacon
/// names it the same), localhost in development.
pub(super) fn frontend_base_url() -> String {
    std::env::var("FRONTEND_BASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim_end_matches('/').to_string())
        .unwrap_or_else(|| "http://localhost:3000".to_string())
}
