//! What a person types in: email normalisation and password rules/hashing.

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};

use crate::payments::{ApiError, ApiResult};

pub const MIN_PASSWORD_LEN: usize = 8;

pub fn hash_password(password: &str) -> ApiResult<String> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|error| ApiError::Internal(format!("could not hash the password: {error}")))
}

pub fn verify_password(password: &str, stored: &str) -> bool {
    PasswordHash::new(stored)
        .map(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
        .unwrap_or(false)
}

pub(crate) fn normalize_email(email: &str) -> ApiResult<String> {
    let normalized = email.trim().to_lowercase();
    if !normalized.is_empty()
        && normalized.len() <= 254
        && !normalized.chars().any(char::is_whitespace)
        && normalized.matches('@').count() == 1
        && {
            let (local, domain) = normalized.split_once('@').expect("one @ above");
            !local.is_empty() && domain.contains('.') && !domain.starts_with('.')
        }
    {
        return Ok(normalized);
    }
    Err(ApiError::BadRequest("Enter a valid email address.".into()))
}

pub(super) fn validate_password(password: &str) -> ApiResult<String> {
    if password.len() >= MIN_PASSWORD_LEN {
        return Ok(password.to_string());
    }
    Err(ApiError::BadRequest(format!(
        "The password must be at least {MIN_PASSWORD_LEN} characters."
    )))
}

#[cfg(test)]
mod tests;
