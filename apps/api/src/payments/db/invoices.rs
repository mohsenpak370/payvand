//! Best-effort storage of invoices this API created.

use super::super::model::{
    PaymentOption, PaymentRecord, PaymentStatus, MAX_WATCH_PDAS, SOL_CURRENCY, SOL_DECIMALS,
    STABLE_DECIMALS,
};

/// Remembers an invoice this API made: what it is called, what it sells, and
/// everything a status read or a history row needs to rebuild it — the seller,
/// the reference, the cluster, and per address the currency, mint and amount.
///
/// The name, the description and the receipt lines exist only in the code the
/// seller shares, so the chain cannot supply them. Everything else could be read
/// back from the cluster, one `getProgramAccounts` for the whole program; this
/// row is what lets a feed ask about its own invoices instead.
///
/// A row is written for every currency the invoice accepts, not only for its
/// primary address: the history is keyed by the address that was actually paid,
/// so an invoice taking SOL and USDC paid in USDC would otherwise find nothing
/// under that PDA.
///
/// Best effort by design. The invoice already exists on chain when this runs,
/// and a cache that failed to write must never turn a working code into an
/// error.
///
/// `reusable_id` names the code that minted this invoice, when one did: a code
/// is opened far more often than it is paid, and a row that says so is what
/// keeps those unpaid lookups out of the seller's history until they settle.
pub async fn remember_invoice(
    db: &libsql::Database,
    record: &PaymentRecord,
    seller_user: Option<&str>,
    reusable_id: Option<&str>,
) {
    let name = record.name.trim().to_string();
    let description = record.description.trim().to_string();
    let items = serde_json::to_string(&record.items).unwrap_or_default();
    let now = crate::auth::unix_now();

    let mut rows: Vec<(String, String, String, u64)> = record
        .options
        .iter()
        .filter(|option| !option.payment_pda.is_empty())
        .map(|option| {
            (
                option.payment_pda.clone(),
                option.currency.clone(),
                option.mint.clone(),
                option.amount,
            )
        })
        .collect();
    // A request with no options is the older one-address shape: the record's own
    // id is the invoice, priced in SOL.
    if rows.is_empty() {
        rows.push((
            record.id.clone(),
            SOL_CURRENCY.to_string(),
            String::new(),
            record.amount_lamports,
        ));
    }

    let written = async {
        let connection = db.connect()?;
        for (id, currency, mint, amount) in &rows {
            connection
                .execute(
                    "INSERT INTO invoice_meta \
                        (id, seller, reference, cluster, endpoint, currency, mint, amount, \
                         memo, seller_user, name, description, items, reusable_id, created_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15) \
                     ON CONFLICT(id) DO UPDATE SET \
                        seller = excluded.seller, reference = excluded.reference, \
                        cluster = excluded.cluster, endpoint = excluded.endpoint, \
                        currency = excluded.currency, mint = excluded.mint, \
                        amount = excluded.amount, memo = excluded.memo, \
                        seller_user = COALESCE(excluded.seller_user, invoice_meta.seller_user), \
                        name = excluded.name, description = excluded.description, \
                        items = excluded.items, \
                        reusable_id = CASE WHEN excluded.reusable_id <> '' \
                            THEN excluded.reusable_id ELSE invoice_meta.reusable_id END",
                    libsql::params![
                        id.clone(),
                        record.seller.clone(),
                        record.reference.clone(),
                        record.cluster.clone(),
                        record.endpoint.clone(),
                        currency.clone(),
                        mint.clone(),
                        *amount as i64,
                        record.memo.clone(),
                        seller_user.map(str::to_owned),
                        name.clone(),
                        description.clone(),
                        items.clone(),
                        // Empty rather than NULL: the column is NOT NULL, and an
                        // invoice that came from no code is the ordinary case.
                        reusable_id.unwrap_or_default().to_string(),
                        now
                    ],
                )
                .await?;
        }
        Ok::<(), libsql::Error>(())
    }
    .await;

    if let Err(error) = written {
        tracing::info!(%error, "invoice row was not written");
    }
}

