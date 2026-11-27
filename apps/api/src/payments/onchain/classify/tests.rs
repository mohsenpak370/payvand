use super::*;
use crate::payments::onchain::accounts::{decode_payment, decode_token_payment};
use crate::payments::onchain::reader::discriminator_for;
use crate::payments::test_support::{config_data, receipt_data};
use crate::payments::{receipt_pda, PAYVAND_PROGRAM_ID};
use std::str::FromStr;

#[test]
fn the_program_scan_skips_the_config_account() {
    let data = config_data(30_000_000);
    // 184 bytes, the shape the program allocates.
    assert_eq!(data.len(), 184);

    // No decoder mistakes it for something the scan keeps.
    assert!(decode_payment(&data).is_none());
    assert!(decode_token_payment(&data).is_none());
    assert!(decode_receipt_marker(&data).is_none());
    assert!(decode_budget(&data).is_none());

    // And classification returns “nothing to keep” rather than panicking.
    let classified = classify_program_account(
        Pubkey::from([8u8; 32]),
        &data,
        "devnet".into(),
        "https://api.devnet.solana.com".into(),
    );
    assert!(
        classified.is_none(),
        "Config must not become a payment or budget"
    );
}

/// The skip is not a blanket “ignore everything”: markers and budgets in
/// the same scan still classify.
#[test]
fn the_program_scan_still_classifies_receipts_and_budgets() {
    let invoice = Pubkey::from([4u8; 32]);
    let marker = classify_program_account(
        receipt_pda(&Pubkey::from_str(PAYVAND_PROGRAM_ID).unwrap(), &invoice),
        &receipt_data(invoice, 200),
        "devnet".into(),
        "https://example".into(),
    );
    assert!(
        matches!(
            marker,
            Some(ScannedAccount::Settled(found))
                if found.invoice == invoice && found.settled_at == 200
        ),
        "a marker must classify as a settlement of the invoice it names, keeping its own time"
    );

    let mut budget = discriminator_for("Budget").to_vec();
    budget.extend_from_slice(&[1u8; 32]); // buyer
    budget.extend_from_slice(&[2u8; 32]); // session key
    budget.extend_from_slice(&10u64.to_le_bytes());
    budget.extend_from_slice(&0u64.to_le_bytes());
    budget.extend_from_slice(&1u64.to_le_bytes());
    budget.push(255);
    let classified = classify_program_account(
        Pubkey::from([3u8; 32]),
        &budget,
        "devnet".into(),
        "https://example".into(),
    );
    assert!(matches!(classified, Some(ScannedAccount::Budget(_))));
}

/// A token allowance classifies too: a payment made silently from a USDC or
/// EURC pot is signed by that session key, so without it the payment would
/// belong to nobody the history can name.
#[test]
fn the_program_scan_classifies_token_budgets() {
    let mut budget = discriminator_for("TokenBudget").to_vec();
    budget.extend_from_slice(&[1u8; 32]); // buyer
    budget.extend_from_slice(&[2u8; 32]); // session key
    budget.extend_from_slice(&[3u8; 32]); // mint
    budget.extend_from_slice(&10u64.to_le_bytes());
    budget.extend_from_slice(&0u64.to_le_bytes());
    budget.extend_from_slice(&1u64.to_le_bytes());
    budget.push(255);

    match classify_program_account(
        Pubkey::from([3u8; 32]),
        &budget,
        "devnet".into(),
        "https://example".into(),
    ) {
        Some(ScannedAccount::Budget(found)) => {
            assert_eq!(found.buyer, Pubkey::from([1u8; 32]));
            assert_eq!(found.session_key, Pubkey::from([2u8; 32]));
        }
        _ => panic!("a token budget must classify as a budget"),
    }
}
