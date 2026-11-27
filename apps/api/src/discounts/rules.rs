//! The rules a code and its percentage follow, and the price they give.

use crate::payments::{bad, ApiResult};

/// Uppercases and range-checks a seller- or buyer-supplied code.
pub fn normalize_code(raw: &str) -> ApiResult<String> {
    let code = raw.trim().to_uppercase();
    let ok_len = (3..=32).contains(&code.len());
    let ok_chars = code
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !ok_len || !ok_chars {
        return Err(bad(
            "A discount code is 3 to 32 letters, numbers, dashes or underscores.",
        ));
    }
    Ok(code)
}

pub(super) fn validate_percent(percent_off: u32) -> ApiResult<u32> {
    if !(1..=99).contains(&percent_off) {
        return Err(bad("A discount must be between 1% and 99%."));
    }
    Ok(percent_off)
}

/// `price - percent`, rounded down to the nearest lamport so the seller
/// never under-collects by rounding up.
pub fn discounted_lamports(price_lamports: u64, percent_off: u32) -> u64 {
    let percent_off = percent_off.min(99);
    (u128::from(price_lamports) * u128::from(100 - percent_off) / 100) as u64
}

#[cfg(test)]
mod tests;
