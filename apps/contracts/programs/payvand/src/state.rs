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
