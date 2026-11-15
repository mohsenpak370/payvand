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
pub use instructions::config::*;
pub use instructions::sol::*;
pub use instructions::token::*;

declare_id!("4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J");

#[program]
pub mod payvand {
    use super::*;

    /// Creates the singleton fee policy. Callable once, by the program's
    /// upgrade authority, which becomes the admin — so no caller can install
    /// an admin of its choosing.
    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        fee_bps: u16,
        fixed_fee_lamports: u64,
        fee_mode: FeeMode,
        session_fee_float: u64,
    ) -> Result<()> {
        instructions::config::initialize_config_handler(
            ctx,
            fee_bps,
            fixed_fee_lamports,
            fee_mode,
            session_fee_float,
        )
    }

    /// Changes the fee policy. Admin only; it never rewrites `admin` itself.
    pub fn update_config(
        ctx: Context<UpdateConfig>,
        fee_bps: u16,
        fixed_fee_lamports: u64,
        fee_mode: FeeMode,
        session_fee_float: u64,
    ) -> Result<()> {
        instructions::config::update_config_handler(
            ctx,
            fee_bps,
            fixed_fee_lamports,
            fee_mode,
            session_fee_float,
        )
    }

    /// Moves treasury surplus to the admin, leaving its rent-exempt minimum.
    /// No payment path can touch the treasury this way.
    pub fn withdraw_treasury(ctx: Context<WithdrawTreasury>, amount: u64) -> Result<()> {
        instructions::config::withdraw_treasury_handler(ctx, amount)
    }

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

    /// Reclaim whatever is left in the budget and retire it. One wallet
    /// signature: the pot and the account's rent go back to the buyer, less the
    /// network fee. The session key's fee float is not this account's money — a
    /// PDA cannot move lamports off a plain keypair — so the client sweeps it
    /// from the device that holds the key.
    pub fn close_budget(ctx: Context<CloseBudget>) -> Result<()> {
        instructions::sol::close_budget_handler(ctx)
    }

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

    /// Opens the treasury's own token account for a mint, paid by the admin.
    ///
    /// A mint the treasury has no account for cannot be settled: the fee has
    /// nowhere to go, and the buyer would only find out after signing. Run this
    /// once for every mint the app can price in, before its first invoice.
    pub fn open_treasury_token_account(ctx: Context<OpenTreasuryTokenAccount>) -> Result<()> {
        instructions::token::open_treasury_token_account_handler(ctx)
    }

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
    /// wallet signature: the tokens come back, and so do the vault's rent, the
    /// budget account's rent and the lamports it kept to refund invoice rents.
    /// The session key's float is swept by the client, as in `close_budget`.
    pub fn close_token_budget(ctx: Context<CloseTokenBudget>) -> Result<()> {
        instructions::token::close_token_budget_handler(ctx)
    }
}
