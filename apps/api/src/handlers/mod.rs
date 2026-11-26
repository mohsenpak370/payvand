//! Handlers that do not belong to a feature module.

mod meta;
mod payments;

pub(crate) use meta::{docs, health, index, openapi};
pub(crate) use payments::{create_payment, get_payment, watch_payments};
