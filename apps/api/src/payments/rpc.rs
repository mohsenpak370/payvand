//! Batched JSON-RPC posts for settlement history.

use std::sync::OnceLock;

/// One HTTP client for the batched history reads; building one per request
/// would pay the TLS handshake for every page.
static RPC_HTTP: OnceLock<Option<reqwest::Client>> = OnceLock::new();

fn rpc_http() -> Option<&'static reqwest::Client> {
    RPC_HTTP
        .get_or_init(|| {
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                // HTTP/1.1 on purpose: the public cluster answers a batch sent
                // over HTTP/2 with 429 while the same batch over HTTP/1.1 is
                // served, and single requests are unaffected either way.
                .http1_only()
                .build()
                .ok()
        })
        .as_ref()
}

/// Requests one batch may carry. The public cluster answers a bigger batch with
/// 429 rather than an error, so this is the size that keeps a page of history
/// from being refused outright.
const RPC_BATCH_LIMIT: usize = 10;

/// Posts every call, in batches the node accepts. A batch that fails costs only
/// its own invoices their detail; the rest still resolve.
pub(crate) async fn rpc_batch_all(
    endpoint: &str,
    calls: Vec<serde_json::Value>,
) -> Vec<serde_json::Value> {
    let mut results = Vec::with_capacity(calls.len());
    for batch in calls.chunks(RPC_BATCH_LIMIT) {
        if let Some(mut answered) = rpc_batch(endpoint, batch.to_vec()).await {
            results.append(&mut answered);
        }
    }
    results
}

/// Posts a JSON-RPC batch and returns its entries. One retry, because the
/// public cluster answers a batch it is pacing with 429 and a retry usually
/// lands after the window; anything else is a miss the caller already handles.
async fn rpc_batch(
    endpoint: &str,
    calls: Vec<serde_json::Value>,
) -> Option<Vec<serde_json::Value>> {
    let client = rpc_http()?;
    let body = serde_json::to_vec(&serde_json::Value::Array(calls)).ok()?;

    for attempt in 0..2 {
        let response = client
            .post(endpoint)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body.clone())
            .send()
            .await;
        let Ok(response) = response else {
            continue;
        };
        if response.status().is_success() {
            let text = response.text().await.ok()?;
            return serde_json::from_str::<serde_json::Value>(&text)
                .ok()?
                .as_array()
                .cloned();
        }
        tracing::info!(status = %response.status(), attempt, "settlement history batch was refused");
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    None
}
