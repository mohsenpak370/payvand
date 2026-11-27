//! Starter tokens for a new wallet, from the deployment's own treasury.
//!
//! Payvand runs on a test network, and a wallet with an empty balance cannot try
//! anything: every payment needs a little SOL for the network fee, and the
//! stablecoins have to be in the wallet before the checkout can spend them. The
//! public faucets work, but they are another site and another set of addresses to
//! copy, which is exactly where someone trying the app for the first time stops.
//!
//! So one claim per account, once: 1 SOL and 10 USDC and 10 EURC, sent by the
//! server's treasury key. Only on devnet — a mainnet treasury behind this endpoint
//! would be a way to drain real money — and only to the wallet the account has
//! linked, so the tokens land where the app will spend them.
//!
//! This file is the public surface. Implementation and unit tests live in the
//! `faucet/` directory:
//!
//! - `handler`: `GET` and `POST /api/faucet`
//! - `amounts`: which clusters hand out tokens, and how many
//! - `claims`: the once-per-account record in `faucet_claims`
//! - `send`: signing, sending and confirming the transfer
//!
//! The instructions themselves are built by `crate::spl`.

mod amounts;
mod claims;
mod handler;
mod send;

pub use handler::{claim, status, ClaimResponse, FaucetStatus};

pub(crate) use amounts::{hands_out, sol_lamports, token_base_units};
