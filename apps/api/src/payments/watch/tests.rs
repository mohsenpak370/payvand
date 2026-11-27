use super::*;
use crate::payments::test_support::{stub_rpc, temp_db};

/// A seller watching two addresses is one batch, and the next tick inside
/// the window is no request at all.
#[tokio::test]
async fn a_watch_request_is_one_cluster_read_inside_the_window() {
    let (db, path) = temp_db().await;
    let pdas = vec![
        Pubkey::from([0x33u8; 32]).to_string(),
        Pubkey::from([0x34u8; 32]).to_string(),
    ];
    let (endpoint, hits) = stub_rpc(|call| {
        let id = call.get("id").cloned().unwrap_or_default();
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {"context": {"slot": 1}, "value": [null, null, null, null]},
        })
    })
    .await;

    let first = fetch_many_at(&endpoint, "devnet", &pdas, &db)
        .await
        .expect("first watch");
    let second = fetch_many_at(&endpoint, "devnet", &pdas, &db)
        .await
        .expect("second watch");
    assert_eq!(first.len(), 2);
    assert_eq!(second.len(), 2);
    assert_eq!(
        hits.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "two invoices over two polls: one batched read"
    );
    let _ = std::fs::remove_file(path);
}
