//! Short-lived cache of program scan accounts.

use solana_pubkey::Pubkey;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// How long a program scan may be reused.
///
/// The scan is one `getProgramAccounts` over every account the program owns, and
/// it runs when the feed has to fall back to the chain. That is the cold path,
/// but a page open in three tabs still asks three times in the same second, and
/// the admin list asks beside it. Eight seconds is short enough that an invoice
/// made just now is in the next scan and long enough that a burst is one read.
const SCAN_TTL: Duration = Duration::from_secs(8);

/// Scans one instance keeps: `(endpoint, cluster)`. One entry per cluster in
/// practice, bounded anyway.
const SCAN_CACHE_LIMIT: usize = 8;

/// One scan's accounts, with when they were read.
type ScannedEntry = (Instant, Vec<(Pubkey, Vec<u8>)>);

/// `(endpoint, cluster)` → the scan of it a moment ago.
type ScannedAccountsMap = HashMap<(String, String), ScannedEntry>;

/// The accounts a scan last returned, per `(endpoint, cluster)`.
///
/// Accounts, not records: the caller resolves settlements against the database
/// with its own history budget, and that must not be frozen for the window.
/// A failed scan is never stored — an error is not an answer.
struct ScannedAccounts {
    inner: Mutex<ScannedAccountsMap>,
}

static SCAN_CACHE: OnceLock<ScannedAccounts> = OnceLock::new();

fn scan_cache() -> &'static ScannedAccounts {
    SCAN_CACHE.get_or_init(|| ScannedAccounts {
        inner: Mutex::new(HashMap::new()),
    })
}

pub(crate) fn cached_scan(endpoint: &str, cluster: &str) -> Option<Vec<(Pubkey, Vec<u8>)>> {
    let mut inner = scan_cache()
        .inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cached_scan_in(&mut inner, endpoint, cluster, Instant::now())
}

fn cached_scan_in(
    inner: &mut ScannedAccountsMap,
    endpoint: &str,
    cluster: &str,
    now: Instant,
) -> Option<Vec<(Pubkey, Vec<u8>)>> {
    let key = (endpoint.to_owned(), cluster.to_owned());
    let (read_at, accounts) = inner.get(&key)?;
    if now.duration_since(*read_at) >= SCAN_TTL {
        inner.remove(&key);
        return None;
    }
    Some(accounts.clone())
}

pub(crate) fn remember_scan(endpoint: &str, cluster: &str, accounts: &[(Pubkey, Vec<u8>)]) {
    let mut inner = scan_cache()
        .inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    remember_scan_in(&mut inner, endpoint, cluster, accounts, Instant::now());
}

fn remember_scan_in(
    inner: &mut ScannedAccountsMap,
    endpoint: &str,
    cluster: &str,
    accounts: &[(Pubkey, Vec<u8>)],
    now: Instant,
) {
    // Expired entries are dropped here rather than swept on a timer: the map is
    // one entry per cluster, and this is the only place it grows.
    inner.retain(|_, (read_at, _)| now.duration_since(*read_at) < SCAN_TTL);
    inner.insert(
        (endpoint.to_owned(), cluster.to_owned()),
        (now, accounts.to_vec()),
    );
    while inner.len() > SCAN_CACHE_LIMIT {
        let Some(oldest) = inner
            .iter()
            .min_by_key(|(_, (read_at, _))| *read_at)
            .map(|(key, _)| key.clone())
        else {
            break;
        };
        inner.remove(&oldest);
    }
}

#[cfg(test)]
mod tests;
