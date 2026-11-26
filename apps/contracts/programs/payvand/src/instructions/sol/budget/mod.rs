//! The SOL monthly allowance: one wallet signature to open it, then silent
//! payments from a session key until the buyer closes it.
//!
//! The two halves have different signers and different jobs: `lifecycle` is the
//! wallet funding or retiring the pot, `spend` is the session key settling an
//! invoice out of it.

pub mod lifecycle;
pub mod spend;

pub use lifecycle::*;
pub use spend::*;
