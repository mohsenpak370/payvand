//! Single-invoice status reads and the per-request reader.

use serde::Deserialize;
use solana_pubkey::Pubkey;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;

use super::{
    cache::pending::{cached_pending, remember_pending},
    config::{default_cluster, payment_rpc, program_id, resolve_endpoint},
    error::{parse_pubkey, ApiError, ApiResult},
    model::PaymentRecord,
    onchain::{
        accounts::{decode_receipt_marker, ReceiptMarker},
        records::{pending_record, record_for},
    },
    pda::receipt_pda,
    settlement::{settled_record, HistoryBudget},
};

/// `?cluster=devnet` / `?endpoint=...` on `GET /api/payments/{id}`.
#[derive(Debug, Default, Deserialize)]
pub struct PaymentQuery {
    pub cluster: Option<String>,
    pub endpoint: Option<String>,
}

/// Reads the payment account straight from the cluster. The handler is
/// stateless so it behaves the same locally and inside a serverless isolate.
pub async fn fetch_payment(
    payment_pda: &str,
    query: PaymentQuery,
    db: &libsql::Database,
) -> ApiResult<PaymentRecord> {
    let pubkey = parse_pubkey(payment_pda, "id")?;
    let cluster = query.cluster.clone().unwrap_or_else(default_cluster);
    let endpoint = resolve_endpoint(query.endpoint.as_deref(), &cluster)?;
    fetch_payment_at(&endpoint, &cluster, pubkey, db).await
}

/// The read itself, from an endpoint already checked against the allowlist —
/// which is what lets a test point it at a stand-in cluster and count what it
/// asked.
pub(crate) async fn fetch_payment_at(
    endpoint: &str,
    cluster: &str,
    pubkey: Pubkey,
    db: &libsql::Database,
) -> ApiResult<PaymentRecord> {
    // A page polls this address every five seconds, and the same invoice is
    // open in a second tab and on the seller's screen: one recent answer serves
    // all of them, and the cluster is asked once per address per window instead
    // of once per viewer per tick.
    if let Some(cached) = cached_pending(endpoint, &pubkey) {
        return Ok(cached);
    }

    let program = program_id()?;
    let receipt_address = receipt_pda(&program, &pubkey);

    // The invoice and its receipt are two different addresses, and only one of
    // them exists at a time, so ask for both in one round trip.
    let rpc = payment_rpc(endpoint.to_owned());
    let accounts = match rpc.get_multiple_accounts(&[pubkey, receipt_address]).await {
        Ok(accounts) => accounts,
        Err(error) => {
            tracing::info!(invoice = %pubkey, %error, "payment account not readable");
            // A cluster hiccup is not "unpaid": say so instead of inventing a
            // status, so the caller can retry and keep its last known state.
            return Err(ApiError::Internal(
                "The cluster did not answer. Try again.".into(),
            ));
        }
    };
    let invoice_data = accounts
        .first()
        .and_then(|account| account.as_ref())
        .map(|account| account.data.as_slice());
    let receipt_data = accounts
        .get(1)
        .and_then(|account| account.as_ref())
        .map(|account| account.data.as_slice());

    // One address is one invoice, so one history read is the whole need: a
    // settled invoice the cache has not seen yet is read once, here.
    let mut reader = InvoiceReader::new(&rpc, db, cluster.to_owned(), endpoint.to_owned(), 1);
    let record = reader.record(pubkey, invoice_data, receipt_data).await;
    // A failure above returned already, so what is remembered here is always a
    // status: settled goes to the cache that never expires, and anything else
    // gets the short window.
    remember_pending(endpoint, pubkey, &record);
    Ok(record)
}

/// The best record for one invoice address: the settlement behind its marker if
/// it was paid, the invoice account itself if it still exists, and otherwise the
/// placeholder a watcher reads as "not paid yet".
///
/// Everything one request's invoice reads share: the cluster to ask, the cache
/// behind the chain, and the history this request may still spend. `fetch_many`
/// holds one across a whole batch, so its budget covers the poll rather than
/// each address.
pub(crate) struct InvoiceReader<'a> {
    rpc: &'a RpcClient,
    db: &'a libsql::Database,
    cluster: String,
    endpoint: String,
    budget: HistoryBudget,
}

impl<'a> InvoiceReader<'a> {
    /// `wanted` is how many settlements this caller can show; the budget caps
    /// it, and a cache hit spends nothing.
    pub(crate) fn new(
        rpc: &'a RpcClient,
        db: &'a libsql::Database,
        cluster: String,
        endpoint: String,
        wanted: usize,
    ) -> Self {
        Self {
            rpc,
            db,
            cluster,
            endpoint,
            budget: HistoryBudget::new(wanted),
        }
    }

    /// The record for one settlement marker. A marker alone no longer describes
    /// a payment, so this needs the settle transaction — read at most once per
    /// invoice, and within this request's budget. When neither memory, the
    /// database nor history can answer, the marker's own answer (settled, and
    /// which invoice it closed) is still returned: the cluster settled it, and a
    /// flaky node must not turn that back into "pending".
    pub(crate) async fn settled(&mut self, marker: ReceiptMarker) -> PaymentRecord {
        settled_record(
            self.rpc,
            self.db,
            marker,
            self.cluster.clone(),
            self.endpoint.clone(),
            &mut self.budget,
        )
        .await
    }

    /// The best record for one invoice address: the settlement behind its marker
    /// if it was paid, the invoice account itself if it still exists, and
    /// otherwise the placeholder a watcher reads as "not paid yet".
    pub(crate) async fn record(
        &mut self,
        invoice: Pubkey,
        invoice_data: Option<&[u8]>,
        receipt_data: Option<&[u8]>,
    ) -> PaymentRecord {
        if let Some(marker) = receipt_data.and_then(decode_receipt_marker) {
            return self.settled(marker).await;
        }
        match invoice_data
            .and_then(|data| record_for(invoice, data, self.cluster.clone(), self.endpoint.clone()))
        {
            Some(record) => record,
            None => pending_record(invoice, self.cluster.clone(), self.endpoint.clone()),
        }
    }
}

#[cfg(test)]
mod tests;
