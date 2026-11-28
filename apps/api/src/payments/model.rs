//! Payment domain types shared by creation, reads and storage.

use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;

/// Mirrors `PAYMENT_MAX_MEMO` in `apps/contracts/programs/payvand`.
pub const MAX_MEMO_LEN: usize = 96;

/// Title shown above the amount on the customer's screen.
pub const MAX_NAME_LEN: usize = 64;

/// Longer explanation shown under the title. Bounded because both fields ride
/// in the QR payload: every extra character makes the code denser and harder
/// for a phone camera to read.
pub const MAX_DESCRIPTION_LEN: usize = 160;

/// Lines a receipt may carry. Every line rides in the QR code, so this is the
/// number that keeps a full cart scannable on a phone camera.
pub const MAX_ITEMS: usize = 8;

/// Product names are the dense part of a receipt line, so they are short.
pub const MAX_ITEM_NAME_LEN: usize = 32;

/// One shelf of a shop is not a stockroom; more than this is a typo.
pub const MAX_ITEM_QUANTITY: u32 = 99;

pub const SOL_DECIMALS: u8 = 9;
pub const STABLE_DECIMALS: u8 = 6;

/// The currency tag used for native SOL.
pub const SOL_CURRENCY: &str = "sol";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentStatus {
    /// The invoice exists on chain and is waiting for a payer.
    Pending,
    /// The invoice has been settled on chain.
    Settled,
    /// No matching payment account is readable yet.
    Missing,
}

/// A discount baked into an invoice when the seller created it. The buyer's
/// screen applies it on its own: the code is never typed, and the page that
/// shows the card never displays it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentDiscount {
    pub code: String,
    pub percent_off: u32,
}

/// One way to pay an invoice: a currency, its price, and the on-chain address
/// that price is bound to. A seller who takes SOL and USDC puts two of these
/// on one invoice, and the buyer picks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentOption {
    /// `sol`, `usdc` or `eurc`.
    pub currency: String,
    /// The token mint, or the empty string for native SOL.
    pub mint: String,
    /// Decimals of `amount` — 9 for SOL, 6 for the stablecoins.
    pub decimals: u8,
    /// Price in the currency's base units.
    pub amount: u64,
    /// The PDA that holds this option's invoice once it is paid.
    pub payment_pda: String,
}

impl PaymentOption {
    pub fn is_sol(&self) -> bool {
        self.currency == SOL_CURRENCY
    }
}

/// One line of a receipt: what was sold, how many, and the unit price in
/// lamports. The customer sees it above the amount, so a coffee and a croissant
/// are itemised rather than collapsed into one unexplained total. The chain
/// stores only the memo and the amount; the lines live in the code itself.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaymentItem {
    pub name: String,
    pub quantity: u32,
    /// Unit price, not the line total: quantity is kept beside it.
    pub amount_lamports: u64,
}

/// The single shape the API returns, also embedded in the QR payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRecord {
    /// Same as `payment_pda`: the PDA is the stable payment identifier.
    pub id: String,
    pub status: PaymentStatus,
    pub payment_pda: String,
    pub seller: String,
    /// The signed-in account that created the invoice. Present only on the
    /// record returned by `create_payment`; status reads come from the chain,
    /// which knows nothing about accounts.
    pub seller_user: Option<String>,
    pub reference: String,
    /// The SOL price, kept for older readers: it is the `sol` option's amount,
    /// and zero on an invoice priced only in tokens.
    pub amount_lamports: u64,
    /// Every currency this invoice accepts. Never empty on a created invoice.
    #[serde(default)]
    pub options: Vec<PaymentOption>,
    pub memo: String,
    /// Seller's title for the payment; empty for records read back from chain,
    /// which stores no such fields.
    pub name: String,
    /// Seller's longer description, with the same caveat as `name`.
    pub description: String,
    /// Receipt lines when the seller priced the invoice from their shop. Empty
    /// for a hand-priced invoice, and always empty on a chain read.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<PaymentItem>,
    /// The code attached while creating the invoice, if any. Chain reads carry
    /// none: the discount lives in the payload, not in the account.
    pub discount: Option<PaymentDiscount>,
    /// Whether the buyer may apply a code of this seller's own choosing. An
    /// invoice carries a code or offers the choice, never both.
    #[serde(default)]
    pub allow_discounts: bool,
    pub cluster: String,
    pub endpoint: String,
    pub settled_by: Option<String>,
    pub created_at_unix: i64,
    pub settled_at_unix: i64,
    /// What the settlement charged the seller, in the paid option's base units,
    /// as the program logged it; `None` on an unsettled invoice and on a
    /// settlement whose event was not read.
    ///
    /// Never serialized: the public payment record reports no split (plan 0002,
    /// revision 15, pinned by `a_settled_record_claims_no_split`). The wallet's
    /// own transaction list is where it is shown, as `MyTransaction::fee`.
    #[serde(skip)]
    pub fee: Option<u64>,
}

/// How many addresses one watch request may carry — the seller's screen only
/// ever watches the invoice plus one address per discount code.
pub const MAX_WATCH_PDAS: usize = 64;

/// The identity half of a `Budget` account — enough to know which session
/// key is authorised to spend from a buyer's allowance.
pub struct BudgetAccount {
    pub buyer: Pubkey,
    pub session_key: Pubkey,
}
