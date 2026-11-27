use super::*;

#[test]
fn normalizes_codes() {
    assert_eq!(normalize_code(" save10 ").unwrap(), "SAVE10");
    assert!(normalize_code("ab").is_err());
    assert!(normalize_code("no spaces").is_err());
    assert!(normalize_code(&"x".repeat(33)).is_err());
}

#[test]
fn prices_the_discount() {
    assert_eq!(discounted_lamports(1_000_000_000, 10), 900_000_000);
    // Rounds down: the seller never under-collects by a fraction.
    assert_eq!(discounted_lamports(333, 50), 166);
    assert_eq!(discounted_lamports(999, 99), 9);
    // Even an out-of-range percent can never zero an invoice.
    assert!(discounted_lamports(1_000, 200) > 0);
}

#[test]
fn validates_percent() {
    assert!(validate_percent(0).is_err());
    assert!(validate_percent(100).is_err());
    assert_eq!(validate_percent(50).unwrap(), 50);
}
