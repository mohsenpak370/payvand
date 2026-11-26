//! Seller-owned records an operator may read or correct, the way a database
//! console would. The account gate is the same `ADMIN_EMAILS` check as
//! everything else here, and every write re-checks it.

use serde::Deserialize;

use crate::payments::{ApiError, ApiResult};

pub mod codes;
pub mod products;
pub mod shops;

pub use codes::*;
pub use products::*;
pub use shops::*;

#[derive(Debug, Deserialize)]
pub struct RecordQuery {
    pub search: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

const DEFAULT_RECORD_PAGE: i64 = 25;
const MAX_RECORD_PAGE: i64 = 100;

/// The page this request asks for, clamped rather than trusted.
fn record_page(query: &RecordQuery) -> (i64, i64) {
    (
        query
            .limit
            .unwrap_or(DEFAULT_RECORD_PAGE)
            .clamp(1, MAX_RECORD_PAGE),
        query.offset.unwrap_or(0).max(0),
    )
}

/// A literal substring pattern: wildcards from the input are stripped rather
/// than interpreted, so a search can never widen its own scope.
fn record_search(query: &RecordQuery) -> Option<String> {
    query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(|term| format!("%{}%", term.replace(['%', '_'], " ")))
}

fn required(raw: &str, max: usize, field: &str) -> ApiResult<String> {
    let value = raw.trim().to_string();
    if value.is_empty() {
        return Err(ApiError::BadRequest(format!("{field} cannot be empty.")));
    }
    if value.chars().count() > max {
        return Err(ApiError::BadRequest(format!(
            "{field} must be at most {max} characters."
        )));
    }
    Ok(value)
}

fn optional(raw: &str, max: usize, field: &str) -> ApiResult<String> {
    let value = raw.trim().to_string();
    if value.chars().count() > max {
        return Err(ApiError::BadRequest(format!(
            "{field} must be at most {max} characters."
        )));
    }
    Ok(value)
}
