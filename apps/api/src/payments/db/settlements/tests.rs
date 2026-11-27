use super::*;
use crate::payments::onchain::accounts::settled_event_in_log;
use crate::payments::test_support::{settled_log_line, temp_db};
use crate::payments::PaymentStatus;

/// A settled invoice this wallet paid is listed from the stored settlement:
/// the payer is only knowable from the settle transaction, and it is already
/// in the row history wrote.
#[tokio::test]
async fn settlements_this_wallet_paid_are_listed_from_the_db() {
    let (db, path) = temp_db().await;
    let buyer = Pubkey::from([0x61u8; 32]);
    let receipt = OnchainReceipt {
        invoice: Pubkey::from([0x62u8; 32]),
        seller: Pubkey::from([0x63u8; 32]),
        reference: Pubkey::from([0x64u8; 32]),
        mint: Pubkey::default(),
        amount: 2_000_000,
        memo: "coffee".into(),
        payer: buyer,
        created_at: 10,
        settled_at: 20,
    };
    let endpoint = "https://feed-test.invalid";
    store_settlement(&db, endpoint, &receipt).await;

    let paid = settlements_paid_by(&db, endpoint, "devnet", &buyer.to_string(), 10).await;
    assert_eq!(paid.len(), 1);
    assert_eq!(paid[0].id, receipt.invoice.to_string());
    assert_eq!(paid[0].status, PaymentStatus::Settled);
    assert_eq!(paid[0].amount_lamports, 2_000_000);
    assert_eq!(
        paid[0].settled_by.as_deref(),
        Some(buyer.to_string().as_str())
    );

    let stranger = Pubkey::from([0x65u8; 32]);
    assert!(
        settlements_paid_by(&db, endpoint, "devnet", &stranger.to_string(), 10)
            .await
            .is_empty(),
        "another wallet's payments are not in this feed"
    );
    assert!(
        settlements_paid_by(
            &db,
            "https://other.invalid",
            "devnet",
            &buyer.to_string(),
            10
        )
        .await
        .is_empty(),
        "a settlement belongs to the cluster it settled on"
    );

    let _ = std::fs::remove_file(path);
}

/// The scan writes every settlement it resolved in one statement and reads
/// them back in one, which is what keeps a cold history off a round trip per
/// marker.
#[tokio::test]
async fn settlements_are_stored_and_read_in_one_batch() {
    let (db, path) = temp_db().await;
    let endpoint = "https://settled-batch.invalid";
    let first = settled_event_in_log(&settled_log_line(
        Pubkey::from([0xA1u8; 32]),
        Pubkey::default(),
        42,
        "tip",
    ))
    .expect("decodes");
    let second = settled_event_in_log(&settled_log_line(
        Pubkey::from([0xA2u8; 32]),
        Pubkey::default(),
        77,
        "flat white",
    ))
    .expect("decodes");

    store_settlements(&db, endpoint, &[first.clone(), second.clone()]).await;

    let markers = [
        ReceiptMarker {
            invoice: first.invoice,
            settled_at: 1,
        },
        ReceiptMarker {
            invoice: second.invoice,
            settled_at: 2,
        },
    ];
    let stored = stored_settlements(&db, endpoint, &markers).await;
    assert_eq!(stored.len(), 2);
    assert_eq!(stored[&first.invoice].amount, 42);
    assert_eq!(stored[&second.invoice].memo, "flat white");

    // A cluster the rows were not written for has nothing to answer with.
    assert!(stored_settlements(&db, "https://other.invalid", &markers)
        .await
        .is_empty());
    let _ = std::fs::remove_file(path);
}

/// Storing the same invoice twice updates the row instead of failing: a
/// settlement is read once, but an operator re-reading one must not run into
/// a primary-key error.
#[tokio::test]
async fn storing_a_settlement_twice_updates_it() {
    let (db, path) = temp_db().await;
    let endpoint = "https://settled-db-upsert.invalid";
    let invoice = Pubkey::from([0xC2u8; 32]);
    let line = settled_log_line(invoice, Pubkey::default(), 42, "tip");
    let settled = settled_event_in_log(&line).expect("decodes");
    store_settlement(&db, endpoint, &settled).await;
    store_settlement(
        &db,
        endpoint,
        &OnchainReceipt {
            memo: "flat white".into(),
            ..settled.clone()
        },
    )
    .await;

    let stored = stored_settlement(&db, endpoint, &invoice)
        .await
        .expect("stored");
    assert_eq!(stored.memo, "flat white");
    assert_eq!(stored.amount, 42);

    // Another cluster is another settlement, not the same row.
    assert!(stored_settlement(&db, "https://other.invalid", &invoice)
        .await
        .is_none());
    let _ = std::fs::remove_file(path);
}

/// A payment signed by a session key is the wallet's once the key is known:
/// the settle row is keyed by the payer, so the read asks for every key that
/// stands for the buyer.
#[tokio::test]
async fn settlements_a_session_key_paid_are_listed_for_its_wallet() {
    let (db, path) = temp_db().await;
    let endpoint = "https://allowance-b.invalid";
    let wallet = Pubkey::from([0x62u8; 32]);
    let session_key = Pubkey::from([0x63u8; 32]);
    let receipt = |invoice: [u8; 32], payer: Pubkey, settled_at: i64, memo: &str| OnchainReceipt {
        invoice: Pubkey::from(invoice),
        seller: Pubkey::from([0x69u8; 32]),
        reference: Pubkey::from([0x6Au8; 32]),
        mint: Pubkey::default(),
        amount: 4_500_000,
        memo: memo.to_string(),
        payer,
        created_at: settled_at - 60,
        settled_at,
    };
    let direct = receipt([0x64u8; 32], wallet, 100, "direct");
    let silent = receipt([0x65u8; 32], session_key, 200, "silent");
    store_settlements(&db, endpoint, &[direct, silent]).await;

    let direct_only = settlements_paid_by(&db, endpoint, "devnet", &wallet.to_string(), 10).await;
    assert_eq!(direct_only.len(), 1, "the wallet's own payment");
    assert_eq!(direct_only[0].memo, "direct");

    let both = settlements_paid_by_any(
        &db,
        endpoint,
        "devnet",
        &[wallet.to_string(), session_key.to_string()],
        10,
    )
    .await;
    assert_eq!(both.len(), 2, "the allowance payment is the buyer's too");
    assert!(
        both.iter().any(|record| record.memo == "silent"),
        "the silent payment is in the page"
    );
    let _ = std::fs::remove_file(path);
}
