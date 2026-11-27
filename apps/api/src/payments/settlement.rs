//! Resolves a settlement marker into a full record, bounded per request.

use solana_commitment_config::CommitmentConfig;
use solana_pubkey::Pubkey;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client::rpc_client::GetConfirmedSignaturesForAddress2Config;
use solana_rpc_client_api::config::{RpcTransactionConfig, UiTransactionEncoding};
use solana_signature::Signature;
use std::collections::HashMap;

use super::{
    cache::settled::{cached_settlement, remember_settlement},
    db::settlements::{store_settlement, stored_settlement},
    model::PaymentRecord,
    onchain::{
        accounts::{settled_event_in_log, OnchainReceipt, ReceiptMarker},
        records::{marker_record, receipt_record},
    },
    rpc::rpc_batch_all,
};

/// Most settle transactions one request may read from history.
///
/// History is the only expensive read this API does — two round trips per
/// settled invoice — so a request that finds many unknown settlements resolves
/// the newest few and answers the rest from their markers. The cache makes the
/// next poll cheaper instead of re-paying for the same invoices, so a large
/// cold programme fills in over a couple of polls rather than in one burst.
pub const MAX_HISTORY_READS: usize = 32;

/// How many more history reads a request may spend. Cache hits are free and
/// never consume it, so a warm isolate answers a whole page without reading
/// history at all.
pub struct HistoryBudget {
    remaining: usize,
}

impl HistoryBudget {
    /// `wanted` is what the caller can actually show (a page size, the number
    /// of watched addresses); the cap keeps one request bounded anyway.
    pub fn new(wanted: usize) -> Self {
        Self {
            remaining: wanted.min(MAX_HISTORY_READS),
        }
    }

    /// Claims one read, or says there are none left.
    pub(crate) fn take(&mut self) -> bool {
        if self.remaining == 0 {
            return false;
        }
        self.remaining -= 1;
        true
    }
}

/// The record for a settled invoice: from this instance's memory, then from the
/// database that outlives it, then from history when this request still has a
/// read to spend, and otherwise from the marker alone.
pub(crate) async fn settled_record(
    rpc: &RpcClient,
    db: &libsql::Database,
    marker: ReceiptMarker,
    cluster: String,
    endpoint: String,
    budget: &mut HistoryBudget,
) -> PaymentRecord {
    if let Some(cached) = cached_settlement(&endpoint, &marker.invoice) {
        return receipt_record(cached, cluster, endpoint);
    }
    // The database row was written from the settle transaction once, so a
    // brand-new instance answers without touching history at all.
    if let Some(stored) = stored_settlement(db, &endpoint, &marker.invoice).await {
        remember_settlement(&endpoint, marker.invoice, &stored);
        return receipt_record(stored, cluster, endpoint);
    }
    if budget.take() {
        if let Some(settled) = settled_from_history(rpc, marker.invoice).await {
            store_settlement(db, &endpoint, &settled).await;
            remember_settlement(&endpoint, marker.invoice, &settled);
            return receipt_record(settled, cluster, endpoint);
        }
    }
    marker_record(marker, cluster, endpoint)
}

