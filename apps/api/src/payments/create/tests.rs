use super::*;
use crate::payments::test_support::sample_request;
use crate::payments::PaymentStatus;

#[test]
fn accepts_a_valid_request() {
    let record = create_payment(sample_request()).expect("valid request");
    assert_eq!(record.status, PaymentStatus::Pending);
    assert_eq!(record.endpoint, "https://api.devnet.solana.com");
}

#[test]
fn rejects_zero_amount() {
    let request = CreatePaymentRequest {
        amount_lamports: 0,
        ..sample_request()
    };
    assert!(create_payment(request).is_err());
}

#[test]
fn keeps_a_name_and_description_for_the_buyer() {
    let request = CreatePaymentRequest {
        name: Some("  Coffee for two  ".to_string()),
        description: Some("Two flat whites at the corner shop".to_string()),
        ..sample_request()
    };
    let record = create_payment(request).expect("valid request");
    assert_eq!(record.name, "Coffee for two");
    assert_eq!(record.description, "Two flat whites at the corner shop");

    let payload: serde_json::Value =
        serde_json::from_str(&record.qr_payload()).expect("payload is json");
    assert_eq!(payload["name"], "Coffee for two");
    assert_eq!(payload["description"], "Two flat whites at the corner shop");
}

#[test]
fn still_accepts_requests_without_a_name_or_description() {
    let request: CreatePaymentRequest = serde_json::from_value(serde_json::json!({
        "payment_pda": "4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J",
        "seller": "4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J",
        "reference": "4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J",
        "amount_lamports": 1_000_000,
        "memo": "coffee",
        "cluster": "devnet"
    }))
    .expect("an older client's request still deserializes");

    let record = create_payment(request).expect("valid request");
    assert!(record.name.is_empty());
    assert!(record.description.is_empty());
}

#[test]
fn rejects_an_overlong_name_or_description() {
    let long_name = CreatePaymentRequest {
        name: Some("n".repeat(MAX_NAME_LEN + 1)),
        ..sample_request()
    };
    assert!(create_payment(long_name).is_err());

    let long_description = CreatePaymentRequest {
        description: Some("d".repeat(MAX_DESCRIPTION_LEN + 1)),
        ..sample_request()
    };
    assert!(create_payment(long_description).is_err());
}

#[test]
fn rejects_unknown_cluster_without_endpoint() {
    let request = CreatePaymentRequest {
        cluster: "moon".to_string(),
        ..sample_request()
    };
    assert!(create_payment(request).is_err());
}
