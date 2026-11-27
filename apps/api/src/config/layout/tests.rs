use super::*;
use crate::config::test_support::policy_bytes;

#[test]
fn decodes_the_policy_the_program_writes() {
    let data = policy_bytes();
    assert_eq!(data.len(), CONFIG_SPACE);
    let policy = decode_policy(&data, "devnet").expect("a policy");

    assert_eq!(policy.fees.fee_bps_below, 200);
    assert_eq!(policy.fees.fee_bps_above, 100);
    assert_eq!(policy.fees.sol_tier_lamports, 100_000_000);
    assert_eq!(policy.fees.token_tier_base_units, 10_000_000);
    assert_eq!(policy.fees.flat_usdc_base_units, 62_782);
    assert_eq!(policy.fees.flat_eurc_base_units, 57_346);
    assert_eq!(policy.fees.network_fee_lamports, 5_000);
    assert_eq!(policy.fees.network_fee_usdc_base_units, 351);
    assert_eq!(policy.fees.network_fee_eurc_base_units, 320);
    assert_eq!(policy.fee_mode, "buyer");
    assert_eq!(policy.session_fee_float, 12_000_000);
    assert_eq!(
        policy.mints.usdc,
        Pubkey::from([4u8; 32]).to_string(),
        "the mint the account names"
    );
    assert_eq!(policy.mints.eurc, Pubkey::from([5u8; 32]).to_string());
    assert_eq!(policy.spaces.payment, 230);
    assert_eq!(policy.spaces.token_budget, 129);
    assert_eq!(policy.cluster, "devnet");
    assert_eq!(policy.program_id, payments::PAYVAND_PROGRAM_ID);
}

/// The mints a policy names are the ones a quote prices against; one that
/// names none falls back to the cluster's own, so a client is never handed an
/// empty mint it would price a token payment at.
#[test]
fn a_policy_that_names_no_mint_prices_the_clusters() {
    let mut data = policy_bytes();
    data[76..108].fill(0);
    data[108..140].fill(0);
    let policy = decode_policy(&data, "devnet").expect("a policy");
    assert_eq!(
        policy.mints.usdc,
        payments::mint_for("usdc", "devnet")
            .expect("USDC on devnet")
            .to_string()
    );
    assert_eq!(
        policy.mints.eurc,
        payments::mint_for("eurc", "devnet")
            .expect("EURC on devnet")
            .to_string()
    );
}

/// Anything that is not this program's `Config` is refused rather than
/// quoted: a wrong fee is a transaction that fails, or charges the wrong
/// amount.
#[test]
fn a_short_or_foreign_account_is_not_a_policy() {
    assert!(decode_policy(&policy_bytes()[..100], "devnet").is_none());

    let mut foreign = policy_bytes();
    foreign[0] ^= 0xFF;
    assert!(decode_policy(&foreign, "devnet").is_none());
}
