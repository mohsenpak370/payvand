use super::*;

#[test]
fn hands_out_only_test_network_tokens() {
    assert!(hands_out("devnet"));
    assert!(hands_out("localnet"));
    assert!(!hands_out("mainnet"));
    assert!(!hands_out("mainnet-beta"));
    assert!(!hands_out("testnet"));
}

#[test]
fn reads_the_amounts_from_the_environment_or_keeps_the_defaults() {
    std::env::remove_var("FAUCET_SOL_LAMPORTS");
    std::env::remove_var("FAUCET_TOKEN_BASE_UNITS");
    assert_eq!(sol_lamports(), DEFAULT_SOL_LAMPORTS);
    assert_eq!(token_base_units(), DEFAULT_TOKEN_BASE_UNITS);

    std::env::set_var("FAUCET_SOL_LAMPORTS", "0");
    assert_eq!(sol_lamports(), DEFAULT_SOL_LAMPORTS);
    std::env::set_var("FAUCET_SOL_LAMPORTS", "250000000");
    std::env::set_var("FAUCET_TOKEN_BASE_UNITS", "5000000");
    assert_eq!(sol_lamports(), 250_000_000);
    assert_eq!(token_base_units(), 5_000_000);
    std::env::remove_var("FAUCET_SOL_LAMPORTS");
    std::env::remove_var("FAUCET_TOKEN_BASE_UNITS");
}
