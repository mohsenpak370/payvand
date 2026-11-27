//! Wallet linking and wallet sign-in.
//!
//! A wallet proves it belongs to an account by signing a server-issued
//! message: `POST /api/auth/wallet/challenge` stores the exact text (one
//! live challenge per address, five minute expiry), the wallet signs it,
//! and the follow-up call verifies the signature against the address,
//! checks the purpose, and consumes the row — so a signature can never be
//! replayed, nor reused for a different request.
//!
//! `GET /api/me/transactions` lists the invoices that involve the linked wallet
//! — as seller, as payer, or as the buyer behind a budget's session key. It lists
//! them from the rows this API wrote when it made each invoice, asks the cluster
//! only whether they settled, and falls back to the program scan for anything it
//! did not record.
//!
//! This file is the public surface. Implementation and tests live in the
//! `wallet/` directory:
//!
//! - `challenge`: issuing, checking and consuming the signed challenge
//! - `account`: linking, signing in with and unlinking a wallet
//! - `transactions`: `GET /api/me/transactions`
//! - `matching`: which payments are the wallet's, and in which direction
//! - `meta`: the names and receipt lines the chain does not keep
//! - `model`: the request and response bodies

mod account;
mod challenge;
mod matching;
mod meta;
mod model;
mod transactions;

#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod tests;

pub use account::{link, signin, unlink};
pub use challenge::{challenge, CHALLENGE_TTL_SECS};
pub use model::{
    ChallengePurpose, ChallengeRequest, ChallengeResponse, FeedQuery, MyTransaction,
    TransactionsResponse, WalletSignatureRequest,
};
pub use transactions::transactions;
