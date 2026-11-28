//! Settlement rows: durable history and best-effort writes.

use solana_pubkey::Pubkey;
use std::collections::HashMap;
use std::str::FromStr;

use super::super::{
    model::{PaymentRecord, MAX_WATCH_PDAS},
    onchain::{
        accounts::{OnchainReceipt, ReceiptMarker},
        records::receipt_record,
    },
};

/// The settlements this wallet paid, newest first.
///
/// These are rows history already resolved: the payer is only known from the
/// settle transaction, so an invoice this wallet bought is here rather than in
/// the invoice rows it did not make. No cluster read is needed to answer.
pub async fn settlements_paid_by(
    db: &libsql::Database,
    endpoint: &str,
    cluster: &str,
    wallet: &str,
    limit: usize,
) -> Vec<PaymentRecord> {
    settlements_paid_by_any(db, endpoint, cluster, &[wallet.to_string()], limit).await
}

/// The same rows for every key that can stand for one buyer: the wallet itself,
/// and the session keys its allowances pay with.
///
/// A silent payment is signed by the session key, so its row is keyed by that
/// address. The mapping behind the extra keys is in `session_keys` — learned
/// from a budget account once, never scanned for on the usual request.
pub async fn settlements_paid_by_any(
    db: &libsql::Database,
    endpoint: &str,
    cluster: &str,
    payers: &[String],
    limit: usize,
) -> Vec<PaymentRecord> {
    if payers.is_empty() {
        return Vec::new();
    }
    // One placeholder per payer: the wallet first, then its session keys.
    let placeholders = (0..payers.len())
        .map(|index| format!("?{}", index + 2))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT invoice, seller, reference, mint, amount, memo, payer, created_at, settled_at, fee \
         FROM settled_payments WHERE endpoint = ?1 AND payer IN ({placeholders}) \
         ORDER BY settled_at DESC LIMIT {}",
        limit.min(MAX_WATCH_PDAS * 4)
    );
    let read = async {
        let connection = db.connect()?;
        let mut values: Vec<libsql::Value> = Vec::with_capacity(payers.len() + 1);
        values.push(libsql::Value::Text(endpoint.to_string()));
        values.extend(
            payers
                .iter()
                .map(|payer| libsql::Value::Text(payer.clone())),
        );
        connection.query(&sql, values).await
    }
    .await;

    let mut rows = match read {
        Ok(rows) => rows,
        Err(error) => {
            tracing::info!(%error, "stored settlements were not readable");
            return Vec::new();
        }
    };
    let mut records = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        let Ok(invoice) = row.get::<String>(0) else {
            continue;
        };
        let Ok(invoice) = Pubkey::from_str(&invoice) else {
            continue;
        };
        let seller = row.get::<String>(1).unwrap_or_default();
        let reference = row.get::<String>(2).unwrap_or_default();
        let mint = row.get::<String>(3).unwrap_or_default();
        let amount = row.get::<i64>(4).unwrap_or(0);
        let memo = row.get::<String>(5).unwrap_or_default();
        let payer = row.get::<String>(6).unwrap_or_default();
        let created_at = row.get::<i64>(7).unwrap_or(0);
        let settled_at = row.get::<i64>(8).unwrap_or(0);
        let fee = stored_fee(&row);
        records.push(receipt_record(
            OnchainReceipt {
                invoice,
                seller: Pubkey::from_str(&seller).unwrap_or_default(),
                reference: Pubkey::from_str(&reference).unwrap_or_default(),
                mint: Pubkey::from_str(&mint).unwrap_or_default(),
                amount: amount.max(0) as u64,
                memo,
                payer: Pubkey::from_str(&payer).unwrap_or_default(),
                created_at,
                settled_at,
                fee,
            },
            cluster.to_string(),
            endpoint.to_string(),
        ));
    }
    records
}

/// The settlement as the database remembers it, for an instance that has never
/// seen this invoice.
///
/// `None` covers both "not stored yet" and "the database did not answer": this
/// is a cache in front of the chain, so a miss or a hiccup falls through to
/// history rather than failing a status read.
pub(crate) async fn stored_settlement(
    db: &libsql::Database,
    endpoint: &str,
    invoice: &Pubkey,
) -> Option<OnchainReceipt> {
    let marker = ReceiptMarker {
        invoice: *invoice,
        settled_at: 0,
    };
    stored_settlements(db, endpoint, std::slice::from_ref(&marker))
        .await
        .remove(invoice)
}

