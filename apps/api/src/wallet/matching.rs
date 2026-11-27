//! Which payments are this wallet's, and which way the money went.

use crate::payments::{BudgetAccount, PaymentRecord};

use super::model::MyTransaction;

/// Pure part of the transactions feed: keep the invoices this wallet is
/// party to, label the direction, newest first.
///
/// An allowance path records the *session key* as the payer, so a payment made
/// silently from this buyer's allowance counts as theirs too — which is why the
/// budget accounts come along. There is one allowance per currency, so every
/// session key this buyer owns counts, not just the first.
pub(super) fn match_transactions(
    records: Vec<PaymentRecord>,
    budgets: &[BudgetAccount],
    wallet: &str,
) -> Vec<MyTransaction> {
    let sessions: Vec<String> = budgets
        .iter()
        .filter(|budget| budget.buyer.to_string() == wallet)
        .map(|budget| budget.session_key.to_string())
        .collect();

    let mut items: Vec<MyTransaction> = records
        .into_iter()
        .filter_map(|record| {
            let is_seller = record.seller == wallet;
            let payer = record.settled_by.as_deref();
            let is_payer = payer == Some(wallet)
                || payer.is_some_and(|paid| sessions.iter().any(|s| s.as_str() == paid));
            if !is_seller && !is_payer {
                return None;
            }

            let direction = if is_seller { "received" } else { "sent" };
            let counterparty = if is_seller {
                record
                    .settled_by
                    .clone()
                    .unwrap_or_else(|| "awaiting payment".to_string())
            } else {
                record.seller.clone()
            };

            // A settled record carries the one option it was paid with; a
            // marker read before its settle transaction is resolved carries
            // none, and then the SOL price is all there is to show.
            let (currency, amount, decimals) = record
                .options
                .first()
                .map(|option| (option.currency.clone(), option.amount, option.decimals))
                .unwrap_or_else(|| ("sol".to_string(), record.amount_lamports, 9));

            Some(MyTransaction {
                id: record.id,
                direction: direction.to_string(),
                counterparty,
                currency,
                amount,
                decimals,
                amount_lamports: record.amount_lamports,
                memo: record.memo,
                name: record.name,
                description: record.description,
                // The chain keeps no receipt lines; `fill_invoice_meta` puts
                // back whatever the seller typed when the code was made.
                items: Vec::new(),
                status: record.status,
                created_at_unix: record.created_at_unix,
                settled_at_unix: record.settled_at_unix,
            })
        })
        .collect();

    items.sort_by(|a, b| {
        b.settled_at_unix
            .cmp(&a.settled_at_unix)
            .then_with(|| b.created_at_unix.cmp(&a.created_at_unix))
            .then_with(|| a.id.cmp(&b.id))
    });
    items
}

#[cfg(test)]
mod tests;
