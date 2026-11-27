//! Batched watch reads for a seller's screen.

use serde::Deserialize;
use solana_pubkey::Pubkey;
use std::collections::HashMap;

use super::{
    cache::pending::{cached_pending, remember_pending},
    config::{default_cluster, payment_rpc, program_id, resolve_endpoint},
    error::{bad, parse_pubkey, ApiError, ApiResult},
    model::{PaymentRecord, MAX_WATCH_PDAS},
    pda::receipt_pda,
    status::InvoiceReader,
};

/// Accounts one `getMultipleAccounts` request may name, a JSON-RPC limit. Each
/// watched invoice consumes two keys (invoice and receipt), so the watch read
/// batches at half of this.
const MAX_ACCOUNTS_PER_RPC: usize = 100;

/// `POST /api/payments/watch`: several payment addresses in one round trip.
#[derive(Debug, Deserialize)]
pub struct WatchRequest {
    pub pdas: Vec<String>,
    pub cluster: Option<String>,
    pub endpoint: Option<String>,
}

/// Reads many payment accounts in a single RPC round trip so the seller's
/// screen can watch the invoice address and every discounted variant at once.
/// Addresses with no receipt and no invoice account come back as `Pending`,
/// the normal state of a QR nobody has paid yet. A cluster that does not answer
/// is an error, not a status, so the seller's screen keeps its last known one.
pub async fn fetch_many(
    request: WatchRequest,
    db: &libsql::Database,
) -> ApiResult<Vec<PaymentRecord>> {
    if request.pdas.is_empty() {
        return Ok(Vec::new());
    }
    if request.pdas.len() > MAX_WATCH_PDAS {
        return Err(bad(format!(
            "at most {MAX_WATCH_PDAS} addresses per watch request"
        )));
    }

    let cluster = request.cluster.clone().unwrap_or_else(default_cluster);
    let endpoint = resolve_endpoint(request.endpoint.as_deref(), &cluster)?;
    fetch_many_at(&endpoint, &cluster, &request.pdas, db).await
}

/// The batch itself, from an endpoint already checked against the allowlist.
pub(crate) async fn fetch_many_at(
    endpoint: &str,
    cluster: &str,
    pdas: &[String],
    db: &libsql::Database,
) -> ApiResult<Vec<PaymentRecord>> {
    let pubkeys = pdas
        .iter()
        .map(|pda| parse_pubkey(pda, "pdas"))
        .collect::<ApiResult<Vec<_>>>()?;

    let program = program_id()?;
    // `confirmed`, like every other invoice read: a feed that waited for
    // `finalized` would show a payment a dozen seconds after the buyer saw it.
    let rpc = payment_rpc(endpoint.to_owned());
    // The seller's screen watches the very addresses the buyer's page is polling,
    // so a recent answer is shared rather than asked for again; what is left is
    // batched into one round trip as before.
    let mut answered: HashMap<Pubkey, PaymentRecord> = HashMap::new();
    let mut unresolved = Vec::new();
    for pubkey in &pubkeys {
        match cached_pending(endpoint, pubkey) {
            Some(record) => {
                answered.insert(*pubkey, record);
            }
            None => unresolved.push(*pubkey),
        }
    }
    // A watched address is an invoice the caller is about to display, so it may
    // be resolved from history — but only once: the next poll is a cache hit.
    let mut reader = InvoiceReader::new(
        &rpc,
        db,
        cluster.to_owned(),
        endpoint.to_owned(),
        pubkeys.len(),
    );

    // Two accounts per invoice (the invoice and its receipt), and JSON-RPC caps
    // one `getMultipleAccounts` call at 100 keys, so batch the addresses.
    for batch in unresolved.chunks(MAX_WATCH_PDAS.min(MAX_ACCOUNTS_PER_RPC / 2)) {
        let mut keys = Vec::with_capacity(batch.len() * 2);
        for invoice in batch {
            keys.push(*invoice);
            keys.push(receipt_pda(&program, invoice));
        }

        let accounts = match rpc.get_multiple_accounts(&keys).await {
            Ok(accounts) => accounts,
            Err(error) => {
                tracing::info!(%error, "payment accounts not readable");
                return Err(ApiError::Internal(
                    "The cluster did not answer. Try again.".into(),
                ));
            }
        };

        for (index, invoice) in batch.iter().enumerate() {
            let invoice_data = accounts
                .get(index * 2)
                .and_then(|account| account.as_ref())
                .map(|account| account.data.as_slice());
            let receipt_data = accounts
                .get(index * 2 + 1)
                .and_then(|account| account.as_ref())
                .map(|account| account.data.as_slice());
            let record = reader.record(*invoice, invoice_data, receipt_data).await;
            remember_pending(endpoint, *invoice, &record);
            answered.insert(*invoice, record);
        }
    }

    // In the order the caller named them: a seller's screen pairs each address
    // with the discount code it belongs to by position.
    Ok(pubkeys
        .iter()
        .filter_map(|pubkey| answered.remove(pubkey))
        .collect())
}

#[cfg(test)]
mod tests;
