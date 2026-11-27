//! Completes legacy invoice rows from the chain, once.

use super::super::{
    model::{PaymentRecord, MAX_WATCH_PDAS},
    watch::fetch_many_at,
};

/// What an invoice row from before this API kept amounts is missing, filled in
/// from the invoice or its marker.
///
/// The row is only ever completed, never rewritten: `amount = 0` is what marks a
/// row as unfilled, so the statement is a no-op for every row written since, and
/// the seller's own name, description and receipt lines are left exactly as they
/// were. Being one statement per row also makes a retry harmless.
async fn fill_invoice_row(db: &libsql::Database, record: &PaymentRecord) -> bool {
    let Some(option) = record
        .options
        .iter()
        .find(|option| option.amount > 0)
        .or_else(|| record.options.first())
    else {
        return false;
    };
    let written = async {
        let connection = db.connect()?;
        let changed = connection
            .execute(
                "UPDATE invoice_meta SET \
                     reference = CASE WHEN reference = '' THEN ?2 ELSE reference END, \
                     cluster   = CASE WHEN cluster   = '' THEN ?3 ELSE cluster   END, \
                     endpoint  = CASE WHEN endpoint  = '' THEN ?4 ELSE endpoint  END, \
                     currency  = ?5, mint = ?6, amount = ?7, \
                     memo      = CASE WHEN memo = '' THEN ?8 ELSE memo END \
                 WHERE id = ?1 AND amount = 0",
                libsql::params![
                    record.id.clone(),
                    record.reference.clone(),
                    record.cluster.clone(),
                    record.endpoint.clone(),
                    option.currency.clone(),
                    option.mint.clone(),
                    option.amount as i64,
                    record.memo.clone(),
                ],
            )
            .await?;
        Ok::<u64, crate::db::DbError>(changed)
    }
    .await;
    match written {
        Ok(changed) => changed > 0,
        Err(error) => {
            tracing::info!(%error, "a legacy invoice row was not filled");
            false
        }
    }
}

/// The rows this seller made before a row carried an amount, oldest first — the
/// ones the feed cannot list and so has to scan for. Bounded like any watch
/// request, and ids the chain already gave no answer for are left out.
async fn legacy_invoice_ids(db: &libsql::Database, wallet: &str) -> Vec<String> {
    let read = async {
        let connection = db.connect()?;
        connection
            .query(
                &format!(
                    "SELECT id FROM invoice_meta \
                     WHERE seller = ?1 AND amount = 0 \
                       AND id NOT IN (SELECT id FROM invoice_backfill) \
                     ORDER BY created_at ASC LIMIT {MAX_WATCH_PDAS}"
                ),
                libsql::params![wallet.to_string()],
            )
            .await
    }
    .await;

    let Ok(mut rows) = read else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        if let Ok(id) = row.get::<String>(0) {
            ids.push(id);
        }
    }
    ids
}

/// Remembers the ids the chain had nothing to say about, so they are not asked
/// about again.
async fn remember_backfill_tried(db: &libsql::Database, ids: &[String], endpoint: &str) {
    if ids.is_empty() {
        return;
    }
    let now = crate::auth::unix_now();
    let written = async {
        let connection = db.connect()?;
        for id in ids {
            connection
                .execute(
                    "INSERT OR IGNORE INTO invoice_backfill (id, endpoint, tried_at) \
                     VALUES (?1, ?2, ?3)",
                    libsql::params![id.clone(), endpoint.to_string(), now],
                )
                .await?;
        }
        Ok::<(), crate::db::DbError>(())
    }
    .await;
    if let Err(error) = written {
        tracing::info!(%error, "a backfill attempt was not recorded");
    }
}

/// Fills this seller's rows from before amounts were kept, from the chain.
///
/// Without it a returning seller's feed has no row it can list — every row it
/// has is one the read skips — so the program scan becomes the only thing that
/// answers, on every load. One batched read fills the page once; after that the
/// rows carry what the feed needs and the scan is behind `?scan=1` again.
///
/// A read that fails changes nothing, so the next load tries again; a row the
/// chain does not know is remembered and left alone.
pub async fn backfill_invoice_rows(
    db: &libsql::Database,
    wallet: &str,
    cluster: &str,
    endpoint: &str,
) {
    let ids = legacy_invoice_ids(db, wallet).await;
    if ids.is_empty() {
        return;
    }
    // The endpoint is the one the caller already checked against the allowlist,
    // so this is the read itself rather than a request that resolves one.
    let records = match fetch_many_at(endpoint, cluster, &ids, db).await {
        Ok(records) => records,
        Err(error) => {
            tracing::info!(%error, "legacy invoices not readable");
            return;
        }
    };

    let mut unknown = Vec::new();
    for id in &ids {
        match records.iter().find(|record| &record.id == id) {
            Some(record) if record.options.iter().any(|option| option.amount > 0) => {
                fill_invoice_row(db, record).await;
            }
            _ => unknown.push(id.clone()),
        }
    }
    remember_backfill_tried(db, &unknown, endpoint).await;
}

#[cfg(test)]
mod tests;
