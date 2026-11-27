//! Discount codes.
//!
//! A seller defines percentage-off codes on their account; a buyer types one
//! into the scan-to-pay screen and the API prices the invoice. The discount
//! never changes the amount already committed by the QR code — it derives a
//! *second* payment address from the same seller and reference at the lower
//! amount, and the budget/silent path pays that address exactly like the full
//! price. Codes are stored per account, not per invoice, so one list works
//! for every QR the seller creates.
//!
//! This file is the public surface. Implementation and unit tests live in the
//! `discounts/` directory:
//!
//! - `codes`: a seller's own list — list, create and delete
//! - `checkout`: pricing an invoice with a code and recording its use
//! - `store`: the database lookups both of those share
//! - `rules`: code format, percentage range and the discounted price
//! - `model`: the request and response shapes

mod checkout;
mod codes;
mod model;
mod rules;
mod store;

pub use checkout::{use_code, validate};
pub use codes::{create, delete, list};
pub use model::{
    CreateDiscountRequest, DiscountCode, DiscountPreview, UseDiscountRequest,
    ValidateDiscountRequest,
};
pub use rules::{discounted_lamports, normalize_code};
pub use store::{percent_for, resolve_for_seller};
