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

/// The permanent record of a settled invoice — SOL or token.
///
/// The temporary `Payment`/`TokenPayment` account is created, settled and
/// closed inside one transaction; this receipt is what survives. Its address is
/// derived from the invoice address (`["payment-receipt", invoice]`), and
/// `settled_at` doubles as the replay guard: a second settlement of the same
/// invoice finds a receipt with `settled_at != 0`.
///
/// One type for both currencies, with the mint carrying the difference
/// (`Pubkey::default()` for SOL): a watcher holding only an invoice address can
/// then find its receipt with one derivation instead of guessing whether the
/// invoice was priced in SOL or a token.
#[account]
#[derive(Debug)]
pub struct PaymentReceipt {
    /// The invoice PDA this receipt belongs to, so a program-wide scan can name
    /// the invoice without re-deriving it.
    pub invoice: Pubkey,
    pub seller: Pubkey,
    pub reference: Pubkey,
    /// `Pubkey::default()` for a SOL invoice.
    pub mint: Pubkey,
    pub amount: u64,
    pub memo: String,
    /// Wallet or session key that signed the settlement.
    pub payer: Pubkey,
    pub created_at: i64,
    pub settled_at: i64,
    /// Equal to `settled_at`: the invoice account closes in the settle
    /// transaction, so the record is complete the moment it is written.
    pub closed_at: i64,
    pub bump: u8,
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

/// The program's singleton fee policy and treasury pointers.
///
/// Created once per deployment by the program's upgrade authority, which
/// becomes `admin`. Settlements read it and never write it; only the admin can
/// change the policy, and the fee destination is derived from a seed here
/// rather than named by a caller, so no settlement can redirect it.
#[account]
#[derive(Debug)]
pub struct Config {
    /// The upgrade authority that initialized the config. It may change the
    /// policy and withdraw treasury surplus, nothing else.
    pub admin: Pubkey,
    /// Seller-side fee in basis points (200 = 2%).
    pub fee_bps: u16,
    /// Flat lamport fee added on top of the proportional one.
    pub fixed_fee_lamports: u64,
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
