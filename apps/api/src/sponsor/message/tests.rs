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

/// The rebuilt message with its static keys put in another order inside each
/// role group — what a compiler that sorts differently (Kit sorts base58 text,
/// `solana-sdk` sorts bytes) produces for the same settlement.
fn reordered(message: &VersionedMessage) -> VersionedMessage {
    let VersionedMessage::V0(message) = message else {
        panic!("version 0");
    };
    let header = message.header;
    let signed = usize::from(header.num_required_signatures);
    let readonly_unsigned = usize::from(header.num_readonly_unsigned_accounts);
    let total = message.account_keys.len();
    // Reverse the writable unsigned group and the read-only unsigned group.
    let mut order: Vec<usize> = (0..signed).collect();
    order.extend((signed..total - readonly_unsigned).rev());
    order.extend((total - readonly_unsigned..total).rev());
    let account_keys: Vec<Pubkey> = order.iter().map(|&old| message.account_keys[old]).collect();
    let new_index = |old: u8| order.iter().position(|&o| o == usize::from(old)).unwrap() as u8;
    let instructions = message
        .instructions
        .iter()
        .map(
            |instruction| solana_sdk::message::compiled_instruction::CompiledInstruction {
                program_id_index: new_index(instruction.program_id_index),
                accounts: instruction.accounts.iter().map(|&a| new_index(a)).collect(),
                data: instruction.data.clone(),
            },
        )
        .collect();
    VersionedMessage::V0(v0::Message {
        header,
        account_keys,
        recent_blockhash: message.recent_blockhash,
        instructions,
        address_table_lookups: vec![],
    })
}

fn v0_of(message: &mut VersionedMessage) -> &mut v0::Message {
    match message {
        VersionedMessage::V0(message) => message,
        _ => panic!("version 0"),
    }
}

/// The bug a buyer met above their allowance: the wallet signed the right
/// settlement compiled by Kit, its accounts in Kit's order, and a byte match
/// refused it. The same settlement in another order is the same settlement.
#[test]
fn the_same_settlement_in_another_account_order_is_accepted() {
    let sponsor = Pubkey::new_from_array([9; 32]);
    let ours = expected_message(&params("flat white"), &sponsor, Hash::default()).unwrap();
    let theirs = reordered(&ours);
    assert_ne!(
        message_bytes(&ours).unwrap(),
        message_bytes(&theirs).unwrap()
    );
    assert!(same_settlement(&ours, &theirs));
    assert!(same_settlement(&ours, &ours));

    let sol = Params {
        mint: None,
        ..params("coffee")
    };
    let ours = expected_message(&sol, &sponsor, Hash::default()).unwrap();
    assert!(same_settlement(&ours, &reordered(&ours)));
}

#[test]
fn a_different_settlement_is_refused_whatever_its_order() {
    let sponsor = Pubkey::new_from_array([9; 32]);
    let ours = expected_message(&params("flat white"), &sponsor, Hash::default()).unwrap();

    // Another price.
    let mut other = reordered(&ours);
    v0_of(&mut other).instructions[0].data[8] ^= 1;
    assert!(!same_settlement(&ours, &other));

    // Another account in a slot: the seller's token account swapped for a stranger's.
    let mut other = reordered(&ours);
    let position = v0_of(&mut other).instructions[0].accounts[3] as usize;
    v0_of(&mut other).account_keys[position] = Pubkey::new_from_array([7; 32]);
    assert!(!same_settlement(&ours, &other));

    // Two instruction accounts swapped: same keys, different meaning.
    let mut other = reordered(&ours);
    v0_of(&mut other).instructions[0].accounts.swap(2, 3);
    assert!(!same_settlement(&ours, &other));

    // A read-only account made writable by moving the header's boundary.
    let mut other = reordered(&ours);
    v0_of(&mut other).header.num_readonly_unsigned_accounts -= 1;
    assert!(!same_settlement(&ours, &other));

    // Another blockhash, or another fee payer.
    let mut other = reordered(&ours);
    v0_of(&mut other).recent_blockhash = Hash::new_from_array([5; 32]);
    assert!(!same_settlement(&ours, &other));
    let mut other = ours.clone();
    v0_of(&mut other).account_keys.swap(0, 1);
    assert!(!same_settlement(&ours, &other));
}
