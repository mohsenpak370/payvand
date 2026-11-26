//! Token invoices and the per-mint monthly allowance.
//!
//! Settlement mirrors the SOL paths: the invoice account is created, settled
//! and closed in the settle transaction, a receipt keyed by the invoice address
//! keeps the record, and the receipt's `settled_at` is the replay guard.
//!
//! The contexts in these submodules use `anchor_spl`'s *interface* account types for the token
//! accounts and the token program: those parse an account owned by either token
//! program, which is what makes the mint's owner check the rejection a
//! Token-2022 caller sees. The policy is still one program — `mint.owner` must
//! be the SPL Token program, and anything else fails with
//! `Token2022Unsupported` before a transfer is attempted. With the concrete
//! `Account<TokenAccount>` types, Anchor's own owner check would fail first with
//! the generic `AccountOwnedByWrongProgram` and the name would never be reached.
//!
//! Each instruction family lives in its own submodule; all of their public
//! items are re-exported here, so `lib.rs` and the IDL keep the same paths.
//! `settle` is a wallet paying an invoice, `treasury` is the admin opening the
//! fee's destination for a mint, and `budget` is the allowance. The two settle
//! paths — `settle` and the allowance's spend — share `split`, which prices an
//! invoice, and `payout`, which moves its tokens.

mod budget;
mod payout;
mod settle;
mod split;
mod treasury;

pub use budget::*;
pub use settle::*;
pub use treasury::*;
