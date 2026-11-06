//! Seeds, account sizes and the lamport floats the program parks on accounts.

/// Seed prefix for every payment PDA.
pub const PAYMENT_SEED: &[u8] = b"payment";
/// Maximum number of bytes stored in `Payment::memo`.
pub const PAYMENT_MAX_MEMO: usize = 96;

/// Seed prefix for the buyer's monthly budget PDA.
pub const BUDGET_SEED: &[u8] = b"budget";
/// `8` (discriminator) + `32 + 32 + 8 + 8 + 8 + 1` (buyer, session key,
/// monthly limit, spent, month index, bump).
pub const BUDGET_SPACE: usize = 8 + 32 + 32 + 8 + 8 + 8 + 1;
/// Lamports parked on the session key so its silent transactions can pay
/// their own network fees (~2000 fees at 5000 lamports each). Funded by
/// `open_budget` and topped back up to this level on every later open.
pub const SESSION_FEE_FLOAT: u64 = 10_000_000;

/// `8` (discriminator) + `32 + 32 + 8 + 4 + 96 + 1 + 32 + 8 + 8 + 1`.
pub const PAYMENT_SPACE: usize = 8 + 32 + 32 + 8 + 4 + PAYMENT_MAX_MEMO + 1 + 32 + 8 + 8 + 1;

/// Seed prefix for an invoice paid in an SPL token. A separate prefix from
/// `PAYMENT_SEED` on purpose: the account carries a mint, so its layout
/// differs, and keeping it apart leaves every SOL invoice already on chain
/// readable by the old code path.
pub const TOKEN_PAYMENT_SEED: &[u8] = b"token-payment";
/// `8` + `32 (seller) + 32 (reference) + 32 (mint) + 8 (amount) + 4 + 96
/// (memo) + 1 (state) + 32 (payer) + 8 + 8 + 1 (bump)`.
pub const TOKEN_PAYMENT_SPACE: usize =
    8 + 32 + 32 + 32 + 8 + 4 + PAYMENT_MAX_MEMO + 1 + 32 + 8 + 8 + 1;

/// Seed prefix for a buyer's monthly token budget — one per buyer per mint.
pub const TOKEN_BUDGET_SEED: &[u8] = b"token-budget";
/// `8` + `32 (buyer) + 32 (session key) + 32 (mint) + 8 + 8 + 8 + 1 (bump)`.
pub const TOKEN_BUDGET_SPACE: usize = 8 + 32 + 32 + 32 + 8 + 8 + 8 + 1;
/// Lamports parked on a token budget so it can refund the invoice rent its
/// session key advances, without eating into the budget account's own rent.
pub const TOKEN_BUDGET_FEE_FLOAT: u64 = 10_000_000;
