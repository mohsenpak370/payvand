use crate::payments::test_support::sample_request;
use crate::payments::{create_payment, PaymentDiscount};

#[test]
fn omits_an_empty_name_and_description_from_the_payload() {
    let record = create_payment(sample_request()).expect("valid request");
    let payload: serde_json::Value =
        serde_json::from_str(&record.qr_payload()).expect("payload is json");
    assert!(payload.get("name").is_none());
    assert!(payload.get("description").is_none());
    assert!(payload.get("discount").is_none());
}

#[test]
fn carries_an_attached_discount_into_the_payload() {
    let mut record = create_payment(sample_request()).expect("valid request");
    record.discount = Some(PaymentDiscount {
        code: "SAVE10".to_string(),
        percent_off: 10,
    });

    let payload: serde_json::Value =
        serde_json::from_str(&record.qr_payload()).expect("payload is json");
    assert_eq!(payload["discount"]["code"], "SAVE10");
    assert_eq!(payload["discount"]["percent_off"], 10);
    // The invoice keeps its full price: the discount prices it, it does
    // not rewrite the amount the address was derived from.
    assert_eq!(payload["amount_lamports"], 1_000_000);
}

#[test]
fn a_sol_only_invoice_keeps_the_old_payload() {
    let record = create_payment(sample_request()).expect("valid request");
    assert_eq!(record.options.len(), 1);
    let payload: serde_json::Value =
        serde_json::from_str(&record.qr_payload()).expect("payload is json");
    assert!(payload.get("options").is_none());
    assert_eq!(payload["amount_lamports"], 1_000_000);
}