/// The settlements the database already knows, for every marker in one read.
///
/// A scan used to ask the database once per settlement, which on a remote
/// database is a round trip each: with the chain reads behind them, that is
/// what made a cold history slow.
///
/// An empty map covers both "none stored yet" and "the database did not
/// answer": this is a cache in front of the chain, so a miss falls through to
/// history rather than failing the scan.
pub(crate) async fn stored_settlements(
    db: &libsql::Database,
    endpoint: &str,
    markers: &[ReceiptMarker],
) -> HashMap<Pubkey, OnchainReceipt> {
    let mut found = HashMap::new();
    if markers.is_empty() {
        return found;
    }
    let Ok(connection) = db.connect() else {
        return found;
    };

    let placeholders = (2..=markers.len() + 1)
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut params: Vec<libsql::Value> = Vec::with_capacity(markers.len() + 1);
    params.push(libsql::Value::Text(endpoint.to_owned()));
    params.extend(
        markers
            .iter()
            .map(|marker| libsql::Value::Text(marker.invoice.to_string())),
    );

    let read = connection
        .query(
            &format!(
                "SELECT invoice, seller, reference, mint, amount, memo, payer, created_at, settled_at, fee \
                 FROM settled_payments WHERE endpoint = ?1 AND invoice IN ({placeholders})"
            ),
            params,
        )
        .await;

    let Ok(mut rows) = read else {
        return found;
    };
    while let Ok(Some(row)) = rows.next().await {
        let Ok(invoice) = row.get::<String>(0) else {
            continue;
        };
        let Ok(invoice) = Pubkey::from_str(&invoice) else {
            continue;
        };
        if let Some(receipt) = receipt_from_row(&row, invoice) {
            found.insert(invoice, receipt);
        }
    }
    found
}

/// One `settled_payments` row as the receipt it was written from.
fn receipt_from_row(row: &libsql::Row, invoice: Pubkey) -> Option<OnchainReceipt> {
    Some(OnchainReceipt {
        invoice,
        seller: Pubkey::from_str(&row.get::<String>(1).ok()?).ok()?,
        reference: Pubkey::from_str(&row.get::<String>(2).ok()?).ok()?,
        mint: Pubkey::from_str(&row.get::<String>(3).ok()?).ok()?,
        amount: u64::try_from(row.get::<i64>(4).ok()?).ok()?,
        memo: row.get::<String>(5).ok()?,
        payer: Pubkey::from_str(&row.get::<String>(6).ok()?).ok()?,
        created_at: row.get::<i64>(7).ok()?,
        settled_at: row.get::<i64>(8).ok()?,
        fee: stored_fee(row),
    })
}

/// The `fee` column: NULL on a row stored before the fee was kept.
fn stored_fee(row: &libsql::Row) -> Option<u64> {
    row.get::<Option<i64>>(9)
        .ok()
        .flatten()
        .and_then(|fee| u64::try_from(fee).ok())
}

/// Remembers one settlement in the database, so the history read that produced
/// it happens once per invoice per cluster instead of once per cold instance.
///
/// Best effort on purpose: the settlement is already known and the caller has
/// its record. Failing the request because a cache write failed would be worse
/// than reading history one more time on a later poll.
pub(crate) async fn store_settlement(
    db: &libsql::Database,
    endpoint: &str,
    settled: &OnchainReceipt,
) {
    store_settlements(db, endpoint, std::slice::from_ref(settled)).await;
}

/// Remembers a batch of settlements in one statement, for the scan that reads
/// many at once. Same best-effort rule as one row at a time.
pub(crate) async fn store_settlements(
    db: &libsql::Database,
    endpoint: &str,
    settled: &[OnchainReceipt],
) {
    if settled.is_empty() {
        return;
    }
    let Ok(connection) = db.connect() else {
        return;
    };

    let mut rows = Vec::with_capacity(settled.len());
    const COLUMNS: usize = 11;
    let mut params: Vec<libsql::Value> = Vec::with_capacity(settled.len() * COLUMNS);
    for (index, receipt) in settled.iter().enumerate() {
        let at = index * COLUMNS;
        let placeholders = (1..=COLUMNS)
            .map(|column| format!("?{}", at + column))
            .collect::<Vec<_>>()
            .join(", ");
        rows.push(format!("({placeholders})"));
        params.extend([
            libsql::Value::Text(receipt.invoice.to_string()),
            libsql::Value::Text(endpoint.to_owned()),
            libsql::Value::Text(receipt.seller.to_string()),
            libsql::Value::Text(receipt.reference.to_string()),
            libsql::Value::Text(receipt.mint.to_string()),
            libsql::Value::Integer(receipt.amount as i64),
            libsql::Value::Text(receipt.memo.clone()),
            libsql::Value::Text(receipt.payer.to_string()),
            libsql::Value::Integer(receipt.created_at),
            libsql::Value::Integer(receipt.settled_at),
            match receipt.fee {
                Some(fee) => libsql::Value::Integer(fee as i64),
                None => libsql::Value::Null,
            },
        ]);
    }

    let written = connection
        .execute(
            &format!(
                "INSERT INTO settled_payments \
                    (invoice, endpoint, seller, reference, mint, amount, memo, payer, created_at, settled_at, fee) \
                 VALUES {} \
                 ON CONFLICT(invoice, endpoint) DO UPDATE SET \
                    seller = excluded.seller, reference = excluded.reference, \
                    mint = excluded.mint, amount = excluded.amount, \
                    memo = excluded.memo, payer = excluded.payer, \
                    created_at = excluded.created_at, settled_at = excluded.settled_at, \
                    fee = COALESCE(excluded.fee, settled_payments.fee)",
                rows.join(", ")
            ),
            params,
        )
        .await;
    if let Err(error) = written {
        tracing::info!(%error, "settlements were not cached");
    }
}

#[cfg(test)]
mod tests;
