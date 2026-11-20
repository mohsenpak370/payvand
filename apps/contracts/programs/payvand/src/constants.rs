//! Seeds, account sizes and the lamport floats the program parks on accounts.

/// Seed prefix for every payment PDA.
pub const PAYMENT_SEED: &[u8] = b"payment";
/// Maximum number of bytes stored in `Payment::memo`.
pub const PAYMENT_MAX_MEMO: usize = 96;

/// Seed prefix for the permanent marker of a settled invoice.
///
/// The marker is keyed by the invoice address itself, not by
/// `seller`/`reference`/`amount` or the mint: the API, a watcher and an
/// explorer all hold the invoice address from the QR code, and none of them can
/// recover the seeds from it. One marker for SOL and token invoices alike keeps
/// that lookup a single derivation.
pub const PAYMENT_RECEIPT_SEED: &[u8] = b"payment-receipt";
/// `8` (discriminator) + `32` (invoice) + `8` (settled_at): the replay guard
/// and the pointer back to the invoice, and nothing else. The payment itself is
/// in the `PaymentSettled` event the settle transaction logs, which is why this
/// is a marker rather than a 301-byte copy of it — the record is 1,224,960
/// lamports of rent a payment instead of 2,985,840.
pub const PAYMENT_RECEIPT_SPACE: usize = 8 + 32 + 8;

/// Seed prefix for the buyer's monthly budget PDA.
pub const BUDGET_SEED: &[u8] = b"budget";
/// `8` (discriminator) + `32 + 32 + 8 + 8 + 8 + 1` (buyer, session key,
/// monthly limit, spent, month index, bump).
pub const BUDGET_SPACE: usize = 8 + 32 + 32 + 8 + 8 + 8 + 1;
/// Default lamports parked on a session key so its silent transactions can pay
/// their own network fees. `Config::session_fee_float` carries the live value;
/// `open_budget` reads it from there, and `initialize_config` starts from this.
///
/// The token flows read this constant directly, and it is sized for three
/// payments to sellers with no token account at mainnet's 6,960 lamports a byte.
/// Each one fronts 5,983,640 before the refund lands — both records, the seller's
/// account, the signature — and hands 3,939,360 of it back in the same
/// transaction, so the key is 2,044,280 lighter a payment while it needs the full
/// 5,983,640 to start the next one: the third needs 10,072,200 to begin. Beyond
/// three the buyer tops the float up rather than the account being sized for a
/// month of new sellers.
pub const SESSION_FEE_FLOAT: u64 = 12_000_000;
/// Lower bound `update_config` accepts for `session_fee_float`: enough to front
/// the invoice and receipt rents of one SOL payment (`R_p + R_r`), which is
/// 2,491,680 + 1,224,960 with the 48-byte marker.
pub const MIN_SESSION_FEE_FLOAT: u64 = 3_716_640;

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

/// The SPL Token mint layout: authority, supply, decimals, initialization flag
/// and the optional freeze authority. Every mint the program accepts is exactly
/// this long. A Token-2022 mint with extensions is longer, but its owner is
/// refused first, so the named error comes from the program, not a length.
pub const SPL_MINT_LEN: usize = 82;

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
