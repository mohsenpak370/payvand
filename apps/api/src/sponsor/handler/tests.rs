use super::super::params::fixture as params;
use super::*;
use solana_sdk::signature::Keypair;

/// What `prepare` hands the page: the relayer's signature in slot 0 and the
/// buyer's slot empty.
fn prepared(params: &Params, sponsor: &Keypair) -> VersionedTransaction {
    let message = expected_message(params, &sponsor.pubkey(), Hash::new_from_array([5; 32]))
        .expect("compiles");
    let signature = sponsor.sign_message(&message_bytes(&message).unwrap());
    VersionedTransaction {
        signatures: vec![signature, Signature::default()],
        message,
    }
}

/// The buyer's wallet adding its signature to the bytes it was handed.
fn signed_by(mut transaction: VersionedTransaction, buyer: &Keypair) -> VersionedTransaction {
    transaction.signatures[1] = buyer.sign_message(&transaction.message.serialize());
    transaction
}

fn fields_for(buyer: &Keypair) -> Params {
    Params {
        buyer: buyer.pubkey(),
        ..params("latte")
    }
}

#[test]
fn a_prepared_settle_the_buyer_signed_is_accepted() {
    let (sponsor, buyer) = (Keypair::new(), Keypair::new());
    let params = fields_for(&buyer);
    let transaction = signed_by(prepared(&params, &sponsor), &buyer);
    let signature = check_prepared(&params, &sponsor, &transaction, "devnet").expect("accepted");
    assert_eq!(
        signature, transaction.signatures[0],
        "the id is the relayer's signature"
    );
}

/// The relayer signed first, so a wallet that rewrote the transaction — a
/// priority price, an extra instruction — sends something the relayer never
/// signed. It is refused here rather than paid for.
#[test]
fn a_transaction_the_wallet_changed_is_refused() {
    let (sponsor, buyer) = (Keypair::new(), Keypair::new());
    let params = fields_for(&buyer);
    let mut transaction = prepared(&params, &sponsor);
    if let VersionedMessage::V0(message) = &mut transaction.message {
        // setComputeUnitPrice(1_000) appended, as Phantom would.
        let program = message.instructions[0].program_id_index;
        let mut data = vec![3u8];
        data.extend_from_slice(&1_000u64.to_le_bytes());
        message.instructions.push(
            solana_sdk::message::compiled_instruction::CompiledInstruction {
                program_id_index: program,
                accounts: vec![],
                data,
            },
        );
    }
    let transaction = signed_by(transaction, &buyer);
    assert!(check_prepared(&params, &sponsor, &transaction, "devnet").is_err());
}

/// The same bytes with no buyer signature, or another key's, never reach the
/// cluster: the relayer would pay the fee of a transaction that fails.
#[test]
fn a_missing_or_wrong_buyer_signature_is_refused() {
    let (sponsor, buyer) = (Keypair::new(), Keypair::new());
    let params = fields_for(&buyer);
    assert!(check_prepared(&params, &sponsor, &prepared(&params, &sponsor), "devnet").is_err());
    let stranger = signed_by(prepared(&params, &sponsor), &Keypair::new());
    assert!(check_prepared(&params, &sponsor, &stranger, "devnet").is_err());
}

/// A settle for another invoice, or one another key signed as fee payer, is not
/// this relayer's settle.
#[test]
fn another_invoice_or_another_relayer_is_refused() {
    let (sponsor, buyer) = (Keypair::new(), Keypair::new());
    let params = fields_for(&buyer);
    let transaction = signed_by(prepared(&params, &sponsor), &buyer);
    let cheaper = Params {
        amount: params.amount - 1,
        ..fields_for(&buyer)
    };
    assert!(check_prepared(&cheaper, &sponsor, &transaction, "devnet").is_err());
    let elsewhere = signed_by(prepared(&params, &Keypair::new()), &buyer);
    assert!(check_prepared(&params, &sponsor, &elsewhere, "devnet").is_err());
}

/// The Solana Pay fallback is the program's real `settle_payment` — nine
/// accounts, the wallet as its own sponsor — behind a compute-unit limit, with
/// one empty signature slot for the wallet.
#[test]
fn the_self_paid_solana_pay_settle_is_the_programs_settle() {
    let buyer = Pubkey::new_from_array([1; 32]);
    let wire = self_paid_solana_pay_settle(&SolanaPaySettle {
        buyer,
        seller: Pubkey::new_from_array([2; 32]),
        reference: Pubkey::new_from_array([3; 32]),
        amount: 10_000_000,
        memo: "Coffee",
        cluster: "devnet",
        blockhash: Hash::new_from_array([7; 32]),
    })
    .expect("serializes");
    let transaction: VersionedTransaction = bincode::deserialize(&wire).expect("decodes");
    assert!(matches!(transaction.message, VersionedMessage::Legacy(_)));
    assert_eq!(transaction.signatures, vec![Signature::default()]);
    let keys = transaction.message.static_account_keys();
    assert_eq!(keys[0], buyer, "the wallet pays its own fee");
    let settle = transaction
        .message
        .instructions()
        .last()
        .expect("the settle");
    assert_eq!(settle.accounts.len(), 9);
    assert_eq!(
        keys[settle.accounts[8] as usize], buyer,
        "and is its own sponsor"
    );
    assert_eq!(
        transaction.message.instructions().len(),
        2,
        "limit, then settle"
    );
}
