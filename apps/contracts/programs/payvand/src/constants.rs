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
/// The token flows read this constant directly. One settlement fronts both of
/// its records — 2,714,400 for the 262-byte token invoice and 1,224,960 for the
/// 48-byte marker at mainnet's 6,960 lamports a byte — and the treasury refunds
/// both in the same transaction, so the peak is 3,939,360 and only the signature
/// is spent for good. This holds that peak with headroom. A seller's token
/// account is never opened here: the float is the buyer's money, so that rent is
/// the seller's own setup cost, not a line on the buyer's bill.
pub const SESSION_FEE_FLOAT: u64 = 12_000_000;
/// How many token settlements' worth of refunds `withdraw_treasury` always leaves
/// in the treasury, on top of the account's own rent-exempt minimum.
///
/// The treasury is where a token settlement's refund comes from: a token seller
/// cannot be charged lamports, so the pool advances both rents — the invoice's
/// and the marker's — and keeps the fee in the currency instead, and the refund
/// is never repaid in lamports. A withdrawal that left only the empty account's
/// rent would strand the treasury short of what `ensure_treasury_funded`
/// demands, and every token payment would be refused until someone topped it up.
/// One settlement's refund is the floor that guard enforces, so the reserve
/// leaves exactly one settlement's worth.
pub const TREASURY_RESERVE_SETTLEMENTS: u64 = 1;

/// Lower bound `update_config` accepts for `session_fee_float`: enough to front
/// the invoice and receipt rents of one SOL payment (`R_p + R_r`), which is
/// 2,491,680 + 1,224,960 with the 48-byte marker.
pub const MIN_SESSION_FEE_FLOAT: u64 = 3_716_640;

/// Seed of the singleton fee policy account.
pub const CONFIG_SEED: &[u8] = b"config";
/// The version 2 `Config` layout: `8` (discriminator), `32` (admin), the
/// `FeePolicy`'s `100`, `1` (fee_mode), `8` (session_fee_float), `1` (bump),
/// `1` (treasury_bump) and `1` (version). Version 3 adds `flats_updated_at`,
/// the cooldown `refresh_fee_price` reads, and is `CONFIG_SPACE`.
pub const CONFIG_SPACE_V2: usize = 8 + 32 + (2 + 2 + 8 + 8 + 8 + 8 + 32 + 32) + 1 + 8 + 1 + 1 + 1;

/// The version 3 `Config` layout: everything version 4 has but the network fee —
/// v2's `100`-byte `FeePolicy` and the `flats_updated_at` cooldown.
pub const CONFIG_SPACE_V3: usize = CONFIG_SPACE_V2 + 8;

/// `8` (discriminator) + `32 (admin) + 2 + 2 (the two rates) + 8 + 8 (their
/// boundaries) + 8 + 8 (the two per-currency flats) + 32 + 32 (the mints they
/// apply to) + 8 + 8 + 8 (the network fee in lamports, USDC base units and EURC
/// base units) + 1 (fee_mode) + 8 (session_fee_float) + 8 (flats_updated_at) + 1
/// (bump) + 1 (treasury_bump) + 1 (version)`.
pub const CONFIG_SPACE: usize = CONFIG_SPACE_V3 + 3 * 8;

/// The version `initialize_config` and `migrate_config` write.
pub const CONFIG_VERSION: u8 = 4;

/// How long the two token flats must stand before `refresh_fee_price` may move
/// them again. A minute is shorter than the app's own rate cache, so an
/// operator refreshing on its cooldown never finds the program refusing, and
/// long enough that nobody can walk the flats anywhere in a loop of
/// transactions.
pub const FLATS_REFRESH_SECS: i64 = 60;

/// How far one `refresh_fee_price` may price away from the admin's anchor, in
/// basis points. The call is permissionless — the app refreshes it from its own
/// rate feed — so the band is measured from a price only the admin writes and
/// never from the flat the last refresh left behind. A relative band would let
/// anyone walk the flats a step at a time, a minute apart, until the record fee
/// no longer covered the rent the treasury refunds; an anchored one cannot move
/// a country's worth of price no matter how many calls it makes, because every
/// accepted price is within this band of the same anchor.
pub const PRICE_ANCHOR_BAND_BPS: u64 = 1_000;

