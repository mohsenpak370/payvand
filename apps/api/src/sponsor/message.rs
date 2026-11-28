//! The one settle message the relayer will sign, rebuilt from the request's
//! parameters rather than taken from the caller's transaction.

use std::str::FromStr;

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

/// The Compute Budget program.
pub(super) fn compute_budget_program() -> Pubkey {
    Pubkey::from_str("ComputeBudget111111111111111111111111111111")
        .expect("the compute budget id parses")
}

/// The compute-unit limit every sponsored settle names. Naming one is what tells
/// a wallet the transaction's budget is already decided: Phantom adds its own
/// limit and a priority price to a transaction carrying neither. A transaction
/// the relayer has already signed should not be touched at all — any change
/// breaks the relayer's signature and the cluster refuses it — so this is
/// belt and braces, and it carries no price: the relayer pays exactly the base
/// fee of two signatures, which is what the program refunds it.
pub(super) const SETTLE_COMPUTE_UNITS: u32 = 200_000;

/// `setComputeUnitLimit` (discriminator 2, a little-endian `u32`).
pub(super) fn compute_unit_limit_instruction() -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&SETTLE_COMPUTE_UNITS.to_le_bytes());
    Instruction {
        program_id: compute_budget_program(),
        accounts: vec![],
        data,
    }
}

/// The one message the relayer signs for `params`: version 0, the sponsor as
/// fee payer, a compute-unit limit, and the settle instruction above. Nothing
/// else — no price, no other instruction, no lookup table.
///
/// `prepare` signs this and `settle` refuses anything that is not exactly this,
/// byte for byte: both sides are compiled here, by the same code, so the bytes
/// are a fair test and nothing has to be tolerated.
pub(super) fn expected_message(
    params: &Params,
    sponsor: &Pubkey,
    blockhash: Hash,
) -> ApiResult<VersionedMessage> {
    let instructions = [
        compute_unit_limit_instruction(),
        settle_instruction(params, sponsor),
    ];
    let message = v0::Message::try_compile(sponsor, &instructions, &[], blockhash)
        .map_err(|_| ApiError::Internal("the settle message could not be compiled".into()))?;
    Ok(VersionedMessage::V0(message))
}

/// The same settlement as a legacy message, for a Solana Pay wallet: the
/// transaction-request flow hands the wallet a transaction it signs and sends
/// itself, and legacy is the format every such wallet reads.
pub(super) fn legacy_message(
    params: &Params,
    sponsor: &Pubkey,
    blockhash: Hash,
) -> VersionedMessage {
    let instructions = [
        compute_unit_limit_instruction(),
        settle_instruction(params, sponsor),
    ];
    let mut message = solana_sdk::message::Message::new(&instructions, Some(sponsor));
    message.recent_blockhash = blockhash;
    VersionedMessage::Legacy(message)
}

pub(super) fn message_bytes(message: &VersionedMessage) -> ApiResult<Vec<u8>> {
    Ok(message.serialize())
}

#[cfg(test)]
mod tests;
