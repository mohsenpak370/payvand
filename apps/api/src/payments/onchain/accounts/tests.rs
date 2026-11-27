use super::*;
use crate::payments::onchain::records::marker_record;
use crate::payments::test_support::{old_receipt_data, receipt_data};
use crate::payments::PaymentStatus;

#[test]
fn decodes_a_payment_account() {
    // discriminator + seller + reference + amount + memo + state + payer
    // + created_at + settled_at + bump
    let mut data = account_discriminator().to_vec();
    data.extend_from_slice(&[7u8; 32]); // seller
    data.extend_from_slice(&[9u8; 32]); // reference
    data.extend_from_slice(&42u64.to_le_bytes());
    data.extend_from_slice(&3u32.to_le_bytes());
    data.extend_from_slice(b"tip");
    data.push(0); // pending
    data.extend_from_slice(&[0u8; 32]); // payer
    data.extend_from_slice(&100i64.to_le_bytes());
    data.extend_from_slice(&0i64.to_le_bytes());

    let payment = decode_payment(&data).expect("decodes");
    assert_eq!(payment.amount, 42);
    assert_eq!(payment.memo, "tip");
    assert_eq!(payment.state, 0);
    assert_eq!(payment.created_at, 100);
}

#[test]
fn rejects_a_foreign_account() {
    assert!(decode_payment(&[0u8; 100]).is_none());
}

#[test]
fn decodes_a_token_payment_account() {
    let mut data = discriminator_for("TokenPayment").to_vec();
    data.extend_from_slice(&[7u8; 32]); // seller
    data.extend_from_slice(&[9u8; 32]); // reference
    data.extend_from_slice(&[3u8; 32]); // mint
    data.extend_from_slice(&42u64.to_le_bytes());
    data.extend_from_slice(&2u32.to_le_bytes());
    data.extend_from_slice(b"hi");
    data.push(1); // settled
    data.extend_from_slice(&[5u8; 32]); // payer
    data.extend_from_slice(&100i64.to_le_bytes());
    data.extend_from_slice(&200i64.to_le_bytes());

    let payment = decode_token_payment(&data).expect("decodes");
    assert_eq!(payment.amount, 42);
    assert_eq!(payment.state, 1);
    assert_eq!(payment.settled_at, 200);

    // A SOL invoice must not decode as a token invoice and vice versa.
    assert!(decode_payment(&data).is_none());
}

#[test]
fn the_marker_is_only_the_guard_and_the_invoice() {
    let invoice = Pubkey::from([4u8; 32]);
    let data = receipt_data(invoice, 200);

    // 48 bytes is the whole account: the program's `PAYMENT_RECEIPT_SPACE`.
    assert_eq!(data.len(), 48);
    let marker = decode_receipt_marker(&data).expect("decodes");
    assert_eq!(marker.invoice, invoice);
    assert_eq!(marker.settled_at, 200);

    // The guard has to be able to say "already settled" on its own, because
    // that is the fallback when the history read fails.
    let record = marker_record(marker, "devnet".into(), "https://example".into());
    assert_eq!(record.status, PaymentStatus::Settled);
    assert_eq!(record.id, invoice.to_string());
    assert_eq!(record.settled_at_unix, 200);
    assert!(record.settled_by.is_none());
}

/// The IDL publishes the settle event's discriminator; this pins the bytes
/// the cluster actually logs, so a prefix mix-up cannot hide every payment
/// again.
#[test]
fn the_settle_event_discriminator_matches_the_idl() {
    assert_eq!(
        event_discriminator_for("PaymentSettled"),
        [158, 182, 152, 76, 105, 23, 232, 135]
    );
    assert_ne!(
        event_discriminator_for("PaymentSettled"),
        discriminator_for("PaymentSettled"),
        "an event and an account of the same name are prefixed differently"
    );
}

#[test]
fn refuses_to_read_a_receipt_out_of_a_foreign_account() {
    assert!(decode_receipt_marker(&[0u8; 48]).is_none());
    // An invoice account is not a receipt either.
    let mut invoice = account_discriminator().to_vec();
    invoice.extend_from_slice(&[0u8; 48]);
    assert!(decode_receipt_marker(&invoice).is_none());
    // A receipt from the old program is a different layout under the same
    // discriminator: read as a marker, its seller would land in
    // `settled_at`. The length check is what refuses it.
    let old = old_receipt_data(Pubkey::from([4u8; 32]));
    assert_eq!(old.len(), 301);
    assert!(decode_receipt_marker(&old).is_none());

    // A truncated marker does not panic and does not decode.
    assert!(decode_receipt_marker(&receipt_data(Pubkey::from([4u8; 32]), 1)[..20]).is_none());
    // Neither does a longer account, which is what the exact length means.
    let mut longer = receipt_data(Pubkey::from([4u8; 32]), 1);
    longer.push(0);
    assert!(decode_receipt_marker(&longer).is_none());
}
