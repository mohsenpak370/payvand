//! Payvand's on-chain rules: invoices that can only be settled at the exact
//! price and currency they were issued for, and monthly allowances a buyer's
//! session key can spend without waking their wallet.
//!
//! The program itself lives here as a list of entrypoints; each one delegates
//! to a module that holds its logic and its account context:
//!
//! - `state` — the accounts the program owns
//! - `constants` — seeds, account sizes, lamport floats
//! - `errors` — every message a wallet can be shown
//! - `helpers` — the calendar month, and token invoice bookkeeping
//! - `instructions::sol` — SOL invoices and the SOL allowance
//! - `instructions::token` — SPL token invoices and per-mint allowances

use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod helpers;
pub mod instructions;
pub mod state;

pub use constants::*;
pub use errors::PaymentError;
pub use state::*;

// The account contexts and the `__client_accounts_*` modules Anchor generates
// beside them have to be reachable from the crate root, where `#[program]`
// looks them up.
pub use instructions::sol::*;
pub use instructions::token::*;

declare_id!("4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J");

#[program]
pub mod payvand {
    use super::*;

    // ---- SOL invoices -----------------------------------------------------

    /// Buyer pays an invoice. The invoice account is created (and funded by
    /// the buyer) inside this same transaction, so the seller never has to
    /// sign anything to open a payment code — the QR is just an address plus
    /// its details.
    ///
    /// `amount` is part of the PDA seeds, which is what keeps this safe: an
    /// invoice can only ever be settled at the exact amount that was encoded
    /// into the QR code, because any other amount derives a different account
    /// address than the one the seller is watching.
    pub fn settle_payment(ctx: Context<SettlePayment>, amount: u64, memo: String) -> Result<()> {
        instructions::sol::settle_payment_handler(ctx, amount, memo)
    }

    /// Seller reclaims the rent lamports locked in a settled invoice. This is
    /// optional housekeeping — it is the one signature a seller may ever make,
    /// and only when they want the rent back.
    pub fn close_payment(ctx: Context<ClosePayment>, amount: u64) -> Result<()> {
        instructions::sol::close_payment_handler(ctx, amount)
    }

    // ---- SOL allowance ----------------------------------------------------

    /// Buyer funds this month's spending budget and registers a session key.
    /// This is the ONE wallet signature the whole feature needs: afterwards
    /// the session key — an app-generated keypair stored on the device — can
    /// settle invoices from the pot without touching the wallet again.
    pub fn open_budget(ctx: Context<OpenBudget>, monthly_limit: u64) -> Result<()> {
        instructions::sol::open_budget_handler(ctx, monthly_limit)
    }

    /// Silent payment: the session key settles an invoice straight out of the
    /// buyer's budget — no wallet popup, because the only signature here is
    /// the local session key's. The pot pays the seller and refunds the
    /// invoice rent it just advanced from the session key's float.
    ///
    /// The monthly cap and the pot balance are both enforced here, and the
    /// amount still rides in the payment PDA's seeds, so a QR code can only
    /// ever be settled for exactly the price it shows — the same guarantee
    /// `settle_payment` gives.
    pub fn pay_from_budget(ctx: Context<PayFromBudget>, amount: u64, memo: String) -> Result<()> {
        instructions::sol::pay_from_budget_handler(ctx, amount, memo)
    }

    /// Reclaim whatever is left in the budget and retire the session key.
    /// One wallet signature — the buyer gets every lamport back.
    pub fn close_budget(ctx: Context<CloseBudget>) -> Result<()> {
        instructions::sol::close_budget_handler(ctx)
    }

    // ---- token invoices ---------------------------------------------------

    /// Buyer pays a token-denominated invoice straight from their wallet.
    ///
    /// Same guarantee as `settle_payment`: the amount rides in the PDA seeds,
    /// so an invoice can only ever be settled for the exact price its QR
    /// shows. The mint is in the seeds too, so a USDC invoice and a SOL
    /// invoice that happen to share a reference and a price are different
    /// accounts and cannot pay for each other.
    pub fn settle_token_payment(
        ctx: Context<SettleTokenPayment>,
        amount: u64,
        memo: String,
    ) -> Result<()> {
        instructions::token::settle_token_payment_handler(ctx, amount, memo)
    }

    /// Seller reclaims the rent of a settled token invoice. Mirrors
    /// `close_payment`, and is the only signature a seller ever makes.
    pub fn close_token_payment(ctx: Context<CloseTokenPayment>, amount: u64) -> Result<()> {
        instructions::token::close_token_payment_handler(ctx, amount)
    }

    // ---- token allowance --------------------------------------------------

    /// Buyer opens (or tops up) this month's budget for one token and
    /// registers a session key for it. One wallet signature per token; after
    /// that the session key settles token invoices from the vault without
    /// touching the wallet again.
    pub fn open_token_budget(ctx: Context<OpenTokenBudget>, monthly_limit: u64) -> Result<()> {
        instructions::token::open_token_budget_handler(ctx, monthly_limit)
    }

    /// Silent token payment: the session key settles a token invoice out of
    /// the buyer's vault, with no wallet popup. The monthly cap, the vault
    /// balance and the amount in the invoice seeds are all enforced here, and
    /// only the session key registered at open time may spend.
    pub fn pay_from_token_budget(
        ctx: Context<PayFromTokenBudget>,
        amount: u64,
        memo: String,
    ) -> Result<()> {
        instructions::token::pay_from_token_budget_handler(ctx, amount, memo)
    }

    /// Drain the token vault back to the buyer and retire the budget. One
    /// wallet signature; every token and lamport comes back.
    pub fn close_token_budget(ctx: Context<CloseTokenBudget>) -> Result<()> {
        instructions::token::close_token_budget_handler(ctx)
    }
}
