use super::*;

/// The endpoint travels inside a payment code, which is the shape a request
/// forgery attempt takes: a stranger's QR pointing the API at whatever it
/// can reach.
#[test]
fn refuses_an_endpoint_outside_the_allowlist() {
    for hostile in [
        "http://169.254.169.254/latest/meta-data",
        "http://127.0.0.1:5432",
        "https://api.devnet.solana.com@evil.example",
        "https://evil.example/",
        "ftp://api.devnet.solana.com",
        "file:///etc/passwd",
    ] {
        assert!(
            resolve_endpoint(Some(hostile), "devnet").is_err(),
            "{hostile} should not be fetched"
        );
    }
}

/// The checkout reads invoice state, not settlement history: `finalized`
/// trails the `confirmed` a wallet waits for by a dozen seconds, and a paid
/// invoice came back `pending` for exactly that long.
#[test]
fn invoice_reads_use_confirmed_commitment() {
    let rpc = payment_rpc("https://api.devnet.solana.com".into());
    assert_eq!(rpc.commitment(), CommitmentConfig::confirmed());
}

#[test]
fn accepts_the_endpoints_the_operator_already_trusts() {
    assert_eq!(
        resolve_endpoint(Some("https://api.devnet.solana.com"), "devnet").unwrap(),
        "https://api.devnet.solana.com"
    );
    // A path and a port on the same origin are still that origin.
    assert!(resolve_endpoint(Some("http://127.0.0.1:8899/"), "localnet").is_ok());
    // With no endpoint in the code, the cluster decides.
    assert_eq!(
        resolve_endpoint(None, "devnet").unwrap(),
        "https://api.devnet.solana.com"
    );
}
