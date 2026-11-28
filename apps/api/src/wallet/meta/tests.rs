use super::super::fixtures::test_state;
use super::*;
use crate::payments::PaymentStatus;

/// The history reads a payment by the address that was paid, so the name,
/// the description and the receipt lines written when the code was made
/// have to come back for a token payment too — the modal is the only place
/// a seller sees what an invoice was for.
#[tokio::test]
async fn history_rows_carry_the_name_and_receipt_lines() {
    let (state, path) = test_state().await;
    let connection = state.db.connect().expect("connect");
    connection
        .execute(
            "INSERT INTO invoice_meta (id, seller, name, description, items, created_at) \
             VALUES ('paid-in-usdc', 'seller', 'Two flat whites', 'Oat milk', \
             '[{\"name\":\"Flat white\",\"quantity\":2,\"amount_lamports\":5000000}]', 1)",
            (),
        )
        .await
        .expect("row");
    drop(connection);

    let mut items = vec![MyTransaction {
        id: "paid-in-usdc".into(),
        direction: "sent".into(),
        counterparty: "seller".into(),
        currency: "usdc".into(),
        amount: 1_500_000,
        decimals: 6,
        amount_lamports: 0,
        memo: String::new(),
        name: String::new(),
        description: String::new(),
        items: Vec::new(),
        status: PaymentStatus::Pending,
        created_at_unix: 1,
        settled_at_unix: 0,
        fee: None,
    }];

    fill_invoice_meta(&state.db, &mut items).await;

    assert_eq!(items[0].name, "Two flat whites");
    assert_eq!(items[0].description, "Oat milk");
    assert_eq!(items[0].items.len(), 1, "the receipt line came back");
    assert_eq!(items[0].items[0].name, "Flat white");
    assert_eq!(items[0].items[0].quantity, 2);
    assert_eq!(items[0].items[0].amount_lamports, 5_000_000);

    let _ = std::fs::remove_file(path);
}
