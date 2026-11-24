//! The program's events: the structured form of the `msg!` lines beside them.
//!
//! Each one is emitted with `emit!` right after the human-readable `msg!`, so an
//! explorer still reads a sentence while an indexer gets the fields — above all
//! the split, which it would otherwise have to re-derive from `Config` at the
//! block the payment landed in.
//!
//! `#[event]` puts each type in the IDL's `events` section. On the wire an event
//! is one `Program data: <base64>` log line: the 8-byte event discriminator
//! followed by the borsh body.

use anchor_lang::prelude::*;

use crate::state::{FeeMode, FeePolicy};

/// One settled invoice — from the wallet path or an allowance, in SOL or a
/// token.
///
/// This event is the record. The permanent `PaymentReceipt` account keeps only
/// the replay guard and a pointer back to the invoice, so everything a ledger,
/// a seller's screen or the API needs to describe a settlement is here: it
/// lands in the same transaction, it cannot be rewritten, and it costs no rent.
///
/// All four settle instructions emit this same type, deliberately: `mint`
/// carries the currency (`Pubkey::default()` for SOL) and `payer` is whoever
/// signed, wallet or session key. `amount` is the price the QR showed, `fee` is
/// the seller's deduction above the marker's rent — on a token invoice the whole
/// of it, on SOL the part the rent does not already take — and
/// `seller_proceeds` is what the seller actually keeps: the price less the fee,
/// less the receipt rent where the seller carries it (the SOL paths; a token
/// seller is charged no lamports). `fee` is not revenue: the settlement first
/// pays the sponsor back the network fee it spent, and the treasury keeps what
/// is left.
#[event]
pub struct PaymentSettled {
    /// The invoice price, in lamports or the mint's base units.
    pub amount: u64,
    /// The seller's deduction above the marker's rent: the network fee the sponsor
    /// is repaid plus the platform's margin.
    pub fee: u64,
    /// `amount - fee` on a token settle, `amount - fee - receipt rent` on SOL:
    /// the receipt rent is a cost the seller now carries, so the event states
    /// the number after it rather than making every reader know the rule.
    pub seller_proceeds: u64,
    /// Wallet or session key that settled.
    pub payer: Pubkey,
    pub seller: Pubkey,
    /// `Pubkey::default()` for a SOL invoice.
    pub mint: Pubkey,
    /// The invoice's lookup key, as carried in its seeds.
    pub reference: Pubkey,
    /// The invoice PDA this settlement closed, so an indexer can key the payment
    /// by the address the QR carries without re-deriving the seeds.
    pub invoice: Pubkey,
    /// The invoice note, which used to live in the receipt.
    pub memo: String,
}

/// The fee policy as the admin just wrote it, so an indexer prices a settlement
/// with the policy in force rather than today's. It carries the whole
/// `FeePolicy`, because a tiered fee needs both rates and both boundaries to be
/// reproduced.
#[event]
pub struct ConfigUpdated {
    pub admin: Pubkey,
    pub policy: FeePolicy,
    pub fee_mode: FeeMode,
    pub session_fee_float: u64,
}

/// A buyer opened or refilled an allowance. Same type for the SOL pot and a
/// token vault, with `mint` carrying the difference: `monthly_limit` is the
/// ceiling that now applies, and `session_key` is who may spend it.
#[event]
pub struct BudgetOpened {
    pub buyer: Pubkey,
    pub session_key: Pubkey,
    /// `Pubkey::default()` for the SOL budget.
    pub mint: Pubkey,
    pub monthly_limit: u64,
}
