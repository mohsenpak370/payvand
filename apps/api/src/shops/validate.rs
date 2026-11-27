//! Checks every write passes before it reaches the database.

use crate::payments::{bad, ApiResult};

use super::model::{default_currency, MAX_PRICE_LAMPORTS, SHOP_CURRENCIES};

/// Trimmed, required text with a length the storage and the QR can afford.
pub(super) fn required_text(raw: &str, max: usize, field: &str) -> ApiResult<String> {
    let value = raw.trim().to_string();
    if value.is_empty() {
        return Err(bad(format!("{field} cannot be empty.")));
    }
    if value.chars().count() > max {
        return Err(bad(format!("{field} must be at most {max} characters.")));
    }
    Ok(value)
}

pub(super) fn optional_text(raw: &str, max: usize, field: &str) -> ApiResult<String> {
    let value = raw.trim().to_string();
    if value.chars().count() > max {
        return Err(bad(format!("{field} must be at most {max} characters.")));
    }
    Ok(value)
}

/// One of the currencies the app can charge in, lowercased. Absent means SOL,
/// which is what a shop created before the choice existed charges in.
pub(super) fn valid_shop_currency(raw: Option<&str>) -> ApiResult<String> {
    let Some(raw) = raw else {
        return Ok(default_currency());
    };
    let currency = raw.trim().to_ascii_lowercase();
    if !SHOP_CURRENCIES.contains(&currency.as_str()) {
        return Err(bad("A shop can charge in SOL, USDC or EURC."));
    }
    Ok(currency)
}

/// A price of zero would create an invoice nobody can derive an address for,
/// so it is refused here rather than on the payment page.
pub(super) fn valid_price(price_lamports: u64) -> ApiResult<u64> {
    if price_lamports == 0 {
        return Err(bad("A price must be greater than 0."));
    }
    if price_lamports > MAX_PRICE_LAMPORTS {
        return Err(bad(format!(
            "A price must be at most {} SOL.",
            MAX_PRICE_LAMPORTS / 1_000_000_000
        )));
    }
    Ok(price_lamports)
}

#[cfg(test)]
mod tests;
