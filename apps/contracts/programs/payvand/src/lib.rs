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
//! - `events` — the structured events emitted beside the `msg!` logs
//! - `helpers` — the calendar month, and token invoice bookkeeping
//! - `instructions::sol` — SOL invoices and the SOL allowance
//! - `instructions::token` — SPL token invoices and per-mint allowances

use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod events;
pub mod helpers;
pub mod instructions;
pub mod state;

pub use constants::*;
pub use errors::PaymentError;
pub use events::*;
pub use state::*;

// The account contexts and the `__client_accounts_*` modules Anchor generates
// beside them have to be reachable from the crate root, where `#[program]`
// looks them up.
pub use instructions::config::*;
pub use instructions::shop::*;
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
        policy: FeePolicy,
        fee_mode: FeeMode,
        session_fee_float: u64,
    ) -> Result<()> {
        instructions::config::initialize_config_handler(ctx, policy, fee_mode, session_fee_float)
    }

    /// Changes the fee policy. Admin only; it never rewrites `admin` itself.
    pub fn update_config(
        ctx: Context<UpdateConfig>,
        policy: FeePolicy,
        fee_mode: FeeMode,
        session_fee_float: u64,
    ) -> Result<()> {
        instructions::config::update_config_handler(ctx, policy, fee_mode, session_fee_float)
    }

    /// Re-pegs the two token flats — the marker's rent expressed in USDC and in
    /// EURC — to a live SOL price. Permissionless and bounded: a plausible price
    /// only, one within `PRICE_ANCHOR_BAND_BPS` of the admin's anchor, and one
    /// call a minute. The band is measured from the anchor rather than the flat
    /// it rewrites, so repeated calls cannot walk the flats anywhere.
    pub fn refresh_fee_price(
        ctx: Context<RefreshFeePrice>,
        sol_usd_micros: u64,
        sol_eur_micros: u64,
    ) -> Result<()> {
        instructions::config::refresh_fee_price_handler(ctx, sol_usd_micros, sol_eur_micros)
    }

    /// Writes the SOL price the permissionless refresh is measured against —
    /// `["fee-anchor"]`, created on the first call — and prices both token flats
    /// at it from the cluster's current rent. Admin only: it is the one price a
    /// permissionless caller cannot choose.
    pub fn set_fee_price_anchor(
        ctx: Context<SetFeePriceAnchor>,
        sol_usd_micros: u64,
        sol_eur_micros: u64,
    ) -> Result<()> {
        instructions::config::set_fee_price_anchor_handler(ctx, sol_usd_micros, sol_eur_micros)
    }

    /// Upgrades the live config account — version 1 or 2 — to version 3 in place. Admin
    /// only, and only once: the account must be one of the two older layouts and
    /// must record the signer as its admin. It carries the admin, fee mode and
    /// session float over and installs the `FeePolicy` passed here, except both
    /// token flats, which it zeroes until `set_fee_price_anchor` prices them.
    pub fn migrate_config(ctx: Context<MigrateConfig>, policy: FeePolicy) -> Result<()> {
        instructions::config::migrate_config_handler(ctx, policy)
    }

    /// Sets a shop's multiplier on the global fee policy — its own rate, in
    /// basis points on `Config`'s two rates, `10_000` being `Config` unchanged.
    ///
    /// Signed by the seller and seeded by the seller plus the key the API keeps
    /// beside its `shops` row, so a shop is priced on chain by the wallet that
    /// owns it and two shops of one wallet can charge differently. The cap is
    /// enforced here rather than at settle time: a shop may discount itself and
    /// never surcharge a buyer, and no settlement has an out-of-band rate to
    /// handle. The account is optional on every settle path — an invoice that
    /// names none pays `Config`'s rate, which is what keeps every QR printed
    /// before this landed working.
    pub fn set_shop_policy(ctx: Context<SetShopPolicy>, multiplier_bps: u16) -> Result<()> {
        instructions::shop::set_shop_policy_handler(ctx, multiplier_bps)
    }

    /// Moves treasury surplus to the admin. It always leaves the account's
    /// rent-exempt minimum plus `TREASURY_RESERVE_SETTLEMENTS` settlements' worth
    /// of refunds, so a withdrawal cannot be what stops the next token payment.
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
    ///
    /// The buyer signs for its own token account, and a `sponsor` pays for the
    /// settlement's two record rents and receives the treasury's refund of
    /// them. A funded wallet passes itself as the sponsor — one signature, no
    /// change in cost — and a wallet holding only the token passes a relayer's
    /// key, which turns a zero-lamport buyer into a payer without being a
    /// lamport faucet for it.
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
    ///
    /// The price pays everything: the seller receives it less the fee, the fee
    /// lands in the treasury's own account for the mint, and the pool refunds the
    /// two rents the session key advanced and takes the closed invoice's rent
    /// back. The vault therefore falls by exactly the price, and the seller
    /// receives no lamports at all.
    pub fn pay_from_token_budget(
        ctx: Context<PayFromTokenBudget>,
        amount: u64,
        memo: String,
    ) -> Result<()> {
        instructions::token::pay_from_token_budget_handler(ctx, amount, memo)
    }

    /// Drain the token vault back to the buyer and retire the budget. One
    /// wallet signature: the tokens come back, and so do the vault's rent and
    /// the budget account's rent. The session key's float is swept by the
    /// client, as in `close_budget`.
    pub fn close_token_budget(ctx: Context<CloseTokenBudget>) -> Result<()> {
        instructions::token::close_token_budget_handler(ctx)
    }
}
