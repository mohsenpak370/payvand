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
    #[msg("This config account is not the version this migration upgrades")]
    InvalidConfigVersion,
    #[msg("That SOL price is not a price the program will write")]
    InvalidFeePrice,
    #[msg("The flats were refreshed a moment ago")]
    FeePriceTooSoon,
    // Retired with the per-shop fee feature (plan 0005, cancelled 2026-11-27).
    // Kept, never returned, so removing them cannot shift `InvalidNetworkFee`
    // (6023) and any client matching that code keeps working.
    #[msg("Retired: per-shop fee cap")]
    ShopFeeTooHigh,
    #[msg("Retired: shop policy belongs to another seller")]
    ShopPolicySellerMismatch,
    #[msg("The network fee is larger than this program allows a policy to name")]
    InvalidNetworkFee,
}
