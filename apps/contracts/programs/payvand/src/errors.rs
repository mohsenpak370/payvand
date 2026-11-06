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
    #[msg("The seller account does not match the payment record")]
    SellerMismatch,
    #[msg("The payment is not in the expected state")]
    NotPending,
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
}
