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
    // The compute-unit limit, then the settle.
    assert_eq!(message.instructions.len(), 2);
    let instruction = &message.instructions[1];
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
    assert_eq!(instructions.len(), 2);
    let instruction = &instructions[1];
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

/// What the relayer signs is two instructions: a compute-unit limit with no
/// price, then the settle. A priority price would be a fee the program never
/// refunds, so the relayer must never sign one — and a limit is what keeps a
/// wallet from adding its own.
fn compute_budget_setters(message: &VersionedMessage) -> Vec<Vec<u8>> {
    let keys = message.static_account_keys();
    message
        .instructions()
        .iter()
        .filter(|instruction| {
            keys[instruction.program_id_index as usize] == compute_budget_program()
        })
        .map(|instruction| instruction.data.clone())
        .collect()
}

#[test]
fn the_relayer_signs_a_limit_and_never_a_priority_price() {
    let sponsor = Pubkey::new_from_array([9; 32]);
    for params in [
        params("tea"),
        Params {
            mint: None,
            ..params("tea")
        },
    ] {
        let v0 = expected_message(&params, &sponsor, Hash::default()).expect("compiles");
        let legacy = legacy_message(&params, &sponsor, Hash::default());
        for message in [&v0, &legacy] {
            let mut limit = vec![2u8];
            limit.extend_from_slice(&SETTLE_COMPUTE_UNITS.to_le_bytes());
            assert_eq!(compute_budget_setters(message), vec![limit]);
            assert_eq!(
                message.instructions().len(),
                2,
                "the limit and the settle, nothing else"
            );
            assert_eq!(
                message.static_account_keys()[0],
                sponsor,
                "the relayer pays the fee"
            );
            // Two signers: the relayer, then the buyer. The program reads two
            // signatures as a relayer settle and charges the seller for both.
            assert_eq!(message.header().num_required_signatures, 2);
        }
    }
}

/// `settle` compares bytes, which is only fair if the same fields always
/// compile to the same message. They do: one compiler, here.
#[test]
fn the_same_fields_compile_to_the_same_bytes() {
    let sponsor = Pubkey::new_from_array([9; 32]);
    let blockhash = Hash::new_from_array([4; 32]);
    let first = expected_message(&params("x"), &sponsor, blockhash).expect("compiles");
    let second = expected_message(&params("x"), &sponsor, blockhash).expect("compiles");
    assert_eq!(
        message_bytes(&first).unwrap(),
        message_bytes(&second).unwrap()
    );
    let other = expected_message(
        &Params {
            amount: 4_500_001,
            ..params("x")
        },
        &sponsor,
        blockhash,
    )
    .expect("compiles");
    assert_ne!(
        message_bytes(&first).unwrap(),
        message_bytes(&other).unwrap()
    );
}
