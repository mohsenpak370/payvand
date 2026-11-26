//! The per-mint monthly allowance: one wallet signature to open it, then silent
//! payments from a session key until the buyer closes it.
//!
//! `open` and `close` are the wallet funding or retiring the vault; `spend` is
//! the session key settling an invoice out of it, priced and paid out exactly as
//! `settle_token_payment` is.

mod close;
mod open;
mod spend;

pub use close::*;
pub use open::*;
pub use spend::*;
