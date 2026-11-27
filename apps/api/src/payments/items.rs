//! Receipt validation: itemised lines must agree with the price.

use super::{
    error::{bad, ApiResult},
    model::{PaymentItem, PaymentOption, MAX_ITEMS, MAX_ITEM_NAME_LEN, MAX_ITEM_QUANTITY},
};

/// Cleans the receipt and makes it agree with the price. A code whose lines add
/// up to something other than what it charges is worse than no receipt at all,
/// so it is refused here rather than shown to a customer.
pub(crate) fn validate_items(
    items: Vec<PaymentItem>,
    options: &[PaymentOption],
) -> ApiResult<Vec<PaymentItem>> {
    if items.is_empty() {
        return Ok(items);
    }
    if items.len() > MAX_ITEMS {
        return Err(bad(format!(
            "A receipt can list at most {MAX_ITEMS} different products."
        )));
    }

    let mut cleaned = Vec::with_capacity(items.len());
    let mut total: u128 = 0;
    for item in items {
        let name = item.name.trim().to_string();
        if name.is_empty() {
            return Err(bad("Every receipt line needs a name."));
        }
        if name.chars().count() > MAX_ITEM_NAME_LEN {
            return Err(bad(format!(
                "A receipt line name must be at most {MAX_ITEM_NAME_LEN} characters."
            )));
        }
        if item.quantity == 0 || item.quantity > MAX_ITEM_QUANTITY {
            return Err(bad(format!(
                "A receipt line can be for 1 to {MAX_ITEM_QUANTITY} of an item."
            )));
        }
        if item.amount_lamports == 0 {
            return Err(bad("A receipt line needs a price greater than 0."));
        }
        total += u128::from(item.amount_lamports) * u128::from(item.quantity);
        cleaned.push(PaymentItem {
            name,
            quantity: item.quantity,
            amount_lamports: item.amount_lamports,
        });
    }

    // Only a SOL price can be checked here: an invoice priced purely in a
    // stablecoin has no lamport total to compare against.
    if let Some(sol) = options.iter().find(|option| option.is_sol()) {
        if total != u128::from(sol.amount) {
            return Err(bad(
                "The receipt lines do not add up to the invoice amount.",
            ));
        }
    }

    Ok(cleaned)
}

#[cfg(test)]
mod tests;
