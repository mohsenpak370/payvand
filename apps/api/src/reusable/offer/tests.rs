use super::*;
use std::str::FromStr;

fn stored() -> StoredLink {
    StoredLink {
        seller: "9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin".to_string(),
        seller_user: "user-1".to_string(),
        cluster: "devnet".to_string(),
        endpoint: String::new(),
        currency: "usdc".to_string(),
        amount: 5_000_000,
        memo: "Album".to_string(),
        name: "Digital album".to_string(),
        description: "Five tracks".to_string(),
    }
}

/// Two visitors of one code must never share an invoice address: the second
/// payment would be refused as already settled.
#[test]
fn every_visit_gets_its_own_invoice_address() {
    let link = stored();
    let request_for = |reference: String| invoice_request(&reference, &link.offer());

    let first = crate::payments::create_payment(request_for(fresh_reference().unwrap()))
        .expect("first invoice");
    let second = crate::payments::create_payment(request_for(fresh_reference().unwrap()))
        .expect("second invoice");

    assert_ne!(first.id, second.id);
    // The same offer either way: what the seller stored is what is charged.
    assert_eq!(first.options.len(), 1);
    assert_eq!(first.options[0].currency, "usdc");
    assert_eq!(first.options[0].amount, 5_000_000);
    assert_eq!(second.options[0].amount, 5_000_000);
    assert_eq!(first.memo, "Album");
    assert_eq!(first.name, "Digital album");
    assert_eq!(first.description, "Five tracks");
}

/// A reference is a seed in the invoice address, so it has to be a valid
/// 32-byte address of its own.
#[test]
fn a_reference_is_a_usable_address() {
    let reference = fresh_reference().unwrap();
    assert!(Pubkey::from_str(&reference).is_ok());
    assert_ne!(reference, fresh_reference().unwrap());
}
