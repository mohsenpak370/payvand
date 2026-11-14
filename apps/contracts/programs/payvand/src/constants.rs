//! Seeds, account sizes and the lamport floats the program parks on accounts.

/// Seed prefix for every payment PDA.
pub const PAYMENT_SEED: &[u8] = b"payment";
/// Maximum number of bytes stored in `Payment::memo`.
pub const PAYMENT_MAX_MEMO: usize = 96;

/// Seed prefix for the permanent receipt of a settled invoice.
///
/// The receipt is keyed by the invoice address itself, not by
/// `seller`/`reference`/`amount` or the mint: the API, a watcher and an
/// explorer all hold the invoice address from the QR code, and none of them can
/// recover the seeds from it. One receipt type for SOL and token invoices alike
/// keeps that lookup a single derivation, and the receipt stores the mint
/// (`Pubkey::default()` for SOL) so the record stays self-describing.
pub const PAYMENT_RECEIPT_SEED: &[u8] = b"payment-receipt";
/// `8` (discriminator) + `32 (invoice) + 32 (seller) + 32 (reference) + 32
/// (mint) + 8 (amount) + 4 + 96 (memo) + 32 (payer) + 8 (created_at) + 8
/// (settled_at) + 8 (closed_at) + 1 (bump)`.
pub const PAYMENT_RECEIPT_SPACE: usize =
    8 + 32 + 32 + 32 + 32 + 8 + 4 + PAYMENT_MAX_MEMO + 32 + 8 + 8 + 8 + 1;

/// Seed prefix for the buyer's monthly budget PDA.
pub const BUDGET_SEED: &[u8] = b"budget";
/// `8` (discriminator) + `32 + 32 + 8 + 8 + 8 + 1` (buyer, session key,
/// monthly limit, spent, month index, bump).
pub const BUDGET_SPACE: usize = 8 + 32 + 32 + 8 + 8 + 8 + 1;
/// Default lamports parked on a session key so its silent transactions can pay
/// their own network fees. `Config::session_fee_float` carries the live value;
/// `open_budget` reads it from there, and `initialize_config` starts from this.
pub const SESSION_FEE_FLOAT: u64 = 10_000_000;
/// Lower bound `update_config` accepts for `session_fee_float`: enough to front
/// the invoice and receipt rents of one SOL payment (`R_p + R_r`), which is
/// 2,491,680 + 2,985,840 with the 301-byte receipt.
pub const MIN_SESSION_FEE_FLOAT: u64 = 5_477_520;

/// Seed of the singleton fee policy account.
pub const CONFIG_SEED: &[u8] = b"config";
/// `8` (discriminator) + `32 (admin) + 2 (fee_bps) + 8 (fixed_fee_lamports) + 1
/// (fee_mode) + 8 (session_fee_float) + 1 (bump) + 1 (treasury_bump) + 1
/// (version)`.
pub const CONFIG_SPACE: usize = 8 + 32 + 2 + 8 + 1 + 8 + 1 + 1 + 1;

/// Seed of the treasury: a system-owned PDA with no data, the only account a
/// fee may be sent to. Deriving it from a seed is what makes the recipient
/// unredirectable — no settlement names a fee account, and the program moves
/// lamports out of it only by signing with these seeds.
pub const TREASURY_SEED: &[u8] = b"treasury";

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