/// Seed of the price anchor: the small account holding the SOL price the admin
/// last blessed. `refresh_fee_price` reads it to bound a permissionless call,
/// and `set_fee_price_anchor` is the only instruction that writes it.
pub const FEE_ANCHOR_SEED: &[u8] = b"fee-anchor";

/// `8` (discriminator) + `8` (USD micros) + `8` (EUR micros) + `1` (bump).
pub const FEE_ANCHOR_SPACE: usize = 8 + 8 + 8 + 1;

/// Micro-units in one whole currency unit. A price arrives as `micros` per
/// SOL — 61_990_000 for €61.99 — and a flat is that price times the marker's
/// rent in SOL, in the mint's own base units.
pub const MICROS: u128 = 1_000_000;

/// The largest SOL price `refresh_fee_price` accepts, in micros (€10,000 a
/// SOL). A number past this is a unit mistake, not a price.
pub const MAX_SOL_PRICE_MICROS: u64 = 10_000 * 1_000_000;

/// The most a percentage fee may cost in one settlement, in lamports: 0.1 SOL.
///
/// The two rates are a percentage, so without a ceiling a large invoice keeps
/// handing over a larger share of itself however routine the work is: at 2% a
/// 50 SOL payment would cost 1 SOL, more than the service is worth at that size
/// and more than a seller can be expected to price into a big sale. The ceiling
/// makes the fee behave like a rate for the everyday and like a flat cost for the
/// unusual, so a high-volume payment is never punished for being large.
///
/// It bounds the fee actually charged, after the rate and the marker's rent have
/// had their say, so neither can carry the fee past this.
pub const MAX_SOL_FEE_LAMPORTS: u64 = 100_000_000;

/// The same ceiling in each token currency's base units: 10 USDC or 10 EURC.
///
/// One number covers both because the two named currencies are six-decimal
/// stablecoins — the same assumption the flats already make — and ten units is
/// the same money as 0.1 SOL's worth at the deployment's own prices.
pub const MAX_TOKEN_FEE_BASE_UNITS: u64 = 10_000_000;

/// The most a policy may put in `FeePolicy::network_fee_lamports`: 0.01 SOL.
///
/// The field is what a policy says one settlement's network fee costs, and a
/// token settle reimburses that much from the treasury. It is a cost we pay, not
/// a rate we set, so the ceiling exists only to stop a mistyped policy from
/// letting a settlement drain the pool one payment at a time.
pub const MAX_NETWORK_FEE_LAMPORTS: u64 = 10_000_000;

/// The same bound in a six-decimal token: one unit. A network fee's token
/// equivalent is derived from `network_fee_lamports` and a SOL price, and a
/// policy that names a larger one than this is a unit mistake, not a fee.
pub const MAX_NETWORK_FEE_BASE_UNITS: u64 = 1_000_000;

/// The size of the version 1 `Config` layout: 8 bytes of discriminator, 32 of
/// admin, 2 of `fee_bps`, 8 of `fixed_fee_lamports`, 1 of `fee_mode`, 8 of
/// `session_fee_float`, and 1 each of `bump`, `treasury_bump` and `version`.
/// Only `migrate_config` reads or writes this shape, and it is what tells the
/// migration it is looking at the account it can upgrade.
pub const CONFIG_SPACE_V1: usize = 8 + 32 + 2 + 8 + 1 + 8 + 1 + 1 + 1;

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

/// Where `decimals` sits in that base layout: `4` bytes of `COption<Pubkey>`
/// mint authority, its `32` bytes, then `8` of supply. Both token programs keep
/// these offsets, which is why the transfer CPIs can read decimals from the
/// unchecked mint account instead of parsing it as a `Mint`.
pub const MINT_DECIMALS_OFFSET: usize = 44;

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
