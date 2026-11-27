use super::*;
use crate::payments::test_support::{stub_rpc, temp_db};

/// The scan's window end to end: the second request in it is answered from
/// the accounts the first one read.
#[tokio::test]
async fn a_scan_answers_the_next_request_without_asking_again() {
    let (db, path) = temp_db().await;
    let (endpoint, hits) = stub_rpc(|call| {
        let id = call.get("id").cloned().unwrap_or_default();
        serde_json::json!({"jsonrpc": "2.0", "id": id, "result": []})
    })
    .await;

    let first = program_scan_at(&endpoint, "devnet", 0, &db)
        .await
        .expect("first scan");
    let second = program_scan_at(&endpoint, "devnet", 0, &db)
        .await
        .expect("second scan");
    assert!(first.payments.is_empty());
    assert_eq!(second.cluster, "devnet");
    assert_eq!(
        hits.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the window answered the second scan"
    );
    let _ = std::fs::remove_file(path);
}

/// A scan that failed is not a scan of an empty program: it is not kept, so
/// the next request asks again instead of serving ``no payments'' for the
/// window.
#[tokio::test]
async fn a_failed_scan_is_never_remembered() {
    let (db, path) = temp_db().await;
    let (endpoint, hits) = stub_rpc(|call| {
        let id = call.get("id").cloned().unwrap_or_default();
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": -32005, "message": "node is unhealthy"},
        })
    })
    .await;

    assert!(program_scan_at(&endpoint, "devnet", 0, &db).await.is_err());
    assert!(program_scan_at(&endpoint, "devnet", 0, &db).await.is_err());
    assert!(
        hits.load(std::sync::atomic::Ordering::SeqCst) >= 2,
        "each attempt asked the cluster"
    );
    assert!(
        cached_scan(&endpoint, "devnet").is_none(),
        "a failure is not a scan to keep"
    );
    let _ = std::fs::remove_file(path);
}
