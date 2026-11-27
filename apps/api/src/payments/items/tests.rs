use crate::payments::test_support::sample_request;
use crate::payments::{create_payment, CreatePaymentRequest, PaymentItem, MAX_ITEMS};

#[test]
fn carries_a_receipt_that_adds_up_to_the_price() {
    let request = CreatePaymentRequest {
        amount_lamports: 1_000_000,
        items: vec![
            PaymentItem {
                name: " Flat white ".into(),
                quantity: 2,
                amount_lamports: 400_000,
            },
            PaymentItem {
                name: "Croissant".into(),
                quantity: 1,
                amount_lamports: 200_000,
            },
        ],
        ..sample_request()
    };
    let record = create_payment(request).expect("valid receipt");
    assert_eq!(record.items.len(), 2);
    assert_eq!(record.items[0].name, "Flat white");

    let payload: serde_json::Value =
        serde_json::from_str(&record.qr_payload()).expect("payload is json");
    assert_eq!(payload["items"][0]["quantity"], 2);
    assert_eq!(payload["items"][1]["amount_lamports"], 200_000);
}

#[test]
fn refuses_a_receipt_that_does_not_add_up() {
    // 2 × 0.004 SOL is not the 0.01 SOL the invoice charges.
    let request = CreatePaymentRequest {
        items: vec![PaymentItem {
            name: "Flat white".into(),
            quantity: 2,
            amount_lamports: 400_000,
        }],
        ..sample_request()
    };
    assert!(create_payment(request).is_err());
}

#[test]
fn refuses_a_receipt_line_that_makes_no_sense() {
    let free = CreatePaymentRequest {
        items: vec![PaymentItem {
            name: "Coffee".into(),
            quantity: 0,
            amount_lamports: 1_000_000,
        }],
        ..sample_request()
    };
    assert!(create_payment(free).is_err());

    // One more line than a code can carry.
    let crowded = CreatePaymentRequest {
        amount_lamports: u64::from(MAX_ITEMS as u32 + 1),
        items: (0..=MAX_ITEMS)
            .map(|_| PaymentItem {
                name: "x".into(),
                quantity: 1,
                amount_lamports: 1,
            })
            .collect(),
        ..sample_request()
    };
    assert!(create_payment(crowded).is_err());
}
