use super::*;

#[test]
fn rejects_malformed_emails() {
    for bad in [
        "",
        "nope",
        "a@b",
        "a@@b.com",
        "a b@c.com",
        "@b.com",
        "a@.com",
    ] {
        assert!(normalize_email(bad).is_err(), "should reject {bad:?}");
    }
}

#[test]
fn normalizes_a_valid_email() {
    let email = normalize_email("  Ada@Example.COM ").expect("valid");
    assert_eq!(email, "ada@example.com");
}

#[test]
fn enforces_the_password_length() {
    assert!(validate_password("short").is_err());
    assert!(validate_password("longenough").is_ok());
}

#[test]
fn hashes_and_verifies_passwords() {
    let hash = hash_password("correct horse battery staple").expect("hash");
    assert!(verify_password("correct horse battery staple", &hash));
    assert!(!verify_password("wrong password", &hash));
    assert!(!verify_password(
        "correct horse battery staple",
        "not-a-hash"
    ));
}
