use super::*;
use crate::payments::onchain::records::pending_record;
use crate::payments::test_support::temp_db;
use crate::payments::PaymentOption;
use solana_pubkey::Pubkey;

/// An invoice this API made is listed from its own row, with no cluster read
/// at all: the seller's feed should not have to scan the program to show
/// what it created.
#[tokio::test]
async fn an_invoice_we_created_is_listed_from_our_rows() {
    let (db, path) = temp_db().await;
    let invoice = Pubkey::from([0x51u8; 32]);
    let seller = Pubkey::from([0x52u8; 32]);
    let mut record = pending_record(invoice, "devnet".into(), "https://example".into());
    record.seller = seller.to_string();
    record.reference = Pubkey::from([0x53u8; 32]).to_string();
    record.memo = "flat white".into();
    record.name = "Invoice".into();
    record.options = vec![PaymentOption {
        currency: SOL_CURRENCY.to_string(),
        mint: String::new(),
        decimals: SOL_DECIMALS,
        amount: 4_500_000,
        payment_pda: invoice.to_string(),
    }];

    remember_invoice(&db, &record, Some("user-1"), None).await;

    let listed = recorded_invoices(&db, &record.seller, "devnet", 10).await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, invoice.to_string());
    assert_eq!(listed[0].seller, record.seller);
    assert_eq!(listed[0].options[0].amount, 4_500_000);
    assert_eq!(listed[0].memo, "flat white");
    assert!(
        recorded_invoices(&db, "someone-else", "devnet", 10)
            .await
            .is_empty(),
        "another wallet's feed is not served this row"
    );
    assert!(
        recorded_invoices(&db, &record.seller, "mainnet", 10)
            .await
            .is_empty(),
        "a devnet invoice is not a mainnet one"
    );

    // The row carries the account that made it, for the pages that ask by
    // account rather than by wallet.
    let connection = db.connect().expect("connect");
    let mut rows = connection
        .query(
            "SELECT seller_user, amount FROM invoice_meta WHERE id = ?1",
            libsql::params![invoice.to_string()],
        )
        .await
        .expect("read");
    let row = rows.next().await.expect("row").expect("invoice");
    assert_eq!(row.get::<String>(0).expect("account"), "user-1");
    assert_eq!(row.get::<i64>(1).expect("amount"), 4_500_000);

    let _ = std::fs::remove_file(path);
}

/// A reusable code's invoice is not history until it is paid: the seller's
/// page shows money, not the lookups a code's reloads mint. The unpaid rows
/// stay watched instead, so the payment is still found.
#[tokio::test]
async fn an_unpaid_reusable_invoice_is_watched_but_not_listed() {
    let (db, path) = temp_db().await;
    let invoice = Pubkey::from([0x61u8; 32]);
    let seller = Pubkey::from([0x62u8; 32]);
    let mut record = pending_record(invoice, "devnet".into(), "https://example".into());
    record.seller = seller.to_string();
    record.name = "Digital album".into();
    record.options = vec![PaymentOption {
        currency: SOL_CURRENCY.to_string(),
        mint: String::new(),
        decimals: SOL_DECIMALS,
        amount: 5_000_000,
        payment_pda: invoice.to_string(),
    }];

    remember_invoice(&db, &record, Some("user-1"), Some("code-1")).await;

    assert!(
        recorded_invoices(&db, &record.seller, "devnet", 10)
            .await
            .is_empty(),
        "a visit nobody paid is not the seller's history"
    );
    let awaiting = reusable_invoices_awaiting(&db, &record.seller, "devnet", 10).await;
    assert_eq!(awaiting.len(), 1, "but it is still watched");
    assert_eq!(awaiting[0].id, invoice.to_string());

    // Once the settlement is known the same row is history, and there is
    // nothing left to watch.
    let connection = db.connect().expect("connect");
    connection
        .execute(
            "INSERT INTO settled_payments \
                (invoice, endpoint, seller, reference, mint, amount, memo, payer, \
                 created_at, settled_at) \
             VALUES (?1, 'https://example', ?2, 'ref', '', 5000000, '', ?3, 0, 0)",
            libsql::params![
                invoice.to_string(),
                record.seller.clone(),
                Pubkey::from([0x63u8; 32]).to_string()
            ],
        )
        .await
        .expect("settlement");

    let listed = recorded_invoices(&db, &record.seller, "devnet", 10).await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, invoice.to_string());
    assert!(
        reusable_invoices_awaiting(&db, &record.seller, "devnet", 10)
            .await
            .is_empty(),
        "a paid invoice is no longer watched as unpaid"
    );

    // A one-time invoice is history from the moment it is made: only a
    // reusable lookup waits for a payment.
    let one_time = Pubkey::from([0x64u8; 32]);
    let mut plain = pending_record(one_time, "devnet".into(), "https://example".into());
    plain.seller = seller.to_string();
    plain.options = vec![PaymentOption {
        currency: SOL_CURRENCY.to_string(),
        mint: String::new(),
        decimals: SOL_DECIMALS,
        amount: 1_000,
        payment_pda: one_time.to_string(),
    }];
    remember_invoice(&db, &plain, Some("user-1"), None).await;
    assert_eq!(
        recorded_invoices(&db, &record.seller, "devnet", 10)
            .await
            .len(),
        2,
        "a one-time invoice is listed before anyone pays it"
    );

    let _ = std::fs::remove_file(path);
}
