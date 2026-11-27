use super::*;
use crate::payments::test_support::{settled_log_line, stub_rpc, temp_db};
use crate::payments::PaymentStatus;

/// The whole point of the cache: a settled invoice already resolved is
/// answered without touching history, so a poll repeated once a second is
/// one account read and nothing else. The cluster here is unreachable, and
/// the budget is empty, so only the cache can produce this record.
#[tokio::test]
async fn a_resolved_settlement_is_served_from_memory() {
    let (db, path) = temp_db().await;
    let endpoint = "https://cache-test-resolved.invalid";
    let invoice = Pubkey::from([0xB1u8; 32]);
    let line = settled_log_line(invoice, Pubkey::default(), 42, "tip");
    let settled = settled_event_in_log(&line).expect("decodes");
    remember_settlement(endpoint, invoice, &settled);

    let rpc = RpcClient::new("https://example.invalid".into());
    let mut budget = HistoryBudget::new(0);
    let record = settled_record(
        &rpc,
        &db,
        ReceiptMarker {
            invoice,
            settled_at: 200,
        },
        "devnet".into(),
        endpoint.into(),
        &mut budget,
    )
    .await;

    assert_eq!(record.status, PaymentStatus::Settled);
    assert_eq!(record.amount_lamports, 42);
    assert_eq!(record.memo, "tip");
    assert_eq!(record.settled_by, Some(Pubkey::from([5u8; 32]).to_string()));
    let _ = std::fs::remove_file(path);
}

/// When a request has no history read left, the marker still answers —
/// settled, with the time it settled at — instead of the read stalling the
/// whole page.
#[tokio::test]
async fn an_exhausted_history_budget_falls_back_to_the_marker() {
    let (db, path) = temp_db().await;
    let invoice = Pubkey::from([0xB2u8; 32]);
    let rpc = RpcClient::new("https://example.invalid".into());
    let mut budget = HistoryBudget::new(0);

    let record = settled_record(
        &rpc,
        &db,
        ReceiptMarker {
            invoice,
            settled_at: 200,
        },
        "devnet".into(),
        "https://cache-test-budget.invalid".into(),
        &mut budget,
    )
    .await;

    assert_eq!(record.status, PaymentStatus::Settled);
    assert_eq!(record.id, invoice.to_string());
    // The marker carries the settlement's time; a marker answer is not a
    // blank one.
    assert_eq!(record.settled_at_unix, 200);
    let _ = std::fs::remove_file(path);
}

/// The reason the cache moved into the database: a fresh instance has an
/// empty memory, and the settlement it stored earlier still answers — with
/// an unreachable cluster and no history read left, only that row can.
#[tokio::test]
async fn a_settlement_read_once_is_answered_from_the_database() {
    let (db, path) = temp_db().await;
    let endpoint = "https://settled-db-test.invalid";
    let invoice = Pubkey::from([0xC1u8; 32]);
    let line = settled_log_line(invoice, Pubkey::default(), 42, "tip");
    // What a successful history read hands on: the event plus the block time
    // the settle transaction carried.
    let settled = OnchainReceipt {
        created_at: 190,
        settled_at: 200,
        ..settled_event_in_log(&line).expect("decodes")
    };
    store_settlement(&db, endpoint, &settled).await;

    let rpc = RpcClient::new("https://example.invalid".into());
    let mut budget = HistoryBudget::new(0);
    let record = settled_record(
        &rpc,
        &db,
        ReceiptMarker {
            invoice,
            settled_at: 200,
        },
        "devnet".into(),
        endpoint.into(),
        &mut budget,
    )
    .await;

    assert_eq!(record.status, PaymentStatus::Settled);
    assert_eq!(record.amount_lamports, 42);
    assert_eq!(record.memo, "tip");
    assert_eq!(record.settled_by, Some(Pubkey::from([5u8; 32]).to_string()));
    assert_eq!(record.created_at_unix, 190);
    assert_eq!(record.settled_at_unix, 200);

    let _ = std::fs::remove_file(path);
}

/// The batched read has to carry the invoice's own id through both requests
/// and read the settle event out of the transaction's logs.
#[tokio::test]
async fn a_batch_history_read_returns_the_settle_event() {
    let invoice = Pubkey::from([0xB7u8; 32]);
    let line = settled_log_line(invoice, Pubkey::default(), 42, "tip");
    let signature = "5vxXkMhTVpHt4QkHVgcFj8itGev4a7trxRkj5Z3xs37".to_string();

    let (endpoint, _) = stub_rpc(move |call| {
        let id = call.get("id").cloned().unwrap_or_default();
        match call.get("method").and_then(serde_json::Value::as_str) {
            Some("getSignaturesForAddress") => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": [{"signature": signature.clone()}],
            }),
            Some("getTransaction") => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {"blockTime": 700, "meta": {"logMessages": [line.clone()]}},
            }),
            _ => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": -32601, "message": "no such method"},
            }),
        }
    })
    .await;

    let settled = settlements_from_history(&endpoint, &[invoice]).await;
    let receipt = settled
        .get(&invoice)
        .expect("the settle event was read back");
    assert_eq!(receipt.amount, 42);
    assert_eq!(receipt.memo, "tip");
    assert_eq!(receipt.payer, Pubkey::from([5u8; 32]));
    assert_eq!(receipt.settled_at, 700, "the transaction's block time");
}

/// The cluster refuses a batch bigger than ten, so twelve invoices cost two
/// signature requests, and only the invoice that answered is asked about
/// again — a per-item 429 must not fail the rest of the page.
#[tokio::test]
async fn history_batches_are_chunked_and_a_refused_item_does_not_fail_the_page() {
    let invoices: Vec<Pubkey> = (0..12u8)
        .map(|index| Pubkey::from([index + 1; 32]))
        .collect();

    let (endpoint, hits) = stub_rpc(|call| {
        let id = call.get("id").cloned().unwrap_or_default();
        let first = call.get("id").and_then(serde_json::Value::as_u64) == Some(0);
        match call.get("method").and_then(serde_json::Value::as_str) {
            Some("getSignaturesForAddress") if first => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": [{"signature": "sig-0"}],
            }),
            Some("getSignaturesForAddress") => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": 429, "message": "Too many requests for a specific RPC call"},
            }),
            _ => serde_json::json!({"jsonrpc": "2.0", "id": id, "result": null}),
        }
    })
    .await;

    let settled = settlements_from_history(&endpoint, &invoices).await;
    assert!(settled.is_empty(), "no transaction came back");
    // Two signature chunks, then one transaction request for the one
    // invoice that answered.
    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 3);
}
