use super::*;

/// A `SPONSOR_KEYPAIR` the API cannot use is the same as no sponsor: the
/// client falls back to asking for SOL rather than to a broken promise.
#[test]
fn an_unreadable_sponsor_key_is_no_sponsor() {
    std::env::set_var("SPONSOR_KEYPAIR", "not-a-key");
    assert!(sponsor_keypair().is_none());
    let generated = Keypair::new();
    std::env::set_var(
        "SPONSOR_KEYPAIR",
        serde_json::to_string(&generated.to_bytes().to_vec()).expect("json"),
    );
    assert!(sponsor_keypair().is_some());
    std::env::remove_var("SPONSOR_KEYPAIR");
    assert!(sponsor_keypair().is_none());
}
