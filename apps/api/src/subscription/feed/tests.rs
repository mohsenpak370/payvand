use super::super::pricing::quotes_for;
use super::*;

/// The real feeds over the network, through the production read path:
/// `cargo test --lib live_rates -- --ignored --nocapture`. Ignored by
/// default because it needs the internet, and because a feed's own outage
/// should not fail the suite — every feed that answers is checked, and at
/// least one has to.
#[tokio::test]
#[ignore = "reads the live rate feeds"]
async fn live_rates_price_the_plan() {
    let mut answered = 0;
    for url in rates_urls() {
        let Some(read) = read_rates(&url).await else {
            println!("no answer from {url}");
            continue;
        };
        answered += 1;
        println!(
            "{url} → {:.2} EUR/SOL, {:.4} USD/EUR",
            read.eur_per_sol, read.usd_per_eur
        );
        let quotes = quotes_for(&read, "devnet");
        let amount = |currency: &str| {
            quotes
                .iter()
                .find(|quote| quote.currency == currency)
                .map(|quote| quote.amount)
                .expect("the plan has a price in every currency")
        };
        // EURC is the plan's own currency, so it pays at par, exactly.
        assert_eq!(amount("eurc"), 1_700_000);
        // €1.70 at a sane euro-dollar rate is one to two dollars.
        assert!((1_500_000..=2_500_000).contains(&amount("usdc")));
        // And at any real SOL price it is a fraction of a SOL.
        assert!(amount("sol") > 5_000_000 && amount("sol") < 200_000_000);
    }
    assert!(answered > 0, "no rate feed answered");
}

#[test]
fn reads_the_feed_in_both_directions() {
    let (eur_per_sol, usd_per_eur) =
        parse_rates(r#"{"solana":{"eur":125.0,"usd":135.0}}"#).expect("parsed");
    assert_eq!(eur_per_sol, 125.0);
    assert!((usd_per_eur - 1.08).abs() < 1e-9);
    assert!(parse_rates(r#"{"solana":{"eur":0,"usd":135.0}}"#).is_none());
    assert!(parse_rates("not json").is_none());
}

#[test]
fn reads_the_fallback_feed_in_the_same_currency_pair() {
    // Coinbase's shape: decimal strings, one SOL's price in each currency.
    let body = r#"{"data":{"currency":"SOL","rates":{"EUR":"66.03","USD":"72.41"}}}"#;
    let (eur_per_sol, usd_per_eur) = parse_rates(body).expect("parsed");
    assert_eq!(eur_per_sol, 66.03);
    assert!((usd_per_eur - 72.41 / 66.03).abs() < 1e-12);

    // A zero, a missing price or a word where a number belongs is a broken
    // read rather than a free rate.
    assert!(parse_rates(r#"{"data":{"rates":{"EUR":"0","USD":"72.41"}}}"#).is_none());
    assert!(parse_rates(r#"{"data":{"rates":{"EUR":"66.03"}}}"#).is_none());
    assert!(parse_rates(r#"{"data":{"rates":{"EUR":"sixty","USD":"72.41"}}}"#).is_none());
}

#[test]
fn logs_a_bounded_slice_of_an_unexpected_answer() {
    assert_eq!(body_head("short"), "short");
    let long = "x".repeat(RATES_BODY_LOG_CHARS * 2);
    assert_eq!(body_head(&long).chars().count(), RATES_BODY_LOG_CHARS);
    // Counted in characters, so a multi-byte answer cannot split a code point.
    let unicode = "€".repeat(RATES_BODY_LOG_CHARS * 2);
    assert_eq!(body_head(&unicode).chars().count(), RATES_BODY_LOG_CHARS);
}
