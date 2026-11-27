use super::super::message::expected_message;
use super::super::params::fixture as params;
use super::*;
use solana_sdk::hash::Hash;
use solana_sdk::pubkey::Pubkey;

/// The refusal lines are only useful if they point somewhere: the offset
/// where two settles differ, and the shape that says which part of the
/// message it was.
#[test]
fn a_refusal_can_say_where_the_two_settles_differ() {
    assert_eq!(first_difference(b"abc", b"abc"), None);
    assert_eq!(first_difference(b"abc", b"abd"), Some(2));
    assert_eq!(first_difference(b"abc", b"abcd"), Some(3));
    assert_eq!(first_difference(b"", b"a"), Some(0));

    let sponsor = Pubkey::new_from_array([9; 32]);
    let message = expected_message(&params(""), &sponsor, Hash::default()).expect("compiles");
    let (accounts, instruction_accounts, data) = shape(&message);
    assert_eq!(instruction_accounts, 14, "the token settle's account list");
    assert_eq!(data, 20, "selector, amount and the memo's length prefix");
    assert!(
        accounts >= instruction_accounts,
        "the static keys also carry the program and the sponsor"
    );

    // An address is logged as given; a field nobody should see at length is
    // cut, so the request cannot choose how big the log line is.
    assert_eq!(logged("  devnet  "), "devnet");
    assert_eq!(logged(&"x".repeat(200)).len(), 64);
}

/// The mint a refusal names reads the same on both settle paths.
#[test]
fn a_refusal_names_the_mint_or_says_sol() {
    assert_eq!(
        mint_label(&params("")),
        crate::payments::mint_for("usdc", "devnet")
            .expect("USDC")
            .to_string()
    );
    assert_eq!(
        mint_label(&Params {
            mint: None,
            ..params("")
        }),
        "SOL"
    );
}