/// How many of a seller's unpaid reusable invoices a feed still watches. A code
/// is opened far more often than it is paid, so this is deliberately a handful:
/// the newest ones, which are the invoices a code on screen was minted into. An
/// older lookup that settles is found by the watch on its own address, the
/// buyer's poll, or the program scan.
pub const AWAITING_REUSABLE_LIMIT: usize = 32;

/// The invoices this wallet made, newest first, as far as our own rows can
/// say — no cluster read at all.
///
/// Each row is one address the invoice can be paid at, so an invoice priced in
/// SOL and USDC is two entries — the same shape a chain read returns. The facts
/// come from the row rather than from a `pending_record`, on purpose: the chain
/// answering nothing (a hiccup, an invoice not yet created) must not make an
/// invoice disappear from its own seller's history.
///
/// A reusable code mints an invoice on every visit, paid or not, so those rows
/// are history only once they settle: reloading a code page would otherwise
/// fill the seller's page with lookups and push the payments off it. An unpaid
/// one is not lost — [`reusable_invoices_awaiting`] still watches it, and a paid
/// one is listed from the load that finds it.
///
/// `amount > 0` is the line between an invoice this API recorded and a row from
/// before it did: those carry only a name and were filled from the program scan,
/// so they keep the scan as their answer until it backfills them.
pub async fn recorded_invoices(
    db: &libsql::Database,
    wallet: &str,
    cluster: &str,
    limit: usize,
) -> Vec<PaymentRecord> {
    invoice_rows(
        db,
        wallet,
        cluster,
        "AND (reusable_id = '' OR paid_at > 0 \
             OR id IN (SELECT invoice FROM settled_payments))",
        limit,
    )
    .await
}

/// The reusable invoices of this seller that have not settled: not history, but
/// worth one watch each, so a payment made while nobody was polling its status
/// is still discovered here and becomes history.
pub async fn reusable_invoices_awaiting(
    db: &libsql::Database,
    wallet: &str,
    cluster: &str,
    limit: usize,
) -> Vec<PaymentRecord> {
    invoice_rows(
        db,
        wallet,
        cluster,
        "AND reusable_id <> '' AND paid_at = 0 \
             AND id NOT IN (SELECT invoice FROM settled_payments)",
        limit,
    )
    .await
}

/// Marks reusable invoices a feed found settled, so they are history from then
/// on whether or not their settlement row was ever written: that row needs a
/// history read that can be out of budget or fail, and an unpaid visit that
/// newer visits pushed out of [`AWAITING_REUSABLE_LIMIT`] is never asked about
/// again. Best effort, like every write here — the next load marks it again.
pub async fn mark_reusable_paid(db: &libsql::Database, ids: &[String]) {
    if ids.is_empty() {
        return;
    }
    let placeholders = (2..=ids.len() + 1)
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut params: Vec<libsql::Value> = vec![crate::auth::unix_now().into()];
    params.extend(ids.iter().cloned().map(libsql::Value::from));
    let written = async {
        db.connect()?
            .execute(
                &format!(
                    "UPDATE invoice_meta SET paid_at = ?1 \
                     WHERE paid_at = 0 AND reusable_id <> '' AND id IN ({placeholders})"
                ),
                libsql::params_from_iter(params),
            )
            .await
    }
    .await;
    if let Err(error) = written {
        tracing::info!(%error, "paid reusable invoices were not marked");
    }
}

