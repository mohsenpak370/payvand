//! The shops subscription: €1.70 for 30 days, payable in SOL, USDC or EURC.
//!
//! The price is a euro price, so only EURC can be charged at par. The other two
//! currencies are converted at a live rate fetched from a public price API and
//! cached for [`RATE_TTL_SECS`]: a quote is the amount the seller sees, and the
//! invoice created from it is the amount that must settle, so a rate that moves
//! between the two cannot change what was agreed.
//!
//! The payment itself is an ordinary invoice from the platform account, settled
//! by the deployed program exactly like a seller's invoice. Nothing is trusted
//! from the client: [`confirm`] asks the cluster whether the invoice was paid and
//! only then records the period.
//!
//! This file is the public surface. Implementation and unit tests live in the
//! `subscription/` directory:
//!
//! - `handler`: `GET /api/subscription`, `POST /api/subscription/invoice` and
//!   `POST /api/subscription/confirm`
//! - `credit`: verifying a settled invoice and recording its period, including
//!   the repair of payments nobody confirmed
//! - `period`: the account's current period and the `require_active` gate
//! - `pricing`: the plan's price and its quote in each currency
//! - `feed`: the live exchange rate and its cache
//! - `platform`: the selling account and the cluster
//! - `model`: the request and response bodies

mod credit;
mod feed;
mod handler;
mod model;
mod period;
mod platform;
mod pricing;

pub use feed::{rates, RATE_TTL_SECS};
pub use handler::{confirm, create_invoice, status};
pub use model::{ConfirmRequest, InvoiceRequest, InvoiceResponse, SubscriptionStatus};
#[cfg(test)]
pub(crate) use period::activate_for_tests;
pub use period::require_active;
pub use platform::platform_seller;
pub use pricing::{quotes_for, Quote, Rates, CURRENCIES, PERIOD_SECS, PLAN_PRICE_EUR_CENTS};
