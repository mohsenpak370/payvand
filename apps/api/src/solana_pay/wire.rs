//! The unsigned `settle_payment` transaction, in the legacy wire format.

use sha2::{Digest, Sha256};
use solana_pubkey::Pubkey;

use super::invoice::program_id;

pub(super) struct SettlePaymentRequest<'a> {
    pub(super) payer: Pubkey,
    pub(super) payment: Pubkey,
    pub(super) seller: Pubkey,
    pub(super) reference: Pubkey,
    pub(super) amount: u64,
    pub(super) memo: &'a str,
    pub(super) blockhash: [u8; 32],
}

/// The unsigned transaction a wallet signs: one `settle_payment` instruction,
/// the wallet as fee payer and the only signer.
///
/// Built by hand in the legacy wire format because that is the one every
/// wallet agrees on. The fixture test in `wire/tests.rs` parses the bytes back, so a
/// change in layout cannot pass unnoticed.
pub(super) fn settle_payment_transaction(request: SettlePaymentRequest<'_>) -> Vec<u8> {
    let program = program_id().expect("the program id in this crate is valid");
    let system_program = crate::spl::system_program();

    // Account order follows the runtime's rules: signers first, then the
    // writable accounts, then the readonly ones. The payer is the only
    // signer, so the header reads 1 required signature and 3 readonly
    // unsigned accounts (reference, system program, program).
    let keys = [
        request.payer,
        request.payment,
        request.seller,
        request.reference,
        system_program,
        program,
    ];

    let mut data = Vec::with_capacity(8 + 8 + 4 + request.memo.len());
    data.extend_from_slice(&settle_payment_discriminator());
    data.extend_from_slice(&request.amount.to_le_bytes());
    data.extend_from_slice(&(request.memo.len() as u32).to_le_bytes());
    data.extend_from_slice(request.memo.as_bytes());

    let mut message = Vec::with_capacity(3 + 1 + keys.len() * 32 + 32 + 32 + data.len());
    message.extend_from_slice(&[1, 0, 3]);
    push_compact_u16(&mut message, keys.len() as u16);
    for key in keys {
        message.extend_from_slice(key.as_ref());
    }
    message.extend_from_slice(&request.blockhash);
    push_compact_u16(&mut message, 1);
    message.push(5); // program id index
    push_compact_u16(&mut message, 5);
    message.extend_from_slice(&[0, 1, 2, 3, 4]);
    push_compact_u16(&mut message, data.len() as u16);
    message.extend_from_slice(&data);

    // No signatures yet: the spec has the wallet fill the payer's slot (and
    // treat the message's fee payer as the account it was called with).
    let mut transaction = Vec::with_capacity(1 + message.len());
    push_compact_u16(&mut transaction, 0);
    transaction.extend_from_slice(&message);
    transaction
}

/// Anchor's instruction discriminator: the first 8 bytes of
/// `sha256("global:settle_payment")`.
fn settle_payment_discriminator() -> [u8; 8] {
    let digest = Sha256::digest(b"global:settle_payment");
    digest[..8].try_into().expect("8 byte slice")
}

/// Solana's compact-u16 ("shortvec") length prefix.
fn push_compact_u16(out: &mut Vec<u8>, mut value: u16) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        byte |= 0x80;
        out.push(byte);
    }
}

#[cfg(test)]
mod tests;
