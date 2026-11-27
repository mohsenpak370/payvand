//! The one settle message the relayer will sign, rebuilt from the request's
//! parameters rather than taken from the caller's transaction.

use sha2::{Digest, Sha256};
use solana_sdk::hash::Hash;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::message::{v0, VersionedMessage};
use solana_sdk::pubkey::Pubkey;

use crate::payments::{ApiError, ApiResult};
use crate::spl::{ata_for, system_program, token_program};

use super::params::Params;
use super::relayer::program_id;

fn pda(seeds: &[&[u8]], program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(seeds, program).0
}

/// Anchor's instruction selector: the first eight bytes of
/// `sha256("global:<name>")`.
fn discriminator(name: &str) -> [u8; 8] {
    let digest = Sha256::digest(format!("global:{name}").as_bytes());
    let mut out = [0u8; 8];
    out.copy_from_slice(&digest[..8]);
    out
}

/// The settle instruction as the IDL declares it, in the IDL's account order,
/// with `sponsor` last. A request with no mint is `settle_payment`; anything
/// else is `settle_token_payment` with the three token accounts the client's
/// own builder uses. The list is the whole instruction: nothing trails it.
pub(super) fn settle_instruction(params: &Params, sponsor: &Pubkey) -> Instruction {
    let program = program_id();
    let treasury = pda(&[b"treasury"], &program);
    let config = pda(&[b"config"], &program);
    let system = system_program();

    // A SOL invoice: no mint, no token accounts, and `sponsor` is the account
    // that funds both records, as the program requires.
    if params.mint.is_none() {
        let invoice = pda(
            &[
                b"payment",
                params.seller.as_ref(),
                params.reference.as_ref(),
                &params.amount.to_le_bytes(),
            ],
            &program,
        );
        let mut data = discriminator("settle_payment").to_vec();
        data.extend_from_slice(&params.amount.to_le_bytes());
        data.extend_from_slice(&(params.memo.len() as u32).to_le_bytes());
        data.extend_from_slice(params.memo.as_bytes());

        let accounts = vec![
            AccountMeta::new(params.buyer, true),
            AccountMeta::new(invoice, false),
            AccountMeta::new(params.seller, false),
            AccountMeta::new(
                pda(&[b"payment-receipt", invoice.as_ref()], &program),
                false,
            ),
            AccountMeta::new_readonly(params.reference, false),
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(treasury, false),
            AccountMeta::new_readonly(system, false),
            AccountMeta::new(*sponsor, true),
        ];
        return Instruction {
            program_id: program,
            accounts,
            data,
        };
    }

    let mint = params.mint.expect("checked above");
    let invoice = pda(
        &[
            b"token-payment",
            params.seller.as_ref(),
            params.reference.as_ref(),
            mint.as_ref(),
            &params.amount.to_le_bytes(),
        ],
        &program,
    );
    let token = token_program();

    let mut data = discriminator("settle_token_payment").to_vec();
    data.extend_from_slice(&params.amount.to_le_bytes());
    data.extend_from_slice(&(params.memo.len() as u32).to_le_bytes());
    data.extend_from_slice(params.memo.as_bytes());

    let accounts = vec![
        AccountMeta::new(params.buyer, true),
        AccountMeta::new(invoice, false),
        AccountMeta::new(params.seller, false),
        AccountMeta::new(ata_for(&params.seller, &mint), false),
        AccountMeta::new(ata_for(&params.buyer, &mint), false),
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new(
            pda(&[b"payment-receipt", invoice.as_ref()], &program),
            false,
        ),
        AccountMeta::new_readonly(params.reference, false),
        AccountMeta::new_readonly(config, false),
        AccountMeta::new(treasury, false),
        AccountMeta::new(ata_for(&treasury, &mint), false),
        AccountMeta::new_readonly(token, false),
        AccountMeta::new_readonly(system, false),
        AccountMeta::new(*sponsor, true),
    ];

    Instruction {
        program_id: program,
        accounts,
        data,
    }
}

