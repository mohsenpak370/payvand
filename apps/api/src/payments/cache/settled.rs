//! Settled records cached for the life of the instance.

use solana_pubkey::Pubkey;
use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};

use super::super::onchain::accounts::OnchainReceipt;

/// Settlements are immutable: a marker and the transaction behind it are
/// written once, and nothing rewrites a settled invoice. So an isolate can keep
/// what it resolved and never ask again while it lives — which is what turns a
/// poll from "read history for every settled invoice" into "read history for
/// the one that was just paid".
///
/// Bounded by [`SETTLED_CACHE_LIMIT`] with oldest-first eviction: an isolate
/// that has seen a very long programme forgets its oldest settlements, and pays
/// for them again only if they are shown again.
const SETTLED_CACHE_LIMIT: usize = 4_096;

struct SettledCache {
    inner: Mutex<SettledCacheInner>,
}

struct SettledCacheInner {
    /// `(endpoint, invoice)` — the same invoice on two clusters is two
    /// settlements, so the endpoint is part of the key.
    entries: HashMap<(String, Pubkey), OnchainReceipt>,
    /// Insertion order, so the oldest entry is the one evicted.
    order: VecDeque<(String, Pubkey)>,
}

static SETTLED_CACHE: OnceLock<SettledCache> = OnceLock::new();

fn settled_cache() -> &'static SettledCache {
    SETTLED_CACHE.get_or_init(|| SettledCache {
        inner: Mutex::new(SettledCacheInner {
            entries: HashMap::new(),
            order: VecDeque::new(),
        }),
    })
}

/// A lock poisoned by a panicking request is still a usable map: recover the
/// data rather than turning one failed thread into a failing endpoint.
pub(crate) fn cached_settlement(endpoint: &str, invoice: &Pubkey) -> Option<OnchainReceipt> {
    let inner = settled_cache()
        .inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    inner.entries.get(&(endpoint.to_owned(), *invoice)).cloned()
}

/// Only successful reads are remembered: a failed one must be retried, or one
/// bad node would pin an invoice to its marker for the isolate's lifetime.
pub(crate) fn remember_settlement(endpoint: &str, invoice: Pubkey, settled: &OnchainReceipt) {
    let mut inner = settled_cache()
        .inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    remember_in(&mut inner, endpoint, invoice, settled);
}

/// The insertion itself, taking the map rather than reaching for the static
/// one, so the bound can be tested without filling the process-wide cache.
fn remember_in(
    inner: &mut SettledCacheInner,
    endpoint: &str,
    invoice: Pubkey,
    settled: &OnchainReceipt,
) {
    let key = (endpoint.to_owned(), invoice);
    if inner.entries.insert(key.clone(), settled.clone()).is_none() {
        inner.order.push_back(key);
    }
    while inner.entries.len() > SETTLED_CACHE_LIMIT {
        match inner.order.pop_front() {
            Some(oldest) => {
                inner.entries.remove(&oldest);
            }
            None => break,
        }
    }
}

#[cfg(test)]
mod tests;
