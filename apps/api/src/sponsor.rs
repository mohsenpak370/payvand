//! The sponsored settle fee: a relayer's keypair pays an invoice's network fee
//! and its two record rents, so a wallet holding only the price can buy
//! something — in USDC, EURC, or SOL.
//!
//! The relayer never signs a transaction a caller chose. `POST
//! /api/sponsor/settle` rebuilds the one settle message it expects from the
//! request's parameters — `settle_token_payment` with the invoice, the receipt,
//! the three token accounts, `Config` and the treasury, or `settle_payment` with
//! the invoice, the receipt, `Config` and the treasury when the request names no
//! mint — plus the sponsor's own account. Only if that message asks the chain for
//! the same thing the buyer signed does it add its fee-payer signature and
//! submit. A different instruction, account, amount or mint is a mismatch, and a
//! mismatch is refused before anything is signed.
//!
//! The comparison is over meaning, not bytes: the client's compiler and this
//! one order a version 0 message's accounts differently, and the wallet is a
//! second compiler — Phantom adds its own compute-unit limit and priority price
//! at signing time. Those two are tolerated because they name no account and set
//! no state; the priority fee they cost is the sponsor's money, so the endpoint
//! prices it (`SPONSOR_MAX_PRIORITY_FEE_LAMPORTS`, 100,000 by default) and
//! refuses anything above that.
//!
//! The program routes the treasury's refund of both rents back to the sponsor
//! (`plans/0004-relayer-sponsored-fees.md`), so a sponsored settlement is not a
//! lamport faucet for the buyer: the relayer's net cost is the network fee.
//! No session is needed: a buyer who scanned a printed code has no account, and
//! the buyer's own signature is what authorises the payment. Spend is bounded
//! here, not by the client: a rolling 24-hour cap per wallet and one for the
//! deployment, both in libSQL.
//!
//! This file is the public surface. Implementation and unit tests live in the
//! `sponsor/` directory:
//!
//! - `handler`: `GET /api/sponsor` and `POST /api/sponsor/settle`
//! - `params`: validating the settle a request names
//! - `message`: rebuilding the one settle message the relayer will sign
//! - `diagnostics`: the fields a refusal logs
//! - `store`: the sponsored settlements the daily caps count
//! - `relayer`: the keypair, program id and caps from the environment
//! - `model`: the request and response bodies

mod diagnostics;
mod handler;
mod message;
mod model;
mod params;
mod relayer;
mod store;

pub use handler::{info, settle};
pub use model::{InfoQuery, InfoResponse, SettleRequest, SettleResponse};
pub use relayer::SPONSORED_LAMPORTS;