/// The one message this module will ever sign: version 0, the sponsor as fee
/// payer, and the settle instruction above.
pub(super) fn expected_message(
    params: &Params,
    sponsor: &Pubkey,
    blockhash: Hash,
) -> ApiResult<VersionedMessage> {
    let instruction = settle_instruction(params, sponsor);
    let message = v0::Message::try_compile(sponsor, &[instruction], &[], blockhash)
        .map_err(|_| ApiError::Internal("the settle message could not be compiled".into()))?;
    Ok(VersionedMessage::V0(message))
}

/// What a version 0 message asks of one of its static accounts — whether it
/// must sign and whether it is locked writable — read from the header alone, the
/// same way the runtime reads it.
fn role(message: &v0::Message, index: usize) -> (bool, bool) {
    let header = &message.header;
    let signed = usize::from(header.num_required_signatures);
    if index < signed {
        let writable_signed =
            signed.saturating_sub(usize::from(header.num_readonly_signed_accounts));
        (true, index < writable_signed)
    } else {
        let unsigned = message.account_keys.len().saturating_sub(signed);
        let writable_unsigned =
            unsigned.saturating_sub(usize::from(header.num_readonly_unsigned_accounts));
        (false, index - signed < writable_unsigned)
    }
}

/// Whether the buyer's message asks the chain for exactly what the rebuilt one
/// does: the same fee payer, the same accounts each with the same role, and the
/// same instructions over them — the same program, account list and data.
///
/// Bytes are the wrong test. A version 0 message lists its accounts in an order
/// its compiler chooses within each role, and the web client's compiler (Kit)
/// sorts them by their base58 text while `solana-sdk` sorts them by their bytes:
/// the same settlement then serializes differently and a byte match refuses
/// nearly every one. Nothing the order changes is something the chain acts on,
/// so the comparison is over meaning, and every other part of the message —
/// header, blockhash, lookups, key count — still has to match exactly.
pub(super) fn same_settlement(ours: &VersionedMessage, theirs: &VersionedMessage) -> bool {
    let (VersionedMessage::V0(ours), VersionedMessage::V0(theirs)) = (ours, theirs) else {
        return false;
    };
    if ours.header != theirs.header
        || ours.recent_blockhash != theirs.recent_blockhash
        || !ours.address_table_lookups.is_empty()
        || !theirs.address_table_lookups.is_empty()
        || ours.account_keys.len() != theirs.account_keys.len()
        || ours.instructions.len() != theirs.instructions.len()
        || ours.account_keys.first() != theirs.account_keys.first()
    {
        return false;
    }

    // Every key of theirs is one of ours, once, in the same role. With equal
    // lengths and our keys distinct, that makes the two key sets the same.
    let index: std::collections::HashMap<&Pubkey, usize> = ours
        .account_keys
        .iter()
        .enumerate()
        .map(|(position, key)| (key, position))
        .collect();
    let mut seen = std::collections::HashSet::new();
    for (position, key) in theirs.account_keys.iter().enumerate() {
        let Some(&mine) = index.get(key) else {
            return false;
        };
        if !seen.insert(key) || role(ours, mine) != role(theirs, position) {
            return false;
        }
    }

    let key = |message: &v0::Message, position: u8| -> Option<Pubkey> {
        message.account_keys.get(usize::from(position)).copied()
    };
    ours.instructions
        .iter()
        .zip(theirs.instructions.iter())
        .all(|(mine, other)| {
            mine.data == other.data
                && mine.accounts.len() == other.accounts.len()
                && key(ours, mine.program_id_index).is_some()
                && key(ours, mine.program_id_index) == key(theirs, other.program_id_index)
                && mine
                    .accounts
                    .iter()
                    .zip(other.accounts.iter())
                    .all(|(a, b)| {
                        let a = key(ours, *a);
                        a.is_some() && a == key(theirs, *b)
                    })
        })
}

pub(super) fn message_bytes(message: &VersionedMessage) -> ApiResult<Vec<u8>> {
    Ok(message.serialize())
}

#[cfg(test)]
mod tests;
