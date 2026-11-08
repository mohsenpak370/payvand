//! A fixed-window limiter for the endpoints a stranger can reach.
//!
//! The API also runs as a serverless function, so these counts live in the
//! running instance rather than across the fleet: this is a speed bump, not a
//! quota. It turns "try a million passwords" into "try twenty", which is the
//! part a process can do for itself; a real ceiling belongs in the proxy in
//! front of the function.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::http::HeaderMap;

use crate::payments::{ApiError, ApiResult};

/// The longest window any caller asks for. Sweeping with this horizon can
/// never drop a key that is still counting.
const MAX_WINDOW: Duration = Duration::from_secs(3600);

/// Sweep once the map outgrows this, so a busy hour cannot grow it forever.
const SWEEP_ABOVE: usize = 10_000;

/// Named window lengths, so a policy reads as a sentence.
pub(crate) const MINUTE: u64 = 60;
pub(crate) const HOUR: u64 = 3600;

struct Limiter {
    /// Bucket key → the window's start and the hits inside it.
    hits: Mutex<HashMap<String, (Instant, u32)>>,
}

static LIMITER: OnceLock<Limiter> = OnceLock::new();

/// Counts one hit for `bucket` + `key`, and fails once the window is full.
pub(crate) fn hit(bucket: &str, key: &str, limit: u32, window_secs: u64) -> ApiResult<()> {
    let limiter = LIMITER.get_or_init(|| Limiter {
        hits: Mutex::new(HashMap::new()),
    });
    limiter.count(
        &format!("{bucket}:{key}"),
        limit,
        Duration::from_secs(window_secs),
    )
}

impl Limiter {
    fn count(&self, key: &str, limit: u32, window: Duration) -> ApiResult<()> {
        // A poisoned lock is still a counter: recover the data and carry on,
        // rather than turning one failed thread into a failed endpoint.
        let mut hits = self
            .hits
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let now = Instant::now();
        if hits.len() > SWEEP_ABOVE {
            hits.retain(|_, (start, _)| now.duration_since(*start) < MAX_WINDOW);
        }

        let entry = hits.entry(key.to_owned()).or_insert((now, 0));
        if now.duration_since(entry.0) >= window {
            *entry = (now, 0);
        }
        entry.1 += 1;

        if entry.1 > limit {
            return Err(ApiError::TooManyRequests(format!(
                "Too many attempts. Wait {} and try again.",
                about(window)
            )));
        }
        Ok(())
    }
}

/// "an hour", "5 minutes", "a minute" — what the caller is told to wait.
fn about(window: Duration) -> String {
    let secs = window.as_secs();
    if secs >= 3600 {
        "an hour".to_string()
    } else if secs >= 120 {
        format!("{} minutes", secs / 60)
    } else {
        "a minute".to_string()
    }
}

/// The caller's address as the proxy in front of the API reports it. When it
/// cannot be read (a direct hit, a local run) every caller shares one bucket:
/// a limiter that cannot tell who is asking must not hand out free attempts.
pub(crate) fn caller(headers: &HeaderMap) -> String {
    let forwarded = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(address) = forwarded {
        return address.to_owned();
    }
    headers
        .get("x-real-ip")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_until_the_limit_and_then_refuses() {
        let bucket = "test-limit";
        for _ in 0..3 {
            assert!(hit(bucket, "one-caller", 3, HOUR).is_ok());
        }
        let refused = hit(bucket, "one-caller", 3, HOUR);
        assert!(matches!(refused, Err(ApiError::TooManyRequests(_))));
    }

    #[test]
    fn keeps_each_caller_in_its_own_bucket() {
        assert!(hit("test-separate", "a", 1, HOUR).is_ok());
        assert!(hit("test-separate", "b", 1, HOUR).is_ok());
        assert!(hit("test-separate", "a", 1, HOUR).is_err());
    }

    #[test]
    fn names_the_wait() {
        assert_eq!(about(Duration::from_secs(60)), "a minute");
        assert_eq!(about(Duration::from_secs(3600)), "an hour");
        assert_eq!(about(Duration::from_secs(600)), "10 minutes");
    }

    #[test]
    fn reads_the_forwarded_caller() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.9, 10.0.0.1".parse().unwrap());
        assert_eq!(caller(&headers), "203.0.113.9");
        assert_eq!(caller(&HeaderMap::new()), "unknown");
    }
}
