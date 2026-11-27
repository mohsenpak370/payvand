use super::*;

fn sample_invoice() -> (Invoice, String) {
    let seller = Pubkey::new_from_array([2u8; 32]);
    let reference = Pubkey::new_from_array([3u8; 32]);
    let amount = 10_000_000u64;
    let (payment, _) = Pubkey::find_program_address(
        &[
            PAYMENT_SEED,
            seller.as_ref(),
            reference.as_ref(),
            &amount.to_le_bytes(),
        ],
        &program_id().expect("program id"),
    );
    let invoice = Invoice {
        v: QR_PAYLOAD_VERSION,
        id: payment.to_string(),
        seller: seller.to_string(),
        reference: reference.to_string(),
        amount_lamports: amount,
        memo: "Coffee".to_string(),
        name: "Flat white".to_string(),
        description: String::new(),
        seller_user: None,
        discount: None,
        cluster: "devnet".to_string(),
    };
    let token = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&invoice).expect("json"));
    (invoice, token)
}

async fn seeded_db() -> libsql::Database {
    let path = std::env::temp_dir().join(format!(
        "payvand-solana-pay-test-{}.db",
        crate::auth::random_id()
    ));
    let db = libsql::Builder::new_local(&path)
        .build()
        .await
        .expect("temp db");
    crate::db::migrate(&db).await.expect("migrate");
    db
}

async fn add_account(db: &libsql::Database, id: &str, email: &str, wallet: &str) {
    db.connect()
        .expect("connection")
        .execute(
            "INSERT INTO users (id, email, password_hash, wallet, created_at) \
             VALUES (?1, ?2, 'unused-in-tests', ?3, 0)",
            (id, email, wallet),
        )
        .await
        .expect("account");
}

async fn add_code(db: &libsql::Database, user_id: &str, code: &str, percent_off: i64) {
    db.connect()
        .expect("connection")
        .execute(
            "INSERT INTO discount_codes (id, user_id, code, percent_off, created_at) \
             VALUES (?1, ?2, ?3, ?4, 0)",
            (format!("{user_id}-{code}"), user_id, code, percent_off),
        )
        .await
        .expect("code");
}

/// The transaction built for a wallet must price the discount from the
/// invoice's own seller. `seller_user` in the token is editable by whoever
/// holds the code, so an attacker naming their own account and their own
/// 99% code must not make someone else's coffee cost 1%.
#[tokio::test]
async fn prices_the_discount_from_the_invoices_seller() {
    let db = seeded_db().await;
    let (mut invoice, _) = sample_invoice();
    let seller_wallet = invoice.seller.clone();
    add_account(&db, "seller-a", "a@example.com", &seller_wallet).await;
    add_code(&db, "seller-a", "SAVE10", 10).await;
    add_account(&db, "buyer-b", "b@example.com", "BuyerWalletB").await;
    add_code(&db, "buyer-b", "MINE99", 99).await;

    // The buyer's own code, with their own account named in the token.
    invoice.discount = Some(PaymentDiscount {
        code: "MINE99".into(),
        percent_off: 99,
    });
    invoice.seller_user = Some("buyer-b".into());
    assert!(
        invoice.payable_lamports(&db).await.is_err(),
        "a buyer's own code must not price the seller's invoice"
    );

    // The seller's own code, priced from the seller's list.
    invoice.discount = Some(PaymentDiscount {
        code: "save10".into(),
        percent_off: 99,
    });
    assert_eq!(
        invoice.payable_lamports(&db).await.expect("priced"),
        9_000_000
    );
}

#[test]
fn decodes_the_token_from_a_qr_code() {
    let (invoice, token) = sample_invoice();
    let decoded = decode_invoice(&token).expect("token decodes");
    assert_eq!(decoded.id, invoice.id);
    assert_eq!(decoded.amount_lamports, 10_000_000);
    assert_eq!(decoded.label(), "Flat white");
}

#[test]
fn rejects_an_amount_that_does_not_match_the_address() {
    let (mut invoice, _) = sample_invoice();
    invoice.amount_lamports += 1;
    let token = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&invoice).expect("json"));
    assert!(decode_invoice(&token).is_err());
}

#[test]
fn rejects_an_unknown_payload_version() {
    let (mut invoice, _) = sample_invoice();
    invoice.v = 99;
    let token = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&invoice).expect("json"));
    assert!(decode_invoice(&token).is_err());
}

#[test]
fn formats_amounts_without_trailing_zeroes() {
    assert_eq!(format_lamports(10_000_000), "0.01");
    assert_eq!(format_lamports(1_000_000_000), "1");
    assert_eq!(format_lamports(1_500_000_000), "1.5");
}
