//! One module per currency family. Both expose plain functions the
//! `#[program]` entrypoints in `lib.rs` delegate to, so the instruction list
//! stays readable in one place while each body lives next to its accounts.

pub mod sol;
pub mod token;
