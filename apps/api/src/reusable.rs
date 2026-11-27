//! Reusable payment codes: one offer, paid by anyone, as often as they like.
//!
//! A one-time invoice cannot do this. Settlement closes its account, so one
//! address can only ever be paid once. A reusable code is therefore not an
//! invoice but an *offer* the seller stored — a price, a currency, a title and
//! the words around it — and every visit mints a fresh invoice from it: a new
//! reference, therefore a new invoice address, with exactly the same terms.
//! Each payer pays their own invoice, and each one lands in the seller's history
//! like any other payment.
//!
//! Minting is unauthenticated on purpose: the buyer scanning a printed code has
//! no account here. The terms come from the seller's stored row, never from the
//! request, so nothing a visitor sends can change what the invoice charges.
//!
//! This file is the public surface. Implementation and unit tests live in the
//! `reusable/` directory:
//!
//! - `handler`: `POST /api/reusable` and `POST /api/reusable/{id}/invoice`
//! - `links`: storing an offer and minting a visitor's invoice from it
//! - `offer`: the terms a code charges and the invoice they describe
//! - `model`: the request and response shapes

mod handler;
mod links;
mod model;
mod offer;

pub(crate) use handler::{create, invoice};
pub use links::{create_link, mint_invoice};
pub use model::{CreateReusableRequest, ReusableLink, LINK_PATH_PREFIX};
