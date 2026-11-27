use super::*;

async fn test_db() -> (libsql::Database, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!(
        "payvand-reusable-test-{}.db",
        crate::auth::random_id()
    ));
    let db = libsql::Builder::new_local(&path)
        .build()
        .await
        .expect("temp db");
    crate::db::migrate(&db).await.expect("migrate");
    (db, path)
}

fn seller_account(wallet: &str) -> crate::auth::User {
    crate::auth::User {
        id: "user-1".to_string(),
        email: "seller@example.com".to_string(),
        wallet: Some(wallet.to_string()),
        created_at: 0,
    }
}

fn offer(seller: &str) -> CreateReusableRequest {
    CreateReusableRequest {
        seller: seller.to_string(),
        currency: "usdc".to_string(),
        amount: 5_000_000,
        memo: "Album".to_string(),
        name: Some("Digital album".to_string()),
        description: Some("Five tracks".to_string()),
        shop_id: None,
        cluster: "devnet".to_string(),
        endpoint: None,
    }
}

/// The row is the thing that repeats: one stored offer, one invoice per
/// visit, and only the ones that are paid reach the seller's history.
#[tokio::test]
async fn an_offer_is_stored_and_mints_a_fresh_invoice_per_visit() {
    let (db, path) = test_db().await;
    let seller = "9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin";
    let link = create_link(&db, &seller_account(seller), offer(seller))
        .await
        .expect("stored offer");
    assert_eq!(link.path, format!("{LINK_PATH_PREFIX}{}", link.id));
    assert_eq!(link.amount, 5_000_000);

    let (first, _) = mint_invoice(&db, &link.id).await.expect("first invoice");
    let (second, _) = mint_invoice(&db, &link.id).await.expect("second invoice");

    assert_ne!(first.id, second.id);
    assert_eq!(first.options[0].amount, 5_000_000);
    assert_eq!(second.options[0].amount, 5_000_000);
    assert_eq!(first.seller, seller);
    assert_eq!(first.seller_user.as_deref(), Some("user-1"));

    // Every mint names the code it came from, which is what the history
    // filter reads.
    let connection = db.connect().expect("connect");
    let mut rows = connection
        .query(
            "SELECT reusable_id FROM invoice_meta WHERE id = ?1",
            libsql::params![first.options[0].payment_pda.clone()],
        )
        .await
        .expect("read");
    let row = rows.next().await.expect("row").expect("invoice");
    assert_eq!(row.get::<String>(0).expect("tag"), link.id);
    // An open cursor holds the file's read lock, and the writes below come
    // from other connections.
    drop(row);
    drop(rows);

    // Two visits, nothing sold: a lookup is not the seller's history.
    let recorded = crate::payments::recorded_invoices(&db, seller, "devnet", 10).await;
    assert!(
        recorded.is_empty(),
        "an unpaid visit must not be listed as a payment"
    );
    // It is watched instead, so the payment below is found without anyone
    // polling that invoice.
    let awaiting = crate::payments::reusable_invoices_awaiting(&db, seller, "devnet", 10).await;
    assert_eq!(awaiting.len(), 2, "both lookups are still watched");

    // Marking one settled puts exactly that one in the seller's history.
    connection
        .execute(
            "INSERT INTO settled_payments \
                (invoice, endpoint, seller, reference, mint, amount, memo, payer, \
                 created_at, settled_at) \
             VALUES (?1, 'https://api.devnet.solana.com', ?2, 'ref', '', 5000000, \
                     'Album', ?3, 0, 0)",
            libsql::params![
                first.options[0].payment_pda.clone(),
                seller.to_string(),
                "9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin"
            ],
        )
        .await
        .expect("settlement");

    let recorded = crate::payments::recorded_invoices(&db, seller, "devnet", 10).await;
    assert_eq!(recorded.len(), 1, "the paid visit is the seller's sale");
    assert_eq!(recorded[0].id, first.options[0].payment_pda);
    assert_eq!(
        crate::payments::reusable_invoices_awaiting(&db, seller, "devnet", 10)
            .await
            .len(),
        1,
        "only the unpaid one is still watched"
    );

    // A visit a feed saw settled but could not read the settlement of is
    // marked paid instead: history from then on, and no longer watched, so
    // newer visits cannot push it out of the seller's page.
    let second_id = second.options[0].payment_pda.clone();
    crate::payments::mark_reusable_paid(&db, std::slice::from_ref(&second_id)).await;
    let recorded = crate::payments::recorded_invoices(&db, seller, "devnet", 10).await;
    assert_eq!(
        recorded.len(),
        2,
        "a marked visit is a sale without its row"
    );
    assert!(recorded.iter().any(|record| record.id == second_id));
    assert!(
        crate::payments::reusable_invoices_awaiting(&db, seller, "devnet", 10)
            .await
            .is_empty(),
        "nothing paid is still watched"
    );

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn an_offer_pays_the_accounts_own_wallet_only() {
    let (db, path) = test_db().await;
    let stranger = "9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin";
    let error = create_link(&db, &seller_account("someone-else"), offer(stranger)).await;
    assert!(matches!(error, Err(ApiError::Forbidden(_))));

    // And a code nobody made is not invoiced.
    assert!(mint_invoice(&db, "missing").await.is_err());

    std::fs::remove_file(path).ok();
}
