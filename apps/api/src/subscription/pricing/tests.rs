use super::*;

fn rates(eur_per_sol: f64, usd_per_eur: f64) -> Rates {
    Rates {
        eur_per_sol,
        usd_per_eur,
        fetched_at: 1_760_000_000,
    }
}

#[test]
fn an_unread_rate_costs_the_prices_only() {
    let (quotes, rated_at, available) = quoted(None, "devnet");
    assert!(quotes.is_empty());
    assert_eq!(rated_at, 0);
    assert!(!available);

    let read = rates(125.0, 1.08);
    let (quotes, rated_at, available) = quoted(Some(&read), "devnet");
    assert_eq!(quotes.len(), CURRENCIES.len());
    assert_eq!(rated_at, read.fetched_at);
    assert!(available);
}

#[test]
fn prices_euro_at_par_and_the_others_by_rate() {
    // €1.70 at 1.08 USD per EUR, and at 125 EUR per SOL.
    let quotes = quotes_for(&rates(125.0, 1.08), "devnet");
    let by_currency = |name: &str| {
        quotes
            .iter()
            .find(|quote| quote.currency == name)
            .expect("currency")
            .amount
    };
    assert_eq!(by_currency("eurc"), 1_700_000);
    assert_eq!(by_currency("usdc"), 1_836_000);
    // 1.70 / 125 = 0.0136 SOL, rounded up to the lamport.
    assert_eq!(by_currency("sol"), 13_600_000);
    // The stablecoins carry the cluster's mint, SOL carries none.
    assert!(quotes
        .iter()
        .find(|q| q.currency == "sol")
        .unwrap()
        .mint
        .is_none());
    assert!(quotes
        .iter()
        .find(|q| q.currency == "usdc")
        .unwrap()
        .mint
        .is_some());
}

#[test]
fn rounds_up_so_the_euro_price_is_always_covered() {
    // 1.70 EUR at 130.333… EUR/SOL is 0.013043478… SOL — 13,043,478.26
    // lamports, so the ceiling is 13,043,479 and never the shaved 478.
    let quotes = quotes_for(&rates(130.333_333_33, 1.0), "devnet");
    let sol = quotes.iter().find(|q| q.currency == "sol").unwrap().amount;
    assert_eq!(sol, 13_043_479);
    assert!(sol as f64 >= 1.7 / 130.333_333_33 * LAMPORTS_PER_SOL);

    let quotes = quotes_for(&rates(1.0, 1.0), "devnet");
    assert_eq!(
        quotes.iter().find(|q| q.currency == "usdc").unwrap().amount,
        1_700_000
    );
}

#[test]
fn a_breakdown_never_quotes_zero() {
    let quotes = quotes_for(&rates(1e12, 1.0), "devnet");
    let sol = quotes.iter().find(|q| q.currency == "sol").unwrap().amount;
    assert_eq!(sol, 1);
}
