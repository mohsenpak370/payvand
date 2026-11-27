//! The one settle message the relayer will sign, rebuilt from the request's
//! parameters rather than taken from the caller's transaction.

use std::str::FromStr;

use sha2::{Digest, Sha256};
use solana_sdk::hash::Hash;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::message::compiled_instruction::CompiledInstruction;
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

/// The Compute Budget program, where a wallet's own fee instructions live.
///
/// Phantom (and some other wallets) adds two of them at signing time to any
/// transaction that carries neither: a compute-unit limit and a priority-fee
/// price. They name no account and set no state, so they cannot move a lamport —
/// but they do change the message, and whoever pays the fee pays the price.
pub(super) fn compute_budget_program() -> Pubkey {
    Pubkey::from_str("ComputeBudget111111111111111111111111111111")
        .expect("the compute budget id parses")
}

/// The limit a transaction runs under when it sets none, which is what a
/// priority fee is priced against.
const DEFAULT_COMPUTE_UNITS: u64 = 200_000;

/// `setComputeUnitLimit` (discriminator 2, a `u32`) and `setComputeUnitPrice`
/// (discriminator 3, micro-lamports per unit) — the only two compute-budget
/// instructions this module will price. Anything else the wallet put there is a
/// shape it does not know, so the comparison refuses it rather than signing it.
enum BudgetSetter {
    Limit(u64),
    Price(u64),
}

fn budget_setter(instruction: &CompiledInstruction) -> Option<BudgetSetter> {
    match (instruction.data.first().copied()?, instruction.data.len()) {
        (2, 5) => Some(BudgetSetter::Limit(u64::from(u32::from_le_bytes(
            instruction.data[1..5].try_into().ok()?,
        )))),
        (3, 9) => Some(BudgetSetter::Price(u64::from_le_bytes(
            instruction.data[1..9].try_into().ok()?,
        ))),
        _ => None,
    }
}

/// What the wallet's own compute-budget instructions cost the fee payer, in
/// lamports: the price per unit times the limit (the default when the wallet
/// set none). Zero when the message carries none.
///
/// The sponsor pays the transaction's fee and is repaid the policy's network fee
/// and the two rents, so a priority fee is money it does not get back. The
/// endpoint refuses anything above its cap instead of signing it blind.
pub(super) fn priority_fee_lamports(message: &VersionedMessage) -> u64 {
    let VersionedMessage::V0(message) = message else {
        return 0;
    };
    let program = compute_budget_program();
    let mut limit = None;
    let mut price = 0u64;
    for instruction in &message.instructions {
        let named = message
            .account_keys
            .get(usize::from(instruction.program_id_index));
        if named != Some(&program) || !instruction.accounts.is_empty() {
            continue;
        }
        match budget_setter(instruction) {
            Some(BudgetSetter::Limit(units)) => limit = Some(units),
            Some(BudgetSetter::Price(micros)) => price = price.saturating_add(micros),
            None => {}
        }
    }
    let units = limit.unwrap_or(DEFAULT_COMPUTE_UNITS);
    // Round up: the runtime charges a whole lamport for any fraction of one.
    (price as u128 * units as u128)
        .div_ceil(1_000_000)
        .min(u128::from(u64::MAX)) as u64
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
/// same settlement over them — the same program, account list and data.
///
/// Bytes are the wrong test. A version 0 message lists its accounts in an order
/// its compiler chooses within each role, and the web client's compiler (Kit)
/// sorts them by their base58 text while `solana-sdk` sorts them by their bytes:
/// the same settlement then serializes differently and a byte match refuses
/// nearly every one.
///
/// A wallet is the second compiler to touch the message: Phantom adds its own
/// compute-unit limit and priority price to any transaction that carries
/// neither, at the moment it signs, so a message the settlement matches exactly
/// can still arrive with two instructions and the program's key that ours never
/// had. Dropping those from the comparison is safe because neither names an
/// account nor sets state, and `priority_fee_lamports` prices them for the
/// endpoint's cap. Everything else — the blockhash, the lookups, the settle
/// instruction, its account order, every other key's role — still has to match.
pub(super) fn same_settlement(ours: &VersionedMessage, theirs: &VersionedMessage) -> bool {
    let (VersionedMessage::V0(ours), VersionedMessage::V0(theirs)) = (ours, theirs) else {
        return false;
    };
    if ours.recent_blockhash != theirs.recent_blockhash
        || !ours.address_table_lookups.is_empty()
        || !theirs.address_table_lookups.is_empty()
        || ours.account_keys.first() != theirs.account_keys.first()
    {
        return false;
    }

    let budget = compute_budget_program();
    // The wallet's own fee instructions: no accounts, and one of the two shapes
    // this module can price. Anything else is not a difference we can read, so
    // it stays in the list the settlement must match.
    let settles: Vec<&CompiledInstruction> = theirs
        .instructions
        .iter()
        .filter(|instruction| {
            let named = theirs
                .account_keys
                .get(usize::from(instruction.program_id_index));
            if named == Some(&budget) && instruction.accounts.is_empty() {
                return budget_setter(instruction).is_none();
            }
            true
        })
        .collect();
    if ours.instructions.len() != settles.len() {
        return false;
    }

    // Every key of theirs is one of ours, once, in the same role. The wallet's
    // compute-budget program key is the only key allowed to be extra, and it is
    // only extra when a priced instruction above named it.
    let index: std::collections::HashMap<&Pubkey, usize> = ours
        .account_keys
        .iter()
        .enumerate()
        .map(|(position, key)| (key, position))
        .collect();
    let mut seen = std::collections::HashSet::new();
    for (position, key) in theirs.account_keys.iter().enumerate() {
        let Some(&mine) = index.get(key) else {
            if *key == budget {
                continue;
            }
            return false;
        };
        if !seen.insert(key) || role(ours, mine) != role(theirs, position) {
            return false;
        }
    }
    // Ours has no key theirs lacks: equal lengths counted after the one extra.
    if seen.len() != ours.account_keys.len() {
        return false;
    }

    let key = |message: &v0::Message, position: u8| -> Option<Pubkey> {
        message.account_keys.get(usize::from(position)).copied()
    };
    ours.instructions
        .iter()
        .zip(settles.iter())
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
