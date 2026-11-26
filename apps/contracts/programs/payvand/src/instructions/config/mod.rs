//! The fee policy account and the treasury the fee lands in.
//!
//! `Config` is a singleton created once by the program's upgrade authority, so
//! the admin it records is never a caller's choice. Settlements only read it,
//! and the treasury is addressed by seed, so no settlement can name its own fee
//! recipient.
//!
//! Each instruction family lives in its own submodule; all of their public
//! items are re-exported here, so `lib.rs` and the IDL keep the same paths.

pub mod fee_price;
pub mod migrate;
pub mod policy;
pub mod treasury;

pub use fee_price::*;
pub use migrate::*;
pub use policy::*;
pub use treasury::*;
