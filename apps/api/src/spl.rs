//! The Solana system, SPL token and associated-token programs, as this API uses
//! them: their ids, the token-account address an owner holds a mint in, and
//! the handful of instructions the server builds itself.
//!
//! The instructions are written out rather than imported, because the SDK this
//! API pins no longer re-exports the system program's builders, and pulling the
//! SPL crates in for three byte layouts is more than they are worth.

use solana_pubkey::Pubkey;
use solana_sdk::instruction::{AccountMeta, Instruction};
use std::str::FromStr;

/// The classic SPL token program — the only one the app's invoices accept.
pub(crate) const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
/// The associated token account program.
pub(crate) const ATA_PROGRAM: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
pub(crate) const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";

pub(crate) fn token_program() -> Pubkey {
    Pubkey::from_str(TOKEN_PROGRAM).expect("the token program id parses")
}

pub(crate) fn ata_program() -> Pubkey {
    Pubkey::from_str(ATA_PROGRAM).expect("the ATA program id parses")
}

pub(crate) fn system_program() -> Pubkey {
    Pubkey::from_str(SYSTEM_PROGRAM).expect("the system program id parses")
}

/// The owner's associated token account under the classic token program, which
/// is the only program the app's invoices accept.
pub(crate) fn ata_for(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    let token = token_program();
    Pubkey::find_program_address(
        &[owner.as_ref(), token.as_ref(), mint.as_ref()],
        &ata_program(),
    )
    .0
}

/// A lamport transfer: the system program's `Transfer` instruction, which is the
/// tag `2` followed by the amount in little-endian bytes.
pub(crate) fn sol_transfer_ix(from: &Pubkey, to: &Pubkey, lamports: u64) -> Instruction {
    let mut data = Vec::with_capacity(12);
    data.extend_from_slice(&2u32.to_le_bytes());
    data.extend_from_slice(&lamports.to_le_bytes());
    Instruction {
        program_id: system_program(),
        accounts: vec![AccountMeta::new(*from, true), AccountMeta::new(*to, false)],
        data,
    }
}

/// "Create the account if it does not exist", so a wallet that already holds the
/// token is not an error. `payer` funds the rent.
pub(crate) fn create_ata_ix(payer: &Pubkey, owner: &Pubkey, mint: &Pubkey) -> Instruction {
    Instruction {
        program_id: ata_program(),
        accounts: vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new(ata_for(owner, mint), false),
            AccountMeta::new_readonly(*owner, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new_readonly(system_program(), false),
            AccountMeta::new_readonly(token_program(), false),
        ],
        // `AssociatedTokenAccountInstruction::CreateIdempotent`
        data: vec![1],
    }
}

/// `TokenInstruction::Transfer`, which is the instruction tag then the amount in
/// little-endian base units.
pub(crate) fn token_transfer_ix(
    source: &Pubkey,
    destination: &Pubkey,
    authority: &Pubkey,
    amount: u64,
) -> Instruction {
    let mut data = Vec::with_capacity(9);
    data.push(3);
    data.extend_from_slice(&amount.to_le_bytes());
    Instruction {
        program_id: token_program(),
        accounts: vec![
            AccountMeta::new(*source, false),
            AccountMeta::new(*destination, false),
            AccountMeta::new_readonly(*authority, true),
        ],
        data,
    }
}

#[cfg(test)]
mod tests;
