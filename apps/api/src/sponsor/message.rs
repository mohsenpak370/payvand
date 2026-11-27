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
/// with `sponsor` after the system program (before the optional shop policy).
/// A request with no mint is `settle_payment`; anything else is
/// `settle_token_payment` with the three token accounts the client's own
/// builder uses. The shop policy trails only when the invoice names one — the
/// program's optional account takes the program id in that slot to mean "none",
/// which is a message the client never builds.
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

        let mut accounts = vec![
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
        if let Some(shop_policy) = params.shop_policy.as_ref() {
            accounts.push(AccountMeta::new_readonly(*shop_policy, false));
        }
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

    let mut accounts = vec![
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
    // Trailing and read-only, exactly as the program declares it: absent for a
    // shop with no policy, never a placeholder in its slot.
    if let Some(shop_policy) = params.shop_policy.as_ref() {
        accounts.push(AccountMeta::new_readonly(*shop_policy, false));
    }

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

pub(super) fn message_bytes(message: &VersionedMessage) -> ApiResult<Vec<u8>> {
    Ok(message.serialize())
}

#[cfg(test)]
mod tests;
