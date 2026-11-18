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

use crate::state::FeeMode;

/// One settled invoice — from the wallet path or an allowance, in SOL or a
/// token.
///
/// All four settle instructions emit this same type, deliberately: like
/// `PaymentReceipt`, `mint` carries the currency (`Pubkey::default()` for SOL)
/// and `payer` is whoever signed, wallet or session key. The split is the point
/// of the event: `amount` is the price the QR showed, `fee` went to the
/// treasury, and `seller_proceeds = amount - fee` is what the seller keeps —
/// rents are the pool's business, not the seller's.
#[event]
pub struct PaymentSettled {
    /// The invoice price, in lamports or the mint's base units.
    pub amount: u64,
    /// The seller-side fee that went to the treasury.
    pub fee: u64,
    /// `amount - fee`: what the seller keeps of the price.
    pub seller_proceeds: u64,
    /// Wallet or session key that settled.
    pub payer: Pubkey,
    pub seller: Pubkey,
    /// `Pubkey::default()` for a SOL invoice.
    pub mint: Pubkey,
    /// The invoice's lookup key, as carried in its seeds.
    pub reference: Pubkey,
}

/// The fee policy as the admin just wrote it, so an indexer prices a settlement
/// with the policy in force rather than today's.
#[event]
pub struct ConfigUpdated {
    pub admin: Pubkey,
    pub fee_bps: u16,
    pub fixed_fee_lamports: u64,
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
