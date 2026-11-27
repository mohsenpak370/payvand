//! The invoice details the chain does not keep, read back from our own rows.

use crate::payments;

use super::model::MyTransaction;

/// Fills in the name, the description and the receipt lines each invoice was
/// made with, from the rows written when the code was created. The chain keeps
/// none of them, so this is the only place they can come from; a read that
/// fails leaves them empty rather than failing the whole history.
pub(super) async fn fill_invoice_meta(db: &libsql::Database, items: &mut [MyTransaction]) {
    if items.is_empty() {
        return;
    }

    let ids: Vec<String> = items.iter().map(|item| item.id.clone()).collect();
    let placeholders = (1..=ids.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");

    let read = async {
        let connection = db.connect()?;
        connection
            .query(
                &format!(
                    "SELECT id, name, description, items FROM invoice_meta \
                     WHERE id IN ({placeholders})"
                ),
                libsql::params_from_iter(ids.iter().cloned()),
            )
            .await
    }
    .await;

    let Ok(mut rows) = read else {
        return;
    };

    let mut found: std::collections::HashMap<String, (String, String, Vec<payments::PaymentItem>)> =
        std::collections::HashMap::new();
    while let Ok(Some(row)) = rows.next().await {
        let Ok(id) = row.get::<String>(0) else {
            continue;
        };
        let Ok(name) = row.get::<String>(1) else {
            continue;
        };
        let Ok(description) = row.get::<String>(2) else {
            continue;
        };
        // A row written before receipts were kept has an empty string here,
        // and a malformed one is no reason to drop the name beside it.
        let items = row
            .get::<String>(3)
            .ok()
            .and_then(|json| serde_json::from_str::<Vec<payments::PaymentItem>>(&json).ok())
            .unwrap_or_default();
        found.insert(id, (name, description, items));
    }

    for item in items {
        if let Some((name, description, receipt)) = found.get(&item.id) {
            if item.name.is_empty() {
                item.name = name.clone();
            }
            if item.description.is_empty() {
                item.description = description.clone();
            }
            if item.items.is_empty() {
                item.items = receipt.clone();
            }
        }
    }
}

#[cfg(test)]
mod tests;
