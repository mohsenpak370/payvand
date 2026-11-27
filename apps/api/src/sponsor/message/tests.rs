use super::super::params::fixture as params;
use super::*;

/// The message the endpoint compares against is built here, so these are the
/// invariants the byte match rests on: the sponsor is the fee payer and a
/// signer, the instruction is the settle one, and the account list is the
/// program's, not something shorter.
#[test]
fn rebuilds_the_settle_message_the_program_expects() {
    let sponsor = Pubkey::new_from_array([9; 32]);
    let message = expected_message(&params("flat white"), &sponsor, Hash::default())
        .expect("the message compiles");
    let VersionedMessage::V0(message) = message else {
        panic!("the sponsored settle is a version 0 message");
    };
    assert_eq!(message.account_keys[0], sponsor, "the sponsor pays the fee");
    assert_eq!(message.instructions.len(), 1);
    let instruction = &message.instructions[0];
    let program = message.account_keys[instruction.program_id_index as usize];
    assert_eq!(program, program_id());
    assert_eq!(
        message.account_keys[instruction.accounts[0] as usize],
        params("").buyer,
        "the buyer is the first account"
    );
    // The sponsor is both the fee payer and the last account; a version 0
    // message keeps one key per address, so the account list ends at it.
    let last = *instruction.accounts.last().expect("the sponsor account");
    assert_eq!(message.account_keys[last as usize], sponsor);
    assert_eq!(instruction.accounts.len(), 14);
}

/// A shop invoice's settle has one more account, and the relayer's rebuild
/// must carry it: without it the message is a slot short and no sponsored
/// settle of a shop invoice could ever match what the buyer signed.
#[test]
fn a_shop_policy_travels_as_the_trailing_account() {
    let sponsor = Pubkey::new_from_array([9; 32]);
    let shop_policy = Pubkey::new_from_array([8; 32]);
    let plain = settle_instruction(&params(""), &sponsor);
    let mut shop = params("");
    shop.shop_policy = Some(shop_policy);
    let priced = settle_instruction(&shop, &sponsor);

    assert_eq!(priced.accounts.len(), plain.accounts.len() + 1);
    assert_eq!(priced.accounts.len(), 15);
    let last = priced.accounts.last().expect("the policy account");
    assert_eq!(last.pubkey, shop_policy);
    assert!(!last.is_writable, "the policy is read-only");
    assert!(!last.is_signer, "the relayer never signs for a seller");
    assert_eq!(
        &priced.accounts[..14],
        &plain.accounts[..],
        "everything before the policy is the wallet settle's own list"
    );
}

#[test]
fn the_instruction_is_the_idl_shape() {
    let sponsor = Pubkey::new_from_array([9; 32]);
    let instruction = settle_instruction(&params(""), &sponsor);
    assert_eq!(
        &instruction.data[..8],
        &discriminator("settle_token_payment")
    );
    assert_eq!(
        u64::from_le_bytes(instruction.data[8..16].try_into().expect("eight bytes")),
        4_500_000
    );
    // memo: a u32 length then the bytes
    assert_eq!(&instruction.data[16..20], &0u32.to_le_bytes());
    assert!(instruction.accounts[0].is_signer);
    assert!(instruction.accounts[13].is_signer);
}

/// A SOL invoice is settled by `settle_payment`, whose account list is the
/// program's — and the relayer, not the buyer, is its fee payer, which is
/// what keeps the buyer from needing lamports it does not have.
#[test]
fn rebuilds_the_sol_settle_message() {
    let sponsor = Pubkey::new_from_array([9; 32]);
    let params = Params {
        mint: None,
        ..params("coffee")
    };
    let message = expected_message(&params, &sponsor, Hash::default()).expect("compiles");
    let instructions = match &message {
        VersionedMessage::V0(message) => message.instructions.clone(),
        other => panic!("unexpected message version: {other:?}"),
    };
    assert_eq!(instructions.len(), 1);
    let instruction = &instructions[0];
    assert_eq!(instruction.data[..8], discriminator("settle_payment"));
    assert_eq!(
        instruction.accounts.len(),
        9,
        "payer, invoice, seller, receipt, reference, config, treasury, system, sponsor"
    );
    let keys = message.static_account_keys();
    assert_eq!(keys[0], sponsor, "the relayer is the fee payer");
    let account = |index: usize| keys[instruction.accounts[index] as usize];
    assert_eq!(account(0), params.buyer);
    assert_eq!(account(2), params.seller);
    assert_eq!(
        account(8),
        sponsor,
        "the sponsor is the account that funds the records"
    );
}
