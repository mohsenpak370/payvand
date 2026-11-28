//! The relayer: Payvand's own key pays every wallet settlement's network fee and
//! fronts its two record rents, so a buyer pays exactly the price — in USDC,
//! EURC or SOL — and needs no SOL to do it. The program refunds the relayer in
//! the same transaction and charges the seller for the permanent costs; the
//! whole money flow, and the rules that must not change, are in
//! `docs/payment-costs.md`.
//!
//! The relayer signs first and never signs a transaction a caller chose.
//! `POST /api/sponsor/prepare` builds the one settle message the invoice's
//! fields describe — `settle_token_payment` with the invoice, the receipt, the
//! three token accounts, `Config` and the treasury, or `settle_payment` when the
//! request names no mint — behind a compute-unit limit, signs it as fee payer and
//! returns it. The wallet then signs a transaction that is complete except for
//! its own signature, which is what lets it simulate and preview the payment.
//! `POST /api/sponsor/settle` sends it only if its message is byte for byte the
//! one the relayer signed and both signatures verify.
//!
//! A Solana Pay wallet (a code scanned outside Payvand) gets the same relayer-
//! signed settle from `solana_pay_settle` and sends it itself.
//!
//! No session is needed: a buyer who scanned a printed code has no account, and
//! the buyer's own signature is what authorises the payment. The relayer's
//! risk is bounded here, not by the client: every settle is dry-run before it is
//! signed, and the limits count only settles that failed on chain — never a
//! payment that worked (see `store` and `docs/payment-costs.md`).
//!
//! This file is the public surface. Implementation and unit tests live in the
//! `sponsor/` directory:
//!
//! - `handler`: `GET /api/sponsor`, `POST /api/sponsor/prepare` and
//!   `POST /api/sponsor/settle`, and the Solana Pay settle
//! - `params`: validating the settle a request names
//! - `message`: building the one settle message the relayer will sign
//! - `diagnostics`: the fields a refusal logs
//! - `store`: the signed settles, their outcomes, and what the limits count
//! - `relayer`: the keypair and program id from the environment
//! - `model`: the request and response bodies

mod diagnostics;
mod handler;
mod message;
mod model;
mod params;
mod relayer;
mod store;

pub use handler::{
    info, prepare, self_paid_solana_pay_settle, settle, solana_pay_settle, SolanaPaySettle,
};
pub use model::{
    InfoQuery, InfoResponse, PrepareRequest, PrepareResponse, SettleFields, SettleRequest,
    SettleResponse,
};
pub use relayer::SPONSORED_LAMPORTS;
