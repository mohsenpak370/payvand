use super::super::fixtures::wallet_fixture;
use super::*;
use crate::payments::{self, PaymentStatus};

/// A payment account as the chain would hand it over.
fn payment_fixture(
    id: &str,
    seller: &str,
    settled_by: Option<&str>,
    created: i64,
    settled: i64,
) -> PaymentRecord {
    PaymentRecord {
        id: id.to_string(),
        status: if settled > 0 {
            PaymentStatus::Settled
        } else {
            PaymentStatus::Pending
        },
        payment_pda: id.to_string(),
        seller: seller.to_string(),
        seller_user: None,
        reference: "Reference111".into(),
        amount_lamports: 1_000_000,
        options: Vec::new(),
        memo: format!("memo for {id}"),
        name: String::new(),
        description: String::new(),
        items: Vec::new(),
        discount: None,
        allow_discounts: false,
        cluster: "devnet".into(),
        endpoint: "https://api.devnet.solana.com".into(),
        settled_by: settled_by.map(str::to_string),
        created_at_unix: created,
        settled_at_unix: settled,
        fee: None,
    }
}

/// A buyer can hold more than one allowance — SOL, USDC and EURC each have
/// their own session key — and a payment from any of them is still theirs.
#[test]
fn payments_from_every_allowance_count_as_sent() {
    let (wallet, _) = wallet_fixture(1);
    let (other, _) = wallet_fixture(2);
    let (sol_session, _) = wallet_fixture(4);
    let (token_session, _) = wallet_fixture(6);
    let buyer = payments::parse_pubkey(&wallet, "address").expect("wallet");
    let budgets = [
        BudgetAccount {
            buyer,
            session_key: payments::parse_pubkey(&sol_session, "session").expect("session"),
        },
        BudgetAccount {
            buyer,
            session_key: payments::parse_pubkey(&token_session, "session").expect("session"),
        },
    ];

    let records = vec![
        payment_fixture("sol-pot", &other, Some(&sol_session), 100, 500),
        payment_fixture("token-pot", &other, Some(&token_session), 200, 600),
    ];

    let items = match_transactions(records, &budgets, &wallet);
    let ids: Vec<&str> = items.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(ids, vec!["token-pot", "sol-pot"], "every pot is ours");
}

#[test]
fn transactions_are_matched_and_labelled() {
    let (wallet, _) = wallet_fixture(1);
    let (other, _) = wallet_fixture(2);
    let (session, _) = wallet_fixture(4);

    let budget = BudgetAccount {
        buyer: payments::parse_pubkey(&wallet, "address").expect("wallet"),
        session_key: payments::parse_pubkey(&session, "address").expect("session"),
    };

    let records = vec![
        // Sold something: received.
        payment_fixture("aaa", &wallet, Some(&other), 100, 500),
        // Paid a seller from this wallet: sent.
        payment_fixture("bbb", &other, Some(&wallet), 200, 600),
        // Paid silently from this buyer's budget: sent, via session key.
        payment_fixture("ccc", &other, Some(&session), 300, 700),
        // Someone else's invoice: not ours.
        payment_fixture("ddd", &other, Some(&other), 400, 800),
        // Our unpaid invoice: still ours to see.
        payment_fixture("eee", &wallet, None, 50, 0),
    ];

    let items = match_transactions(records, std::slice::from_ref(&budget), &wallet);

    let ids: Vec<&str> = items.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(ids, vec!["ccc", "bbb", "aaa", "eee"], "newest first");

    assert_eq!(items[0].direction, "sent", "budget payment counts as sent");
    assert_eq!(items[1].direction, "sent");
    assert_eq!(items[2].direction, "received");
    assert_eq!(items[3].counterparty, "awaiting payment");

    // Without the budget, the silent payment is invisible.
    let without = match_transactions(
        vec![payment_fixture("ccc", &other, Some(&session), 300, 700)],
        &[],
        &wallet,
    );
    assert!(without.is_empty());
}
