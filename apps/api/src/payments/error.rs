//! Shared API errors and pubkey parsing.

use solana_pubkey::Pubkey;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("authentication required")]
    Unauthorized,
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    TooManyRequests(String),
    #[error("{0}")]
    Internal(String),
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = match &self {
            Self::BadRequest(_) => axum::http::StatusCode::BAD_REQUEST,
            Self::Unauthorized => axum::http::StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => axum::http::StatusCode::FORBIDDEN,
            Self::NotFound(_) => axum::http::StatusCode::NOT_FOUND,
            Self::TooManyRequests(_) => axum::http::StatusCode::TOO_MANY_REQUESTS,
            Self::Internal(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        };
        // An internal failure can name a table, a query or a file, so it is
        // logged where the operator can read it and answered with a sentence
        // that tells the caller only what to do next.
        let body = match &self {
            Self::Internal(message) => {
                tracing::error!(%message, "internal error");
                axum::Json(serde_json::json!({
                    "error": "Something went wrong on our side. Try again."
                }))
            }
            other => axum::Json(serde_json::json!({ "error": other.to_string() })),
        };
        (status, body).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

pub(crate) fn bad(message: impl Into<String>) -> ApiError {
    ApiError::BadRequest(message.into())
}

pub(crate) fn parse_pubkey(value: &str, field: &str) -> ApiResult<Pubkey> {
    Pubkey::from_str(value).map_err(|_| bad(format!("{field} is not a valid solana address")))
}

#[cfg(test)]
mod tests;
