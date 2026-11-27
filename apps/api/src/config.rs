//! `GET /api/config` — every number a quote needs, in one cached answer.
//!
//! A checkout quotes a price from three things: the fee policy the program
//! settles with, the cluster's rent minimums for the accounts a payment opens,
//! and the mint addresses the currencies are priced in. All three live on chain,
//! and the browser used to read them itself — an account read and six rent
//! reads, from every tab, every minute, against the buyer's own RPC.
//!
//! This answers them from the API instead. The API reads the cluster once and
//! keeps the answer for a minute, so a hundred checkouts in that minute cost one
//! set of reads; the response also carries a minute of `Cache-Control`, so a CDN
//! in front of it costs none at all. A read that fails is never cached.
//!
//! This file is the public surface. Implementation and unit tests live in the
//! `config/` directory:
//!
//! - `model`: the response shape the client reads
//! - `layout`: the on-chain `Config` account and the account sizes
//! - `read`: one read of the policy and the rents from a cluster
//! - `cache`: the per-cluster minute an answer is kept for
//! - `handler`: the `GET /api/config` route itself

mod cache;
mod handler;
mod layout;
mod model;
mod read;

#[cfg(test)]
mod test_support;

pub use handler::config;
pub use model::{ConfigQuery, ConfigResponse, Fees, Mints, Rent, Spaces};
pub use read::read_config;
