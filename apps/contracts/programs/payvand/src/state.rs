//! The accounts the program owns, and the invoice state they hold.

use anchor_lang::prelude::*;

/// The buyer's monthly spending pot: lamports held by the program plus the
/// rules that make the session key's signatures safe. Seeded by the buyer's
/// wallet, so there is exactly one per buyer.
#[account]
pub struct Budget {
    /// Main wallet — funds, refills and closes the pot. The only wallet
    /// that ever signs for this feature.
    pub buyer: Pubkey,
    /// App-generated key allowed to spend from the pot without the wallet.
    pub session_key: Pubkey,
    /// Ceiling for one calendar month, in lamports.
    pub monthly_limit: u64,
    /// Spent so far in `month_index`.
    pub spent: u64,
    /// `year * 12 + month` that `spent` belongs to; a new month resets it.
    pub month_index: u64,
    pub bump: u8,
}

#[account]
#[derive(Debug)]
pub struct Payment {
    pub seller: Pubkey,
    /// Free-form lookup key chosen when the invoice was opened.
    pub reference: Pubkey,
    /// Price in lamports.
    pub amount: u64,
    /// Human readable invoice note.
    pub memo: String,
    pub state: PaymentState,
    /// Set once the invoice has been settled.
    pub payer: Pubkey,
    pub created_at: i64,
    pub settled_at: i64,
    pub bump: u8,
}

/// An invoice paid in an SPL token. A separate account type from `Payment`
/// because it records the mint, and keeping it separate leaves every SOL
/// invoice already on chain readable.
#[account]
#[derive(Debug)]
pub struct TokenPayment {
    pub seller: Pubkey,
    /// Free-form lookup key chosen when the invoice was opened.
    pub reference: Pubkey,
    /// The token this invoice is priced in.
    pub mint: Pubkey,
    /// Price in the mint's base units.
    pub amount: u64,
    /// Human readable invoice note.
    pub memo: String,
    pub state: PaymentState,
    /// Set once the invoice has been settled.
    pub payer: Pubkey,
    pub created_at: i64,
    pub settled_at: i64,
    pub bump: u8,
}

/// The permanent marker of a settled invoice — SOL or token.
///
/// The temporary `Payment`/`TokenPayment` account is created, settled and
/// closed inside one transaction; this marker is what survives, and it is
/// deliberately not a copy of the payment. Everything the payment was — the
/// split, the payer, the memo, the reference and the mint — is in the
/// `PaymentSettled` event that the same transaction logs, and the price and
/// memo are in the instruction data that produced it. Storing that again here
/// would cost the seller ~2.3 times the rent for a second copy of facts the
/// cluster already keeps.
///
/// What cannot move off chain is what only the runtime can answer:
///
/// - `settled_at` is the replay guard. A second settlement of the same invoice
///   finds this account already written and is refused with `AlreadySettled`,
///   atomically with the transfer rather than as a check made before it.
/// - `invoice` lets a program-wide scan (`get_program_accounts`) name the
///   invoice a marker belongs to. A PDA cannot be inverted, so without this a
///   scan sees an address and no way to say which payment it belongs to.
///
/// Its address is derived from the invoice address
/// (`["payment-receipt", invoice]`), so a watcher holding only the QR still
/// finds it with one derivation.
#[account]
#[derive(Debug)]
pub struct PaymentReceipt {
    /// The invoice PDA this marker belongs to, so a program-wide scan can name
    /// the invoice without a history read.
    pub invoice: Pubkey,
    /// Equal to the settle transaction's timestamp. Non-zero means this invoice
    /// is done; zero is what `init_if_needed` leaves in a fresh account.
    pub settled_at: i64,
}

/// A buyer's monthly spending pot for one token: the vault holds the tokens
/// and this account holds the rules the session key's signatures obey.
#[account]
#[derive(Debug)]
pub struct TokenBudget {
    /// Main wallet — funds, refills and closes the pot.
    pub buyer: Pubkey,
    /// App-generated key allowed to spend from the vault without the wallet.
    pub session_key: Pubkey,
    /// The token this budget is denominated in.
    pub mint: Pubkey,
    /// Ceiling for one calendar month, in the mint's base units.
    pub monthly_limit: u64,
    /// Spent so far in `month_index`.
    pub spent: u64,
    /// `year * 12 + month` that `spent` belongs to.
    pub month_index: u64,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaymentState {
    Pending,
    Settled,
}

/// The fee policy a `Config` carries: two marginal rates, the boundary they meet
/// at in each currency, and the flat each token currency pays for the marker's
/// rent.
///
/// The rate is marginal, not a cliff. An amount at or below the boundary pays
/// `fee_bps_below` on all of it; an amount above it pays `fee_bps_below` on the
/// part at or below the boundary plus `fee_bps_above` on the part above. A 0.1
/// SOL invoice and one lamport more therefore owe fees a lamport's worth of rate
/// apart, not twice as far.
///
/// The two flat fields exist because a token settlement cannot charge its
/// non-signing seller lamports, so the treasury — not the seller — funds the
/// permanent marker's 1,224,960-lamport rent on that path. Charging the seller
/// that same cost in the currency they are paid in is what keeps the treasury
/// whole. `usdc_mint` and `eurc_mint` select them: the program has no oracle, so
/// the flat is stated per currency by the admin rather than converted at settle
/// time. A mint that names neither pays no flat.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FeePolicy {
    /// Basis points on the part of the amount at or below the boundary.
    pub fee_bps_below: u16,
    /// Basis points on the part above it.
    pub fee_bps_above: u16,
    /// The SOL boundary, in lamports (100_000_000 = 0.1 SOL).
    pub sol_tier_lamports: u64,
    /// The token boundary, in base units (10_000_000 = 10 USDC at 6 decimals).
    pub token_tier_base_units: u64,
    /// Flat charged on a USDC invoice, in base units: the marker's rent at the
    /// SOL price the admin priced it at (1,224,960 lamports ≈ 89,400 base units
    /// at $73).
    pub flat_usdc_base_units: u64,
    /// Flat charged on a EURC invoice, in base units.
    pub flat_eurc_base_units: u64,
    /// The mint `flat_usdc_base_units` applies to.
    pub usdc_mint: Pubkey,
    /// The mint `flat_eurc_base_units` applies to.
    pub eurc_mint: Pubkey,
}

/// The program's singleton fee policy and treasury pointers.
///
/// Created once per deployment by the program's upgrade authority, which
/// becomes `admin`. Settlements read it and never write it; only the admin can
/// change the policy, and the fee destination is derived from a seed here
/// rather than named by a caller, so no settlement can redirect it.
///
/// Version 2 carries the tiered `FeePolicy`; version 1 was one rate plus a flat
/// lamport fee, and `migrate_config` reallocates the account it left behind.
#[account]
#[derive(Debug)]
pub struct Config {
    /// The upgrade authority that initialized the config. It may change the
    /// policy and withdraw treasury surplus, nothing else.
    pub admin: Pubkey,
    /// The two rates, their boundaries and the per-currency flats.
    pub policy: FeePolicy,
    /// Who pays the fee.
    pub fee_mode: FeeMode,
    /// Lamports parked on a session key so its silent transactions can pay
    /// their own network fees. Replaces the old hardcoded float.
    pub session_fee_float: u64,
    pub bump: u8,
    pub treasury_bump: u8,
    pub version: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeeMode {
    /// The seller receives `amount - fee`; the payer pays the price only.
    SellerPays,
    /// The payer pays `amount + fee` and the seller receives `amount`.
    BuyerPays,
}
