use super::super::accounts::decode_receipt_marker;
use super::super::reader::account_discriminator;
use super::*;
use crate::payments::onchain::accounts::settled_event_in_log;
use crate::payments::test_support::{receipt_data, settled_log_line};
use crate::payments::{mint_for, PaymentStatus, STABLE_DECIMALS};

#[test]
fn a_marker_wins_over_a_leftover_invoice_account() {
    let invoice = Pubkey::from([4u8; 32]);

    // An old-program invoice account still sitting on chain. A marker in the
    // same read is the answer, whichever transaction left the invoice there:
    // the read branches on the marker before it looks at invoice data.
    let mut stale = account_discriminator().to_vec();
    stale.extend_from_slice(&[7u8; 32]); // seller
    stale.extend_from_slice(&[9u8; 32]); // reference
    stale.extend_from_slice(&42u64.to_le_bytes());
    stale.extend_from_slice(&3u32.to_le_bytes());
    stale.extend_from_slice(b"tip");
    stale.push(0); // pending
    stale.extend_from_slice(&[0u8; 32]); // payer
    stale.extend_from_slice(&100i64.to_le_bytes());
    stale.extend_from_slice(&0i64.to_le_bytes());
    assert!(record_for(invoice, &stale, "devnet".into(), "https://example".into()).is_some());

    let marker = decode_receipt_marker(&receipt_data(invoice, 200)).expect("decodes");
    let record = marker_record(marker, "devnet".into(), "https://example".into());
    assert_eq!(record.status, PaymentStatus::Settled);
    assert_eq!(record.settled_at_unix, 200);
}

#[test]
fn a_sol_settle_event_reads_back_as_a_settled_invoice() {
    let invoice = Pubkey::from([4u8; 32]);
    let line = settled_log_line(invoice, Pubkey::default(), 42, "tip");

    let settled = settled_event_in_log(&line).expect("decodes");
    let record = receipt_record(settled, "devnet".into(), "https://example".into());
    assert_eq!(record.status, PaymentStatus::Settled);
    // The invoice address is the identity, not the marker's own address.
    assert_eq!(record.id, invoice.to_string());
    assert_eq!(record.amount_lamports, 42);
    assert_eq!(record.options[0].currency, "sol");
    assert!(record.options[0].mint.is_empty());
    assert_eq!(record.options[0].amount, 42);
    assert_eq!(record.memo, "tip");
    assert_eq!(record.settled_by, Some(Pubkey::from([5u8; 32]).to_string()));
}

#[test]
fn a_token_settle_event_reads_back_in_its_mint() {
    let invoice = Pubkey::from([4u8; 32]);
    let usdc = mint_for("usdc", "devnet").expect("usdc on devnet");
    let line = settled_log_line(invoice, usdc, 5_000_000, "flat white");

    let settled = settled_event_in_log(&line).expect("decodes");
    let record = receipt_record(settled, "devnet".into(), "https://example".into());
    assert_eq!(record.status, PaymentStatus::Settled);
    assert_eq!(record.amount_lamports, 0);
    assert_eq!(record.options[0].currency, "usdc");
    assert_eq!(record.options[0].amount, 5_000_000);
    assert_eq!(record.options[0].decimals, STABLE_DECIMALS);
    assert_eq!(record.options[0].mint, usdc.to_string());
}

/// A settled record reports no split, and that is a contract, not an
/// omission: `0002` revision 15 records why. The event now read here carries
/// `fee` and `seller_proceeds`, but publishing them would be a new field with
/// a new meaning for every consumer of this record — a decision of its own,
/// not a side effect of where the settlement is stored. A test forbids them
/// appearing by accident, and pins the fields the record does promise.
#[test]
fn a_settled_record_claims_no_split() {
    let invoice = Pubkey::from([4u8; 32]);
    let line = settled_log_line(invoice, Pubkey::default(), 100_000_000, "tip");
    let settled = settled_event_in_log(&line).expect("decodes");
    let record = receipt_record(settled, "devnet".into(), "https://example".into());

    let json = serde_json::to_value(&record).expect("serializes");
    for absent in [
        "fee",
        "fee_lamports",
        "seller_proceeds",
        "receipt_rent_lamports",
    ] {
        assert!(
            json.get(absent).is_none(),
            "a settled record must not report {absent}"
        );
    }
    // What it does promise: the price it was settled at, and who signed.
    assert_eq!(json["amount_lamports"], 100_000_000);
    assert_eq!(json["status"], "settled");
    assert_eq!(json["settled_by"], Pubkey::from([5u8; 32]).to_string());
}
