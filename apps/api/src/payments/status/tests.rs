use super::*;
use crate::payments::test_support::{stub_rpc, temp_db};
use crate::payments::PaymentStatus;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;

#[tokio::test]
async fn an_unpaid_invoice_is_pending_not_missing() {
    let (db, path) = temp_db().await;
    let invoice = Pubkey::from([4u8; 32]);
    let rpc = RpcClient::new("https://example.invalid".into());
    let mut reader = InvoiceReader::new(&rpc, &db, "devnet".into(), "https://example".into(), 1);
    let record = reader.record(invoice, None, None).await;
    assert_eq!(record.status, PaymentStatus::Pending);
    assert_eq!(record.payment_pda, invoice.to_string());
    assert_eq!(record.amount_lamports, 0);
    assert!(record.settled_by.is_none());
    let _ = std::fs::remove_file(path);
}

/// The memo through the real read: the buyer's page and a second viewer of
/// the same invoice cost one cluster request, not two.
#[tokio::test]
async fn a_second_viewer_is_answered_from_the_first_read() {
    let (db, path) = temp_db().await;
    let invoice = Pubkey::from([0x31u8; 32]);
    let (endpoint, hits) = stub_rpc(|call| {
        let id = call.get("id").cloned().unwrap_or_default();
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {"context": {"slot": 1}, "value": [null, null]},
        })
    })
    .await;

    let first = fetch_payment_at(&endpoint, "devnet", invoice, &db)
        .await
        .expect("first read");
    let second = fetch_payment_at(&endpoint, "devnet", invoice, &db)
        .await
        .expect("second read");
    assert_eq!(first.status, PaymentStatus::Pending);
    assert_eq!(second.id, first.id);
    assert_eq!(
        hits.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the window answered the second viewer"
    );
    let _ = std::fs::remove_file(path);
}

/// A cluster that does not answer is an error, not "unpaid": nothing is
/// remembered, and the next poll asks again rather than serving the failure
/// as a status.
#[tokio::test]
async fn a_failed_read_is_never_remembered() {
    let (db, path) = temp_db().await;
    let invoice = Pubkey::from([0x32u8; 32]);
    let (endpoint, hits) = stub_rpc(|call| {
        let id = call.get("id").cloned().unwrap_or_default();
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": -32000, "message": "node is behind"},
        })
    })
    .await;

    assert!(fetch_payment_at(&endpoint, "devnet", invoice, &db)
        .await
        .is_err());
    assert!(fetch_payment_at(&endpoint, "devnet", invoice, &db)
        .await
        .is_err());
    assert!(
        hits.load(std::sync::atomic::Ordering::SeqCst) >= 2,
        "each attempt asked the cluster"
    );
    assert!(
        cached_pending(&endpoint, &invoice).is_none(),
        "a failure is not an answer to keep"
    );
    let _ = std::fs::remove_file(path);
}
