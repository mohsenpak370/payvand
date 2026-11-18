//! Every failure the program reports, with the message wallets show.

use anchor_lang::prelude::*;

#[error_code]
pub enum PaymentError {
    #[msg("Payment amount must be greater than zero")]
    InvalidAmount,
    #[msg("Memo exceeds the maximum length")]
    MemoTooLong,
    #[msg("This payment has already been settled")]
    AlreadySettled,
    #[msg("This amount cannot cover the invoice account's rent")]
    InvoiceTooSmall,
    #[msg("The seller address is not valid")]
    InvalidSeller,
    #[msg("The monthly spending limit has been reached")]
    BudgetExhausted,
    #[msg("The budget does not hold enough SOL for this payment")]
    InsufficientBudget,
    #[msg("The session key is not the one registered for this budget")]
    InvalidSessionKey,
    #[msg("The token does not match this budget")]
    MintMismatch,
    #[msg("This deployment has no fee config yet")]
    ConfigMissing,
    #[msg("Only the program's upgrade authority may initialize the config")]
    InvalidConfigAdmin,
    #[msg("This program is not upgradeable, so it cannot bootstrap a config")]
    ProgramNotUpgradeable,
    #[msg("The fee rate cannot exceed 100%")]
    InvalidFeeBps,
    #[msg("The session float must cover one payment's rents")]
    InvalidSessionFloat,
    #[msg("The treasury cannot cover the refund it owes")]
    TreasuryUnderfunded,
    #[msg("This fee mode is not supported yet")]
    FeeModeUnsupported,
    #[msg("The fee calculation overflowed")]
    FeeOverflow,
    #[msg("Payvand settles only SPL Token mints; Token-2022 is not supported")]
    Token2022Unsupported,
}
