//! One answer per cluster, kept for a minute.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::model::ConfigResponse;

/// How long one read of the policy and the rents answers every request.
///
/// A fee change is an admin instruction, not a market move: a minute of lag
/// costs a quote nothing, and the same minute is what the checkout already holds
/// its own reads for.
pub(super) const CONFIG_TTL: Duration = Duration::from_secs(60);

struct ConfigCache {
    inner: Mutex<HashMap<String, (Instant, ConfigResponse)>>,
}

static CONFIG_CACHE: OnceLock<ConfigCache> = OnceLock::new();

fn config_cache() -> &'static ConfigCache {
    CONFIG_CACHE.get_or_init(|| ConfigCache {
        inner: Mutex::new(HashMap::new()),
    })
}

pub(super) fn cached_config(cluster: &str) -> Option<ConfigResponse> {
    let mut inner = config_cache()
        .inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cached_config_in(&mut inner, cluster, Instant::now())
}

fn cached_config_in(
    inner: &mut HashMap<String, (Instant, ConfigResponse)>,
    cluster: &str,
    now: Instant,
) -> Option<ConfigResponse> {
    let (read_at, config) = inner.get(cluster)?;
    if now.duration_since(*read_at) >= CONFIG_TTL {
        inner.remove(cluster);
        return None;
    }
    Some(config.clone())
}

pub(super) fn remember_config(cluster: &str, config: &ConfigResponse) {
    let mut inner = config_cache()
        .inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    remember_config_in(&mut inner, cluster, config, Instant::now());
}

fn remember_config_in(
    inner: &mut HashMap<String, (Instant, ConfigResponse)>,
    cluster: &str,
    config: &ConfigResponse,
    now: Instant,
) {
    // One entry per cluster, and a cluster is one of two or three names: the map
    // cannot grow past a handful, so expired entries are simply dropped here.
    inner.retain(|_, (read_at, _)| now.duration_since(*read_at) < CONFIG_TTL);
    inner.insert(cluster.to_owned(), (now, config.clone()));
}

#[cfg(test)]
mod tests;
