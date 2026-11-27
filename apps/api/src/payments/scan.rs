//! Program-wide scan used as a fallback for history.

use solana_pubkey::Pubkey;
use std::str::FromStr;

use super::{
    cache::{
        scan::{cached_scan, remember_scan},
        settled::{cached_settlement, remember_settlement},
    },
    config::{default_cluster, payment_rpc, resolve_endpoint, PAYVAND_PROGRAM_ID},
    db::settlements::{store_settlements, stored_settlements},
    error::{ApiError, ApiResult},
    model::{BudgetAccount, PaymentRecord},
    onchain::{
        classify::{classify_program_account, ScannedAccount},
        records::{marker_record, receipt_record},
    },
    settlement::{settlements_from_history, HistoryBudget},
};

/// Every payment account the cluster holds for this program, undecodable
/// accounts skipped.
pub struct AllPayments {
    pub records: Vec<PaymentRecord>,
    pub cluster: String,
    pub endpoint: String,
}

/// The two account kinds the program owns that a scan must tell apart:
/// invoices, and the monthly budgets that can pay them.
pub struct ProgramScan {
    pub payments: Vec<PaymentRecord>,
    pub budgets: Vec<BudgetAccount>,
    pub cluster: String,
    pub endpoint: String,
}

/// Invoices live only on chain — the API keeps no copy — so listing them
/// means asking the program for its accounts (`get_program_accounts`).
/// That scan is fine while the platform is small; revisit if the account
/// count ever grows into the thousands.
///
/// `history_reads` is how many settled invoices this caller can actually show
/// (a page size): those are resolved from their settle transactions, newest
/// first, and every other marker is answered from itself. A cache hit costs
/// nothing, so a warm instance resolves none of them.
pub async fn fetch_program_scan(
    history_reads: usize,
    db: &libsql::Database,
) -> ApiResult<ProgramScan> {
    let cluster = default_cluster();
    let endpoint = resolve_endpoint(None, &cluster)?;
    program_scan_at(&endpoint, &cluster, history_reads, db).await
}

/// The scan itself, from an endpoint already checked against the allowlist.
pub(crate) async fn program_scan_at(
    endpoint: &str,
    cluster: &str,
    history_reads: usize,
    db: &libsql::Database,
) -> ApiResult<ProgramScan> {
    let program_id = Pubkey::from_str(PAYVAND_PROGRAM_ID)
        .map_err(|error| ApiError::Internal(format!("bad program id: {error}")))?;

    // A scan a moment ago answers this one: the fallback is the cold path, but
    // three tabs and the admin list still make a burst of the same read.
    let accounts = match cached_scan(endpoint, cluster) {
        Some(accounts) => accounts,
        None => {
            // `confirmed`, not the `finalized` a bare `RpcClient::new` reads at:
            // an invoice is created, and a receipt written, in transactions the
            // wallet has already seen confirmed, so a finalized scan left a
            // payment that had just gone through out of the history for another
            // dozen seconds.
            let rpc = payment_rpc(endpoint.to_owned());
            let read = match rpc.get_program_accounts(&program_id).await {
                Ok(accounts) => accounts,
                Err(error) => {
                    tracing::info!(%error, "program accounts not readable");
                    // A failure is not a scan: it is not remembered, so the next
                    // request asks again rather than serving an empty program.
                    return Err(ApiError::Internal(
                        "The cluster did not return the payment list. Try again.".into(),
                    ));
                }
            };
            let accounts: Vec<(Pubkey, Vec<u8>)> = read
                .into_iter()
                .map(|(pubkey, account)| (pubkey, account.data))
                .collect();
            remember_scan(endpoint, cluster, &accounts);
            accounts
        }
    };

    let mut payments = Vec::new();
    let mut budgets = Vec::new();
    // Markers are collected rather than resolved where they are found: the
    // budget has to go to the newest settlements, and the account order a scan
    // returns is arbitrary.
    let mut markers = Vec::new();
    for (pubkey, data) in accounts {
        match classify_program_account(pubkey, &data, cluster.to_owned(), endpoint.to_owned()) {
            Some(ScannedAccount::Payment(record)) => payments.push(*record),
            Some(ScannedAccount::Settled(marker)) => markers.push(marker),
            Some(ScannedAccount::Budget(budget)) => budgets.push(budget),
            None => {}
        }
    }

    markers.sort_by_key(|marker| std::cmp::Reverse(marker.settled_at));

    // Every marker in one database read, and the ones history has to answer for
    // a few at a time. Serial — one remote query per marker, then two paced RPC
    // calls behind each miss — a page of twenty settlements took half a minute.
    let stored = stored_settlements(db, endpoint, &markers).await;
    let mut budget = HistoryBudget::new(history_reads);
    let mut unresolved = Vec::new();
    for marker in markers {
        if let Some(cached) = cached_settlement(endpoint, &marker.invoice) {
            payments.push(receipt_record(
                cached,
                cluster.to_owned(),
                endpoint.to_owned(),
            ));
        } else if let Some(known) = stored.get(&marker.invoice) {
            remember_settlement(endpoint, marker.invoice, known);
            payments.push(receipt_record(
                known.clone(),
                cluster.to_owned(),
                endpoint.to_owned(),
            ));
        } else if budget.take() {
            unresolved.push(marker);
        } else {
            payments.push(marker_record(
                marker,
                cluster.to_owned(),
                endpoint.to_owned(),
            ));
        }
    }

    let invoices: Vec<Pubkey> = unresolved.iter().map(|marker| marker.invoice).collect();
    let resolved = settlements_from_history(endpoint, &invoices).await;
    let mut fresh = Vec::new();
    for marker in unresolved {
        match resolved.get(&marker.invoice) {
            Some(receipt) => {
                remember_settlement(endpoint, marker.invoice, receipt);
                fresh.push(receipt.clone());
            }
            None => payments.push(marker_record(
                marker,
                cluster.to_owned(),
                endpoint.to_owned(),
            )),
        }
    }
    store_settlements(db, endpoint, &fresh).await;
    payments.extend(
        fresh
            .into_iter()
            .map(|receipt| receipt_record(receipt, cluster.to_owned(), endpoint.to_owned())),
    );

    Ok(ProgramScan {
        payments,
        budgets,
        cluster: cluster.to_owned(),
        endpoint: endpoint.to_owned(),
    })
}

/// The payment half of a program scan (the admin feed needs nothing else).
pub async fn fetch_all_payments(
    history_reads: usize,
    db: &libsql::Database,
) -> ApiResult<AllPayments> {
    let scan = fetch_program_scan(history_reads, db).await?;
    Ok(AllPayments {
        records: scan.payments,
        cluster: scan.cluster,
        endpoint: scan.endpoint,
    })
}

#[cfg(test)]
mod tests;