/// The settlement behind a receipt marker, read from the settle transaction's
/// own logs.
///
/// A program cannot read history, which is why the replay guard is an account.
/// The API can: the marker names the invoice, the invoice appears in the one
/// transaction that settled it, and that transaction logged `PaymentSettled`.
/// One `getSignaturesForAddress` and at most a few `getTransaction` calls per
/// settled invoice — the price of keeping the payment's 2,094,960 lamports of
/// rent off chain. `None` means the history was unreadable, and the caller falls
/// back to what the marker itself says.
/// Every settlement in at most two HTTP round trips: one batch asking each
/// invoice for its newest signature, one batch asking for those transactions.
///
/// Asking one invoice at a time is what made a cold history expensive: the
/// public cluster paces single calls hard, and the client retries a 429 with
/// backoff, so a page of twenty settlements spent tens of seconds waiting on a
/// rate limiter before answering — and often answered with nothing. A batch is
/// one request to a node that accepts it as such.
///
/// A missing entry means history did not answer for that invoice; the caller
/// falls back to the marker it already has.
pub(crate) async fn settlements_from_history(
    endpoint: &str,
    invoices: &[Pubkey],
) -> HashMap<Pubkey, OnchainReceipt> {
    let mut settled = HashMap::new();
    if invoices.is_empty() {
        return settled;
    }

    // The id is the invoice's own index, kept across both batches so a chunked
    // answer still says which invoice it belongs to.
    let calls = invoices
        .iter()
        .enumerate()
        .map(|(id, invoice)| {
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "getSignaturesForAddress",
                "params": [invoice.to_string(), {"limit": 5, "commitment": "confirmed"}],
            })
        })
        .collect();
    let mut signatures: HashMap<usize, (Pubkey, String)> = HashMap::new();
    for result in rpc_batch_all(endpoint, calls).await {
        let Some(id) = result.get("id").and_then(serde_json::Value::as_u64) else {
            continue;
        };
        let Some(invoice) = invoices.get(id as usize) else {
            continue;
        };
        let Some(signature) = result
            .get("result")
            .and_then(serde_json::Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("signature"))
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        signatures.insert(id as usize, (*invoice, signature.to_string()));
    }
    if signatures.is_empty() {
        return settled;
    }

    let mut ordered: Vec<(usize, (Pubkey, String))> = signatures.into_iter().collect();
    ordered.sort_by_key(|(id, _)| *id);

    let calls = ordered
        .iter()
        .map(|(id, (_, signature))| {
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "getTransaction",
                "params": [signature, {
                    "encoding": "json",
                    "commitment": "confirmed",
                    "maxSupportedTransactionVersion": 0,
                }],
            })
        })
        .collect();
    let results = rpc_batch_all(endpoint, calls).await;

    for result in results {
        let Some(id) = result.get("id").and_then(serde_json::Value::as_u64) else {
            continue;
        };
        let Some(invoice) = invoices.get(id as usize) else {
            continue;
        };
        let Some(transaction) = result.get("result").filter(|value| !value.is_null()) else {
            continue;
        };
        let Some(logs) = transaction
            .pointer("/meta/logMessages")
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };
        for line in logs.iter().filter_map(serde_json::Value::as_str) {
            let Some(receipt) = settled_event_in_log(line) else {
                continue;
            };
            if receipt.invoice != *invoice {
                continue;
            }
            // A transaction's block time is the only clock the event has; the
            // marker's own `settled_at` is written from it.
            let at = transaction
                .get("blockTime")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);
            settled.insert(
                *invoice,
                OnchainReceipt {
                    created_at: at,
                    settled_at: at,
                    ..receipt
                },
            );
            break;
        }
    }

    settled
}

pub(crate) async fn settled_from_history(
    rpc: &RpcClient,
    invoice: Pubkey,
) -> Option<OnchainReceipt> {
    let signatures = rpc
        .get_signatures_for_address_with_config(
            &invoice,
            GetConfirmedSignaturesForAddress2Config {
                commitment: Some(CommitmentConfig::confirmed()),
                limit: Some(5),
                ..Default::default()
            },
        )
        .await
        .ok()?;

    for entry in signatures {
        let Ok(signature) = entry.signature.parse::<Signature>() else {
            continue;
        };
        let Ok(transaction) = rpc
            .get_transaction_with_config(
                &signature,
                RpcTransactionConfig {
                    encoding: Some(UiTransactionEncoding::Json),
                    commitment: Some(CommitmentConfig::confirmed()),
                    max_supported_transaction_version: Some(0),
                },
            )
            .await
        else {
            continue;
        };

        let logs = transaction
            .transaction
            .meta
            .and_then(|meta| Option::<Vec<String>>::from(meta.log_messages))
            .unwrap_or_default();
        for line in logs {
            let Some(settled) = settled_event_in_log(&line) else {
                continue;
            };
            if settled.invoice != invoice {
                continue;
            }
            let at = transaction.block_time.unwrap_or(0);
            return Some(OnchainReceipt {
                created_at: at,
                settled_at: at,
                ..settled
            });
        }
    }

    None
}

#[cfg(test)]
mod tests;
