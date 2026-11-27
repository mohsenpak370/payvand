use super::*;

#[test]
fn keeps_names_short_and_prices_positive() {
    assert_eq!(
        required_text("  Flat white ", 32, "Name").unwrap(),
        "Flat white"
    );
    assert!(required_text("   ", 32, "Name").is_err());
    assert!(required_text(&"x".repeat(33), 32, "Name").is_err());
    // Counted in characters, so a short non-ASCII name is not rejected.
    assert!(required_text("Crème brûlée", 12, "Name").is_ok());
    assert!(valid_price(0).is_err());
    assert!(valid_price(1).is_ok());
    assert!(valid_price(MAX_PRICE_LAMPORTS + 1).is_err());
    // A shop that names no currency charges in SOL, and only the three the
    // app can charge in are accepted.
    assert_eq!(valid_shop_currency(None).unwrap(), "sol");
    assert_eq!(valid_shop_currency(Some(" EURC ")).unwrap(), "eurc");
    assert!(valid_shop_currency(Some("gbp")).is_err());
}
