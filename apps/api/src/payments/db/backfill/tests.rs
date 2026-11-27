use super::*;
use crate::payments::onchain::reader::account_discriminator;
use crate::payments::recorded_invoices;
use crate::payments::test_support::{stub_rpc, temp_db};
use crate::payments::PAYVAND_PROGRAM_ID;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use solana_pubkey::Pubkey;

/// A row from before amounts were kept is completed from its own account,
/// once, and the seller's own words are left exactly as they were.
#[tokio::test]
async fn a_legacy_row_is_filled_from_the_chain_and_asked_about_once() {
    let (db, path) = temp_db().await;
    let invoice = Pubkey::from([0x71u8; 32]);
    let seller = Pubkey::from([0x72u8; 32]);
    let reference = Pubkey::from([0x73u8; 32]);

    let mut data = account_discriminator().to_vec();
    data.extend_from_slice(seller.as_ref());
    data.extend_from_slice(reference.as_ref());
    data.extend_from_slice(&4_500_000u64.to_le_bytes());
    data.extend_from_slice(&10u32.to_le_bytes());
    data.extend_from_slice(b"flat white");
    data.push(0); // pending
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&100i64.to_le_bytes());
    data.extend_from_slice(&0i64.to_le_bytes());
    let account = serde_json::json!({
        "data": [STANDARD.encode(&data), "base64"],
        "executable": false,
        "lamports": 1000,
        "owner": PAYVAND_PROGRAM_ID,
        "rentEpoch": 0,
    });
    let (endpoint, hits) = stub_rpc(move |call| {
        let id = call.get("id").cloned().unwrap_or_default();
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {"context": {"slot": 1}, "value": [account.clone(), null]},
        })
    })
    .await;

    let connection = db.connect().expect("connect");
    connection
        .execute(
            "INSERT INTO invoice_meta (id, seller, name, description, created_at) \
             VALUES (?1, ?2, 'Flat white', 'Cardamom bun', 100)",
            libsql::params![invoice.to_string(), seller.to_string()],
        )
        .await
        .expect("a row from before amounts were kept");

    backfill_invoice_rows(&db, &seller.to_string(), "devnet", &endpoint).await;

    let mut rows = connection
        .query(
            "SELECT amount, currency, reference, cluster, endpoint, name, description \
             FROM invoice_meta WHERE id = ?1",
            libsql::params![invoice.to_string()],
        )
        .await
        .expect("read");
    let row = rows.next().await.expect("row").expect("the row");
    assert_eq!(row.get::<i64>(0).expect("amount"), 4_500_000);
    assert_eq!(row.get::<String>(1).expect("currency"), "sol");
    assert_eq!(
        row.get::<String>(2).expect("reference"),
        reference.to_string()
    );
    assert_eq!(row.get::<String>(3).expect("cluster"), "devnet");
    assert_eq!(row.get::<String>(4).expect("endpoint"), endpoint);
    assert_eq!(
        row.get::<String>(5).expect("name"),
        "Flat white",
        "the seller's own words are not overwritten"
    );
    assert_eq!(row.get::<String>(6).expect("description"), "Cardamom bun");

    // And the row is now something the feed can list.
    let listed = recorded_invoices(&db, &seller.to_string(), "devnet", 10).await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].amount_lamports, 4_500_000);

    let asked = hits.load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(asked, 1, "one batched read filled the page");
    backfill_invoice_rows(&db, &seller.to_string(), "devnet", &endpoint).await;
    assert_eq!(
        hits.load(std::sync::atomic::Ordering::SeqCst),
        asked,
        "a filled row is not asked about again"
    );
    let _ = std::fs::remove_file(path);
}

/// A row the chain knows nothing about — an invoice from another cluster, or
/// one whose account is long gone — is asked about once and then left alone.
#[tokio::test]
async fn a_legacy_row_the_chain_does_not_know_is_not_asked_again() {
    let (db, path) = temp_db().await;
    let invoice = Pubkey::from([0x74u8; 32]);
    let seller = Pubkey::from([0x75u8; 32]);
    let (endpoint, hits) = stub_rpc(|call| {
        let id = call.get("id").cloned().unwrap_or_default();
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {"context": {"slot": 1}, "value": [null, null]},
        })
    })
    .await;

    let connection = db.connect().expect("connect");
    connection
        .execute(
            "INSERT INTO invoice_meta (id, seller, name, description, created_at) \
             VALUES (?1, ?2, 'Flat white', '', 100)",
            libsql::params![invoice.to_string(), seller.to_string()],
        )
        .await
        .expect("legacy row");

    backfill_invoice_rows(&db, &seller.to_string(), "devnet", &endpoint).await;
    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(
        recorded_invoices(&db, &seller.to_string(), "devnet", 10)
            .await
            .is_empty(),
        "nothing to list until the chain says something"
    );

    backfill_invoice_rows(&db, &seller.to_string(), "devnet", &endpoint).await;
    assert_eq!(
        hits.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the next load does not ask about it again"
    );
    let _ = std::fs::remove_file(path);
}

/// A cluster that did not answer is not an answer: nothing is remembered as
/// tried, so the next load asks again instead of leaving the row behind.
#[tokio::test]
async fn a_backfill_that_failed_is_tried_again() {
    let (db, path) = temp_db().await;
    let invoice = Pubkey::from([0x76u8; 32]);
    let seller = Pubkey::from([0x77u8; 32]);
    let (endpoint, hits) = stub_rpc(|call| {
        let id = call.get("id").cloned().unwrap_or_default();
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": -32005, "message": "node is unhealthy"},
        })
    })
    .await;

    let connection = db.connect().expect("connect");
    connection
        .execute(
            "INSERT INTO invoice_meta (id, seller, name, description, created_at) \
             VALUES (?1, ?2, 'Flat white', '', 100)",
            libsql::params![invoice.to_string(), seller.to_string()],
        )
        .await
        .expect("legacy row");

    backfill_invoice_rows(&db, &seller.to_string(), "devnet", &endpoint).await;
    backfill_invoice_rows(&db, &seller.to_string(), "devnet", &endpoint).await;
    assert!(
        hits.load(std::sync::atomic::Ordering::SeqCst) >= 2,
        "each load asks again until the cluster answers"
    );

    let mut rows = connection
        .query("SELECT count(*) FROM invoice_backfill", ())
        .await
        .expect("read");
    let row = rows.next().await.expect("row").expect("count");
    assert_eq!(
        row.get::<i64>(0).expect("count"),
        0,
        "a failure is not remembered as an answer"
    );
    let _ = std::fs::remove_file(path);
}