/// One seller's invoice rows, newest first, narrowed by `selection`.
///
/// `selection` is a fragment this module owns, never a request value, so it is
/// written into the statement rather than bound — like the limit.
async fn invoice_rows(
    db: &libsql::Database,
    wallet: &str,
    cluster: &str,
    selection: &str,
    limit: usize,
) -> Vec<PaymentRecord> {
    let read = async {
        let connection = db.connect()?;
        connection
            .query(
                // The limit is a number this module owns, so it is written into
                // the statement rather than bound: a bound LIMIT is read as a
                // string by some drivers, which is an error rather than a page.
                &format!(
                    "SELECT id, seller, reference, endpoint, currency, mint, amount, memo, created_at \
                     FROM invoice_meta \
                     WHERE seller = ?1 AND cluster = ?2 AND amount > 0 {selection} \
                     ORDER BY created_at DESC LIMIT {}",
                    limit.min(MAX_WATCH_PDAS * 4)
                ),
                (wallet.to_string(), cluster.to_string()),
            )
            .await
    }
    .await;

    let mut rows = match read {
        Ok(rows) => rows,
        Err(error) => {
            tracing::info!(%error, "recorded invoices were not readable");
            return Vec::new();
        }
    };
    let mut records = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        let Ok(id) = row.get::<String>(0) else {
            continue;
        };
        let seller = row.get::<String>(1).unwrap_or_default();
        let reference = row.get::<String>(2).unwrap_or_default();
        let endpoint = row.get::<String>(3).unwrap_or_default();
        let currency = row.get::<String>(4).unwrap_or_default();
        let mint = row.get::<String>(5).unwrap_or_default();
        let amount = row.get::<i64>(6).unwrap_or(0).max(0) as u64;
        let memo = row.get::<String>(7).unwrap_or_default();
        let created_at = row.get::<i64>(8).unwrap_or(0);
        let is_sol = currency == SOL_CURRENCY;
        records.push(PaymentRecord {
            id: id.clone(),
            status: PaymentStatus::Pending,
            payment_pda: id.clone(),
            seller,
            seller_user: None,
            reference,
            amount_lamports: if is_sol { amount } else { 0 },
            options: vec![PaymentOption {
                currency,
                mint,
                decimals: if is_sol {
                    SOL_DECIMALS
                } else {
                    STABLE_DECIMALS
                },
                amount,
                payment_pda: id,
            }],
            memo,
            name: String::new(),
            description: String::new(),
            items: Vec::new(),
            discount: None,
            allow_discounts: false,
            cluster: cluster.to_string(),
            endpoint,
            settled_by: None,
            created_at_unix: created_at,
            settled_at_unix: 0,
            fee: None,
        });
    }
    records
}

/// Writes what a fallback scan found, so the next request is answered from our
/// own rows: the fallback is a one-off cost per invoice, not a per-request one.
///
/// `INSERT OR IGNORE` on purpose — a row this API wrote carries a seller's name,
/// description and receipt lines, and a scan cannot improve on any of them. The
/// facts it does add are the ones the old rows were missing.
pub async fn remember_scanned(db: &libsql::Database, records: &[PaymentRecord]) {
    let now = crate::auth::unix_now();
    let written = async {
        let connection = db.connect()?;
        for record in records {
            let mut rows: Vec<(String, String, String, u64)> = record
                .options
                .iter()
                .filter(|option| !option.payment_pda.is_empty())
                .map(|option| {
                    (
                        option.payment_pda.clone(),
                        option.currency.clone(),
                        option.mint.clone(),
                        option.amount,
                    )
                })
                .collect();
            if rows.is_empty() {
                rows.push((
                    record.id.clone(),
                    SOL_CURRENCY.to_string(),
                    String::new(),
                    record.amount_lamports,
                ));
            }
            for (id, currency, mint, amount) in &rows {
                connection
                    .execute(
                        "INSERT OR IGNORE INTO invoice_meta \
                            (id, seller, reference, cluster, endpoint, currency, mint, amount, \
                             memo, seller_user, name, description, items, created_at) \
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, '', '', '', ?10)",
                        libsql::params![
                            id.clone(),
                            record.seller.clone(),
                            record.reference.clone(),
                            record.cluster.clone(),
                            record.endpoint.clone(),
                            currency.clone(),
                            mint.clone(),
                            *amount as i64,
                            record.memo.clone(),
                            now
                        ],
                    )
                    .await?;
            }
        }
        Ok::<(), libsql::Error>(())
    }
    .await;

    if let Err(error) = written {
        tracing::info!(%error, "scanned invoices were not recorded");
    }
}

#[cfg(test)]
mod tests;
