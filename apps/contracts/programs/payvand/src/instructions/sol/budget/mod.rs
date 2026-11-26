//! The SOL monthly allowance: one wallet signature to open it, then silent
//! payments from a session key until the buyer closes it.
//!
//! `open` and `close` are the wallet funding or retiring the pot; `spend` is the
//! session key settling an invoice out of it.

pub mod close;
pub mod open;
pub mod spend;

pub use close::*;
pub use open::*;
pub use spend::*;
