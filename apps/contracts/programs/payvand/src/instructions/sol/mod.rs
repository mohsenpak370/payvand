//! SOL invoices and the SOL monthly allowance.
//!
//! Settlement is single-use and leaves a permanent marker: the invoice account
//! is created, settled and closed inside the settle transaction, and a receipt
//! keyed by the invoice address survives it. The marker's `settled_at` is the
//! replay guard, which is why the invoice account can be `init` rather than
//! `init_if_needed`. The payment's own facts are in the `PaymentSettled` event
//! the same transaction logs.
//!
//! Each instruction family lives in its own submodule; all of their public
//! items are re-exported here, so `lib.rs` and the IDL keep the same paths.

pub mod budget;
pub mod settle;

pub use budget::*;
pub use settle::*;
