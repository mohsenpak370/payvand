//! Short-lived cache for unsettled status answers.

use solana_pubkey::Pubkey;
use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::super::model::{PaymentRecord, PaymentStatus};

/// How long an unsettled answer may be reused.
///
/// A pending invoice is the one status that changes, and it changes once. The
/// polling clients ask about the same address from several places at the same
/// time — the buyer's page, a second tab, the seller's screen watching the code
/// it printed — and each of those reads used to be its own request to the
/// cluster. Eight seconds spans a poll tick (`STATUS_POLL_MS` is five), so a
/// payment shows up within two polls at most — the checkout's own receipt read
/// covers the gap in between — and a single viewer's polls, not just concurrent
/// viewers of one invoice, share a cluster read.
const PENDING_TTL: Duration = Duration::from_secs(8);

/// Addresses one instance remembers an unsettled answer for, oldest-first: a
/// very busy day forgets what stopped being asked.
const PENDING_CACHE_LIMIT: usize = 4_096;

/// The last unsettled answer per `(endpoint, invoice)`, with when it was read.
///
/// Only unsettled records are kept. A settled one is final and lives in the
/// settled cache, which never expires; an answer that failed to read is not a
/// status at all and is never stored.
struct PendingCache {
    inner: Mutex<PendingCacheInner>,
}

#[derive(Default)]
struct PendingCacheInner {
    entries: HashMap<(String, Pubkey), (Instant, PaymentRecord)>,
    /// Insertion order, so the oldest entry is the one evicted.
    order: VecDeque<(String, Pubkey)>,
}

static PENDING_CACHE: OnceLock<PendingCache> = OnceLock::new();

fn pending_cache() -> &'static PendingCache {
    PENDING_CACHE.get_or_init(|| PendingCache {
        inner: Mutex::new(PendingCacheInner::default()),
    })
}

/// A recent unsettled answer, if one is still inside [`PENDING_TTL`].
pub(crate) fn cached_pending(endpoint: &str, invoice: &Pubkey) -> Option<PaymentRecord> {
    let mut inner = pending_cache()
        .inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cached_pending_in(&mut inner, endpoint, invoice, Instant::now())
}

/// The lookup itself, taking the map and the clock rather than reaching for the
/// statics, so the expiry can be tested without waiting for it.
fn cached_pending_in(
    inner: &mut PendingCacheInner,
    endpoint: &str,
    invoice: &Pubkey,
    now: Instant,
) -> Option<PaymentRecord> {
    let key = (endpoint.to_owned(), *invoice);
    let (read_at, record) = inner.entries.get(&key)?;
    if now.duration_since(*read_at) >= PENDING_TTL {
        inner.entries.remove(&key);
        return None;
    }
    Some(record.clone())
}

/// Remembers an unsettled answer. A settled record is refused here on purpose:
/// it belongs to the cache that never expires, and a copy with a deadline would
/// only make the next read slower.
pub(crate) fn remember_pending(endpoint: &str, invoice: Pubkey, record: &PaymentRecord) {
    let mut inner = pending_cache()
        .inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    remember_pending_in(&mut inner, endpoint, invoice, record, Instant::now());
}

fn remember_pending_in(
    inner: &mut PendingCacheInner,
    endpoint: &str,
    invoice: Pubkey,
    record: &PaymentRecord,
    now: Instant,
) {
    if record.status == PaymentStatus::Settled {
        return;
    }
    let key = (endpoint.to_owned(), invoice);
    if inner
        .entries
        .insert(key.clone(), (now, record.clone()))
        .is_none()
    {
        inner.order.push_back(key);
    }
    while inner.entries.len() > PENDING_CACHE_LIMIT {
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
