//! One module per instruction family. Each exposes plain functions the
//! `#[program]` entrypoints in `lib.rs` delegate to, so the instruction list
//! stays readable in one place while each body lives next to its accounts.
//! Larger families — `config`, `sol` and `token` — keep those bodies in a
//! directory of submodules.

pub mod config;
pub mod sol;
pub mod token;
