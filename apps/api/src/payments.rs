//! Payment request validation, QR payload formatting and on-chain status reads.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use solana_commitment_config::CommitmentConfig;
use solana_pubkey::Pubkey;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client::rpc_client::GetConfirmedSignaturesForAddress2Config;
use solana_rpc_client_api::config::{RpcTransactionConfig, UiTransactionEncoding};
use solana_signature::Signature;
use std::collections::{HashMap, VecDeque};
use std::str::FromStr;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use thiserror::Error;

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

/// Version tag carried in the QR payload so scanners can evolve the format.
pub const QR_PAYLOAD_VERSION: u8 = 1;

/// Seed prefix for SOL invoices, mirroring the Anchor program.
pub const PAYMENT_SEED: &[u8] = b"payment";
/// Seed prefix for token invoices. A different prefix, and the mint is one of
/// its seeds, so a USDC invoice and a SOL invoice never share an address.
pub const TOKEN_PAYMENT_SEED: &[u8] = b"token-payment";
/// Seed prefix for a settled invoice's permanent receipt. Keyed by the invoice
/// address, so the record is reachable from the address in the QR code alone —
/// which is all a watcher ever holds once the invoice account is closed.
pub const PAYMENT_RECEIPT_SEED: &[u8] = b"payment-receipt";

/// `["payment-receipt", invoice]`, mirroring the Anchor program.
pub fn receipt_pda(program: &Pubkey, invoice: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[PAYMENT_RECEIPT_SEED, invoice.as_ref()], program).0
}

/// SOL has nine decimals; the stablecoins the app takes have six.
pub const SOL_DECIMALS: u8 = 9;
pub const STABLE_DECIMALS: u8 = 6;

/// The currency tag used for native SOL.
pub const SOL_CURRENCY: &str = "sol";

/// The stablecoins an invoice may be priced in, with the mint each one uses on
/// a given cluster. Circle issues both; the addresses come from Circle's own
/// published list, never from an invoice or a request.
pub fn mint_for(currency: &str, cluster: &str) -> Option<Pubkey> {
    let (mainnet, devnet) = match currency {
        "usdc" => (
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU",
        ),
        // Circle publishes one EURC address per cluster; on devnet it is the
        // same one listed for testnet.
        "eurc" => (
            "HzwqbKZw8HxMN6bF2yFZNrht3c2iXXzpKcFu7uBEDKtr",
            "HzwqbKZw8HxMN6bF2yFZNrht3c2iXXzpKcFu7uBEDKtr",
        ),
        _ => return None,
    };
    let address = if matches!(cluster, "mainnet" | "mainnet-beta") {
        mainnet
    } else {
        devnet
    };
    Pubkey::from_str(address).ok()
}

/// The currency a mint is priced in, so a chain read can name it.
pub fn currency_for_mint(mint: &Pubkey, cluster: &str) -> Option<&'static str> {
    if mint_for("usdc", cluster).as_ref() == Some(mint) {
        return Some("usdc");
    }
    if mint_for("eurc", cluster).as_ref() == Some(mint) {
        return Some("eurc");
    }
    None
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("authentication required")]
    Unauthorized,
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    TooManyRequests(String),
    #[error("{0}")]
    Internal(String),
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = match &self {
            Self::BadRequest(_) => axum::http::StatusCode::BAD_REQUEST,
            Self::Unauthorized => axum::http::StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => axum::http::StatusCode::FORBIDDEN,
            Self::NotFound(_) => axum::http::StatusCode::NOT_FOUND,
            Self::TooManyRequests(_) => axum::http::StatusCode::TOO_MANY_REQUESTS,
            Self::Internal(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        };
        // An internal failure can name a table, a query or a file, so it is
        // logged where the operator can read it and answered with a sentence
        // that tells the caller only what to do next.
        let body = match &self {
            Self::Internal(message) => {
                tracing::error!(%message, "internal error");
                axum::Json(serde_json::json!({
                    "error": "Something went wrong on our side. Try again."
                }))
            }
            other => axum::Json(serde_json::json!({ "error": other.to_string() })),
        };
        (status, body).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

pub(crate) fn bad(message: impl Into<String>) -> ApiError {
    ApiError::BadRequest(message.into())
}

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
}

impl PaymentRecord {
    /// Compact, self-describing blob the seller encodes into the QR code.
    pub fn qr_payload(&self) -> String {
        // An invoice that takes SOL and nothing else keeps the old shape: the
        // code scans exactly as it did before a seller could offer currencies.
        let many = self.options.len() > 1 || self.options.iter().any(|option| !option.is_sol());
        let primary = self
            .options
            .iter()
            .find(|option| option.is_sol())
            .or_else(|| self.options.first())
            .map(|option| option.payment_pda.as_str())
            .unwrap_or(self.id.as_str());
        serde_json::to_string(&QrPayload {
            v: QR_PAYLOAD_VERSION,
            id: primary,
            seller: &self.seller,
            // Lets the buyer's screen ask the API for this seller's codes.
            seller_user: self.seller_user.as_deref(),
            reference: &self.reference,
            amount_lamports: self.amount_lamports,
            memo: &self.memo,
            name: &self.name,
            description: &self.description,
            items: &self.items,
            discount: self.discount.as_ref(),
            allow_discounts: self.allow_discounts,
            options: if many { &self.options } else { &[] },
            cluster: &self.cluster,
            endpoint: &self.endpoint,
        })
        .expect("qr payload is serializable")
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Serialize)]
struct QrPayload<'a> {
    v: u8,
    id: &'a str,
    seller: &'a str,
    seller_user: Option<&'a str>,
    reference: &'a str,
    amount_lamports: u64,
    memo: &'a str,
    // Skipped when empty so a payment without them keeps the code as sparse
    // (and as easy to scan) as it was before these fields existed.
    #[serde(skip_serializing_if = "str::is_empty")]
    name: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    description: &'a str,
    /// The receipt, skipped on an invoice that has none so ordinary codes keep
    /// exactly the shape they had before shops existed.
    #[serde(skip_serializing_if = "<[PaymentItem]>::is_empty")]
    items: &'a [PaymentItem],
    /// Skipped when absent, so an ordinary invoice scans exactly as before.
    #[serde(skip_serializing_if = "Option::is_none")]
    discount: Option<&'a PaymentDiscount>,
    /// Skipped when false: only an invoice that invites a code says so.
    #[serde(skip_serializing_if = "is_false")]
    allow_discounts: bool,
    /// The currencies this invoice accepts. Skipped for a SOL-only invoice,
    /// whose `id` and `amount_lamports` already say everything.
    #[serde(skip_serializing_if = "<[PaymentOption]>::is_empty")]
    options: &'a [PaymentOption],
    cluster: &'a str,
    endpoint: &'a str,
}

/// What the seller posts after its wallet has created the payment account.
#[derive(Debug, Deserialize)]
pub struct CreatePaymentRequest {
    /// Kept for older clients: the SOL invoice address they derived. Newer
    /// clients send `options` and leave this out, and the API derives every
    /// address itself.
    #[serde(default)]
    pub payment_pda: String,
    pub seller: String,
    pub reference: String,
    #[serde(default)]
    pub amount_lamports: u64,
    /// The currencies this invoice accepts, each with its own price. Empty
    /// means the older shape: one SOL price in `amount_lamports`.
    #[serde(default)]
    pub options: Vec<CreateOption>,
    pub memo: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// Receipt lines, when the invoice was built from a shop. Checked to add up
    /// to the SOL price, so a code can never itemise one total and charge
    /// another.
    #[serde(default)]
    pub items: Vec<PaymentItem>,
    /// Optional code to bake into this invoice. The handler resolves it to a
    /// percentage against the signed-in seller's own codes, so the body can
    /// name a code but never choose the discount.
    #[serde(default)]
    pub discount_code: Option<String>,
    /// Let the buyer pick one of the seller's codes on their own screen. A code
    /// in `discount_code` already decides the price, so the two are exclusive.
    #[serde(default)]
    pub allow_discounts: bool,
    pub cluster: String,
    pub endpoint: Option<String>,
}

/// One currency the seller chose to accept, with the price it charges in it.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateOption {
    pub currency: String,
    pub amount: u64,
}

/// `?cluster=devnet` / `?endpoint=...` on `GET /api/payments/{id}`.
#[derive(Debug, Default, Deserialize)]
pub struct PaymentQuery {
    pub cluster: Option<String>,
    pub endpoint: Option<String>,
}

/// Validates an intent and returns the canonical record plus the QR payload.
pub fn create_payment(request: CreatePaymentRequest) -> ApiResult<PaymentRecord> {
    parse_pubkey(&request.seller, "seller")?;
    parse_pubkey(&request.reference, "reference")?;
    if !request.payment_pda.is_empty() {
        parse_pubkey(&request.payment_pda, "payment_pda")?;
    }

    if request.options.is_empty() && request.amount_lamports == 0 {
        return Err(bad("amount_lamports must be greater than 0"));
    }
    if request.memo.len() > MAX_MEMO_LEN {
        return Err(bad(format!("memo must be at most {MAX_MEMO_LEN} bytes")));
    }

    if request.allow_discounts && request.discount_code.is_some() {
        return Err(bad(
            "An invoice carries a discount code or lets the buyer choose one, not both.",
        ));
    }

    // Derived before any field is moved out of the request.
    let options = build_options(&request)?;
    let items = validate_items(request.items, &options)?;

    let name = request.name.unwrap_or_default().trim().to_string();
    if name.len() > MAX_NAME_LEN {
        return Err(bad(format!("name must be at most {MAX_NAME_LEN} bytes")));
    }
    let description = request.description.unwrap_or_default().trim().to_string();
    if description.len() > MAX_DESCRIPTION_LEN {
        return Err(bad(format!(
            "description must be at most {MAX_DESCRIPTION_LEN} bytes"
        )));
    }

    let endpoint = resolve_endpoint(request.endpoint.as_deref(), &request.cluster)?;

    // The SOL option is what older readers understand, so it leads when the
    // seller accepts it; a token-only invoice leads with its first option.
    let id = options
        .iter()
        .find(|option| option.is_sol())
        .or_else(|| options.first())
        .map(|option| option.payment_pda.clone())
        .filter(|pda| !pda.is_empty())
        .unwrap_or_else(|| request.payment_pda.clone());
    let amount_lamports = options
        .iter()
        .find(|option| option.is_sol())
        .map(|option| option.amount)
        .unwrap_or(0);

    Ok(PaymentRecord {
        id: id.clone(),
        // Nothing is on chain yet: a freshly created invoice is unpaid.
        status: PaymentStatus::Pending,
        payment_pda: id,
        seller: request.seller,
        seller_user: None,
        reference: request.reference,
        amount_lamports,
        options,
        memo: request.memo,
        name,
        description,
        items,
        discount: None,
        allow_discounts: request.allow_discounts,
        cluster: request.cluster,
        endpoint,
        settled_by: None,
        created_at_unix: 0,
        settled_at_unix: 0,
    })
}

/// Remembers an invoice this API made: what it is called, what it sells, and
/// everything a status read or a history row needs to rebuild it — the seller,
/// the reference, the cluster, and per address the currency, mint and amount.
///
/// The name, the description and the receipt lines exist only in the code the
/// seller shares, so the chain cannot supply them. Everything else could be read
/// back from the cluster, one `getProgramAccounts` for the whole program; this
/// row is what lets a feed ask about its own invoices instead.
///
/// A row is written for every currency the invoice accepts, not only for its
/// primary address: the history is keyed by the address that was actually paid,
/// so an invoice taking SOL and USDC paid in USDC would otherwise find nothing
/// under that PDA.
///
/// Best effort by design. The invoice already exists on chain when this runs,
/// and a cache that failed to write must never turn a working code into an
/// error.
pub async fn remember_invoice(
    db: &libsql::Database,
    record: &PaymentRecord,
    seller_user: Option<&str>,
) {
    let name = record.name.trim().to_string();
    let description = record.description.trim().to_string();
    let items = serde_json::to_string(&record.items).unwrap_or_default();
    let now = crate::auth::unix_now();

    let mut rows: Vec<(String, String, String, u64)> = record
        .options
        .iter()
        .filter(|option| !option.payment_pda.is_empty())
        .map(|option| {
            (
                option.payment_pda.clone(),
                option.currency.clone(),
                option.mint.clone(),
                option.amount,
            )
        })
        .collect();
    // A request with no options is the older one-address shape: the record's own
    // id is the invoice, priced in SOL.
    if rows.is_empty() {
        rows.push((
            record.id.clone(),
            SOL_CURRENCY.to_string(),
            String::new(),
            record.amount_lamports,
        ));
    }

    let written = async {
        let connection = db.connect()?;
        for (id, currency, mint, amount) in &rows {
            connection
                .execute(
                    "INSERT INTO invoice_meta \
                        (id, seller, reference, cluster, endpoint, currency, mint, amount, \
                         memo, seller_user, name, description, items, created_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14) \
                     ON CONFLICT(id) DO UPDATE SET \
                        seller = excluded.seller, reference = excluded.reference, \
                        cluster = excluded.cluster, endpoint = excluded.endpoint, \
                        currency = excluded.currency, mint = excluded.mint, \
                        amount = excluded.amount, memo = excluded.memo, \
                        seller_user = COALESCE(excluded.seller_user, invoice_meta.seller_user), \
                        name = excluded.name, description = excluded.description, \
                        items = excluded.items",
                    libsql::params![
                        id.clone(),
                        record.seller.clone(),
                        record.reference.clone(),
                        record.cluster.clone(),
                        record.endpoint.clone(),
                        currency.clone(),
                        mint.clone(),
                        *amount as i64,
                        record.memo.clone(),
                        seller_user.map(str::to_owned),
                        name.clone(),
                        description.clone(),
                        items.clone(),
                        now
                    ],
                )
                .await?;
        }
        Ok::<(), libsql::Error>(())
    }
    .await;

    if let Err(error) = written {
        tracing::info!(%error, "invoice row was not written");
    }
}

/// The invoices this wallet made, newest first, as far as our own rows can
/// say — no cluster read at all.
///
/// Each row is one address the invoice can be paid at, so an invoice priced in
/// SOL and USDC is two entries — the same shape a chain read returns. The facts
/// come from the row rather than from a `pending_record`, on purpose: the chain
/// answering nothing (a hiccup, an invoice not yet created) must not make an
/// invoice disappear from its own seller's history.
///
/// `amount > 0` is the line between an invoice this API recorded and a row from
/// before it did: those carry only a name and were filled from the program scan,
/// so they keep the scan as their answer until it backfills them.
pub async fn recorded_invoices(
    db: &libsql::Database,
    wallet: &str,
    cluster: &str,
    limit: usize,
) -> Vec<PaymentRecord> {
    let read = async {
        let connection = db.connect()?;
        connection
            .query(
                // The limit is a number this module owns, so it is written into
                // the statement rather than bound: a bound LIMIT is read as a
                // string by some drivers, which is an error rather than a page.
                &format!(
                    "SELECT id, seller, reference, endpoint, currency, mint, amount, memo, created_at \
                     FROM invoice_meta \
                     WHERE seller = ?1 AND cluster = ?2 AND amount > 0 \
                     ORDER BY created_at DESC LIMIT {}",
                    limit.min(MAX_WATCH_PDAS * 4)
                ),
                (wallet.to_string(), cluster.to_string()),
            )
            .await
    }
    .await;

    let mut rows = match read {
        Ok(rows) => rows,
        Err(error) => {
            tracing::info!(%error, "recorded invoices were not readable");
            return Vec::new();
        }
    };
    let mut records = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        let Ok(id) = row.get::<String>(0) else {
            continue;
        };
        let seller = row.get::<String>(1).unwrap_or_default();
        let reference = row.get::<String>(2).unwrap_or_default();
        let endpoint = row.get::<String>(3).unwrap_or_default();
        let currency = row.get::<String>(4).unwrap_or_default();
        let mint = row.get::<String>(5).unwrap_or_default();
        let amount = row.get::<i64>(6).unwrap_or(0).max(0) as u64;
        let memo = row.get::<String>(7).unwrap_or_default();
        let created_at = row.get::<i64>(8).unwrap_or(0);
        let is_sol = currency == SOL_CURRENCY;
        records.push(PaymentRecord {
            id: id.clone(),
            status: PaymentStatus::Pending,
            payment_pda: id.clone(),
            seller,
            seller_user: None,
            reference,
            amount_lamports: if is_sol { amount } else { 0 },
            options: vec![PaymentOption {
                currency,
                mint,
                decimals: if is_sol {
                    SOL_DECIMALS
                } else {
                    STABLE_DECIMALS
                },
                amount,
                payment_pda: id,
            }],
            memo,
            name: String::new(),
            description: String::new(),
            items: Vec::new(),
            discount: None,
            allow_discounts: false,
            cluster: cluster.to_string(),
            endpoint,
            settled_by: None,
            created_at_unix: created_at,
            settled_at_unix: 0,
        });
    }
    records
}

/// The settlements this wallet paid, newest first.
///
/// These are rows history already resolved: the payer is only known from the
/// settle transaction, so an invoice this wallet bought is here rather than in
/// the invoice rows it did not make. No cluster read is needed to answer.
pub async fn settlements_paid_by(
    db: &libsql::Database,
    endpoint: &str,
    cluster: &str,
    wallet: &str,
    limit: usize,
) -> Vec<PaymentRecord> {
    settlements_paid_by_any(db, endpoint, cluster, &[wallet.to_string()], limit).await
}

/// The same rows for every key that can stand for one buyer: the wallet itself,
/// and the session keys its allowances pay with.
///
/// A silent payment is signed by the session key, so its row is keyed by that
/// address. The mapping behind the extra keys is in `session_keys` — learned
/// from a budget account once, never scanned for on the usual request.
pub async fn settlements_paid_by_any(
    db: &libsql::Database,
    endpoint: &str,
    cluster: &str,
    payers: &[String],
    limit: usize,
) -> Vec<PaymentRecord> {
    if payers.is_empty() {
        return Vec::new();
    }
    // One placeholder per payer: the wallet first, then its session keys.
    let placeholders = (0..payers.len())
        .map(|index| format!("?{}", index + 2))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT invoice, seller, reference, mint, amount, memo, payer, created_at, settled_at \
         FROM settled_payments WHERE endpoint = ?1 AND payer IN ({placeholders}) \
         ORDER BY settled_at DESC LIMIT {}",
        limit.min(MAX_WATCH_PDAS * 4)
    );
    let read = async {
        let connection = db.connect()?;
        let mut values: Vec<libsql::Value> = Vec::with_capacity(payers.len() + 1);
        values.push(libsql::Value::Text(endpoint.to_string()));
        values.extend(
            payers
                .iter()
                .map(|payer| libsql::Value::Text(payer.clone())),
        );
        connection.query(&sql, values).await
    }
    .await;

    let mut rows = match read {
        Ok(rows) => rows,
        Err(error) => {
            tracing::info!(%error, "stored settlements were not readable");
            return Vec::new();
        }
    };
    let mut records = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        let Ok(invoice) = row.get::<String>(0) else {
            continue;
        };
        let Ok(invoice) = Pubkey::from_str(&invoice) else {
            continue;
        };
        let seller = row.get::<String>(1).unwrap_or_default();
        let reference = row.get::<String>(2).unwrap_or_default();
        let mint = row.get::<String>(3).unwrap_or_default();
        let amount = row.get::<i64>(4).unwrap_or(0);
        let memo = row.get::<String>(5).unwrap_or_default();
        let payer = row.get::<String>(6).unwrap_or_default();
        let created_at = row.get::<i64>(7).unwrap_or(0);
        let settled_at = row.get::<i64>(8).unwrap_or(0);
        records.push(receipt_record(
            OnchainReceipt {
                invoice,
                seller: Pubkey::from_str(&seller).unwrap_or_default(),
                reference: Pubkey::from_str(&reference).unwrap_or_default(),
                mint: Pubkey::from_str(&mint).unwrap_or_default(),
                amount: amount.max(0) as u64,
                memo,
                payer: Pubkey::from_str(&payer).unwrap_or_default(),
                created_at,
                settled_at,
            },
            cluster.to_string(),
            endpoint.to_string(),
        ));
    }
    records
}

/// How long a wallet's allowances are trusted to be what they were when this
/// instance last looked.
///
/// The lookup is one bounded `getMultipleAccounts` on the addresses a buyer's
/// budgets live at, so it is cheap — but it is still a cluster read, and a feed
/// load is not the place to spend one every time. An hour is short enough that
/// an allowance opened mid-session is found on its own, and the refresh button
/// asks for a scan immediately regardless.
const ALLOWANCE_CHECK_SECS: i64 = 60 * 60;

/// The session keys the allowances of this wallet pay with, newest first.
///
/// Rows written when a budget account was read — by a scan, or by the one
/// lookup `detect_allowance` does. Empty is the normal answer for a wallet with
/// no allowance.
pub async fn session_keys_for(
    db: &libsql::Database,
    wallet: &str,
    cluster: &str,
    endpoint: &str,
) -> Vec<Pubkey> {
    let read = async {
        let connection = db.connect()?;
        connection
            .query(
                "SELECT session_key FROM session_keys \
                 WHERE wallet = ?1 AND cluster = ?2 AND endpoint = ?3 \
                 ORDER BY first_seen DESC LIMIT 8",
                libsql::params![
                    wallet.to_string(),
                    cluster.to_string(),
                    endpoint.to_string()
                ],
            )
            .await
    }
    .await;

    let Ok(mut rows) = read else {
        return Vec::new();
    };
    let mut keys = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        let Ok(key) = row.get::<String>(0) else {
            continue;
        };
        if let Ok(key) = Pubkey::from_str(&key) {
            keys.push(key);
        }
    }
    keys
}

/// Remembers that one session key spends for one wallet. `INSERT OR IGNORE`:
/// a budget is created once, so the pair cannot change, and a repeated look at
/// the same account costs nothing.
pub async fn remember_session_key(
    db: &libsql::Database,
    session_key: &str,
    wallet: &str,
    cluster: &str,
    endpoint: &str,
) {
    let now = crate::auth::unix_now();
    let written = async {
        let connection = db.connect()?;
        connection
            .execute(
                "INSERT OR IGNORE INTO session_keys \
                     (session_key, wallet, endpoint, cluster, first_seen) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                libsql::params![
                    session_key.to_string(),
                    wallet.to_string(),
                    endpoint.to_string(),
                    cluster.to_string(),
                    now
                ],
            )
            .await?;
        Ok::<(), crate::db::DbError>(())
    }
    .await;
    if let Err(error) = written {
        tracing::info!(%error, "a session key was not remembered");
    }
}

/// Remembers every budget a scan read, for whichever buyer it belongs to: the
/// next feed load then matches that buyer's silent payments without scanning.
pub async fn remember_budgets(
    db: &libsql::Database,
    budgets: &[BudgetAccount],
    cluster: &str,
    endpoint: &str,
) {
    for budget in budgets {
        if budget.session_key == budget.buyer {
            continue;
        }
        remember_session_key(
            db,
            &budget.session_key.to_string(),
            &budget.buyer.to_string(),
            cluster,
            endpoint,
        )
        .await;
    }
}

/// Whether this wallet's allowances are worth looking up again.
///
/// True when it has never been looked up, or when the last look is older than
/// [`ALLOWANCE_CHECK_SECS`]. A database that cannot answer says no: a feed load
/// is the wrong place to invent a cluster read.
pub async fn allowance_check_due(
    db: &libsql::Database,
    wallet: &str,
    cluster: &str,
    endpoint: &str,
) -> bool {
    let read = async {
        let connection = db.connect()?;
        let mut rows = connection
            .query(
                "SELECT checked_at FROM allowance_checks \
                 WHERE wallet = ?1 AND cluster = ?2 AND endpoint = ?3",
                libsql::params![
                    wallet.to_string(),
                    cluster.to_string(),
                    endpoint.to_string()
                ],
            )
            .await?;
        Ok::<Option<i64>, crate::db::DbError>(rows.next().await?.and_then(|row| row.get(0).ok()))
    }
    .await;

    match read {
        Ok(None) => true,
        Ok(Some(checked_at)) => crate::auth::unix_now() - checked_at >= ALLOWANCE_CHECK_SECS,
        Err(error) => {
            tracing::info!(%error, "allowance checks were not readable");
            false
        }
    }
}

/// Records that this wallet's allowances were just looked up, so the next load
/// is a row read whatever the answer was.
pub async fn mark_allowance_checked(
    db: &libsql::Database,
    wallet: &str,
    cluster: &str,
    endpoint: &str,
) {
    let now = crate::auth::unix_now();
    let written = async {
        let connection = db.connect()?;
        connection
            .execute(
                "INSERT INTO allowance_checks (wallet, endpoint, cluster, checked_at) \
                 VALUES (?1, ?2, ?3, ?4) \
                 ON CONFLICT(wallet, endpoint, cluster) \
                 DO UPDATE SET checked_at = excluded.checked_at",
                libsql::params![
                    wallet.to_string(),
                    endpoint.to_string(),
                    cluster.to_string(),
                    now
                ],
            )
            .await?;
        Ok::<(), crate::db::DbError>(())
    }
    .await;
    if let Err(error) = written {
        tracing::info!(%error, "an allowance check was not recorded");
    }
}

/// The session keys this buyer's allowances pay with, read off the cluster.
///
/// A budget's address is derived from the buyer and, for a token budget, the
/// mint, so the few an account can have are known without a scan: one
/// `getMultipleAccounts` on those addresses. `None` means the cluster did not
/// answer — the caller then leaves the check unrecorded and tries again later;
/// `Some(empty)` means this wallet has no allowance, which is worth knowing.
pub async fn detect_allowance(
    db: &libsql::Database,
    endpoint: &str,
    cluster: &str,
    buyer: &Pubkey,
) -> Option<Vec<Pubkey>> {
    let program = program_id().ok()?;
    let mut addresses = vec![crate::push_budget::budget_address(buyer, &program)];
    for currency in ["usdc", "eurc"] {
        if let Some(mint) = mint_for(currency, cluster) {
            addresses.push(crate::push_budget::token_budget_address(
                buyer, &mint, &program,
            ));
        }
    }

    let rpc = payment_rpc(endpoint.to_owned());
    let accounts = match rpc.get_multiple_accounts(&addresses).await {
        Ok(accounts) => accounts,
        Err(error) => {
            tracing::info!(%endpoint, %error, "allowance accounts not readable");
            return None;
        }
    };

    let mut session_keys = Vec::new();
    for (address, account) in addresses.iter().zip(accounts) {
        let Some(account) = account else {
            continue;
        };
        let Some(ScannedAccount::Budget(budget)) = classify_program_account(
            *address,
            &account.data,
            cluster.to_owned(),
            endpoint.to_owned(),
        ) else {
            continue;
        };
        // The address was derived from this buyer, so a different one in the
        // account is not this wallet's allowance.
        if budget.buyer != *buyer {
            continue;
        }
        session_keys.push(budget.session_key);
    }

    let budgets: Vec<BudgetAccount> = session_keys
        .iter()
        .map(|session_key| BudgetAccount {
            buyer: *buyer,
            session_key: *session_key,
        })
        .collect();
    remember_budgets(db, &budgets, cluster, endpoint).await;
    Some(session_keys)
}

/// What an invoice row from before this API kept amounts is missing, filled in
/// from the invoice or its marker.
///
/// The row is only ever completed, never rewritten: `amount = 0` is what marks a
/// row as unfilled, so the statement is a no-op for every row written since, and
/// the seller's own name, description and receipt lines are left exactly as they
/// were. Being one statement per row also makes a retry harmless.
async fn fill_invoice_row(db: &libsql::Database, record: &PaymentRecord) -> bool {
    let Some(option) = record
        .options
        .iter()
        .find(|option| option.amount > 0)
        .or_else(|| record.options.first())
    else {
        return false;
    };
    let written = async {
        let connection = db.connect()?;
        let changed = connection
            .execute(
                "UPDATE invoice_meta SET \
                     reference = CASE WHEN reference = '' THEN ?2 ELSE reference END, \
                     cluster   = CASE WHEN cluster   = '' THEN ?3 ELSE cluster   END, \
                     endpoint  = CASE WHEN endpoint  = '' THEN ?4 ELSE endpoint  END, \
                     currency  = ?5, mint = ?6, amount = ?7, \
                     memo      = CASE WHEN memo = '' THEN ?8 ELSE memo END \
                 WHERE id = ?1 AND amount = 0",
                libsql::params![
                    record.id.clone(),
                    record.reference.clone(),
                    record.cluster.clone(),
                    record.endpoint.clone(),
                    option.currency.clone(),
                    option.mint.clone(),
                    option.amount as i64,
                    record.memo.clone(),
                ],
            )
            .await?;
        Ok::<u64, crate::db::DbError>(changed)
    }
    .await;
    match written {
        Ok(changed) => changed > 0,
        Err(error) => {
            tracing::info!(%error, "a legacy invoice row was not filled");
            false
        }
    }
}

/// The rows this seller made before a row carried an amount, oldest first — the
/// ones the feed cannot list and so has to scan for. Bounded like any watch
/// request, and ids the chain already gave no answer for are left out.
async fn legacy_invoice_ids(db: &libsql::Database, wallet: &str) -> Vec<String> {
    let read = async {
        let connection = db.connect()?;
        connection
            .query(
                &format!(
                    "SELECT id FROM invoice_meta \
                     WHERE seller = ?1 AND amount = 0 \
                       AND id NOT IN (SELECT id FROM invoice_backfill) \
                     ORDER BY created_at ASC LIMIT {MAX_WATCH_PDAS}"
                ),
                libsql::params![wallet.to_string()],
            )
            .await
    }
    .await;

    let Ok(mut rows) = read else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        if let Ok(id) = row.get::<String>(0) {
            ids.push(id);
        }
    }
    ids
}

/// Remembers the ids the chain had nothing to say about, so they are not asked
/// about again.
async fn remember_backfill_tried(db: &libsql::Database, ids: &[String], endpoint: &str) {
    if ids.is_empty() {
        return;
    }
    let now = crate::auth::unix_now();
    let written = async {
        let connection = db.connect()?;
        for id in ids {
            connection
                .execute(
                    "INSERT OR IGNORE INTO invoice_backfill (id, endpoint, tried_at) \
                     VALUES (?1, ?2, ?3)",
                    libsql::params![id.clone(), endpoint.to_string(), now],
                )
                .await?;
        }
        Ok::<(), crate::db::DbError>(())
    }
    .await;
    if let Err(error) = written {
        tracing::info!(%error, "a backfill attempt was not recorded");
    }
}

/// Fills this seller's rows from before amounts were kept, from the chain.
///
/// Without it a returning seller's feed has no row it can list — every row it
/// has is one the read skips — so the program scan becomes the only thing that
/// answers, on every load. One batched read fills the page once; after that the
/// rows carry what the feed needs and the scan is behind `?scan=1` again.
///
/// A read that fails changes nothing, so the next load tries again; a row the
/// chain does not know is remembered and left alone.
pub async fn backfill_invoice_rows(
    db: &libsql::Database,
    wallet: &str,
    cluster: &str,
    endpoint: &str,
) {
    let ids = legacy_invoice_ids(db, wallet).await;
    if ids.is_empty() {
        return;
    }
    // The endpoint is the one the caller already checked against the allowlist,
    // so this is the read itself rather than a request that resolves one.
    let records = match fetch_many_at(endpoint, cluster, &ids, db).await {
        Ok(records) => records,
        Err(error) => {
            tracing::info!(%error, "legacy invoices not readable");
            return;
        }
    };

    let mut unknown = Vec::new();
    for id in &ids {
        match records.iter().find(|record| &record.id == id) {
            Some(record) if record.options.iter().any(|option| option.amount > 0) => {
                fill_invoice_row(db, record).await;
            }
            _ => unknown.push(id.clone()),
        }
    }
    remember_backfill_tried(db, &unknown, endpoint).await;
}

/// Writes what a fallback scan found, so the next request is answered from our
/// own rows: the fallback is a one-off cost per invoice, not a per-request one.
///
/// `INSERT OR IGNORE` on purpose — a row this API wrote carries a seller's name,
/// description and receipt lines, and a scan cannot improve on any of them. The
/// facts it does add are the ones the old rows were missing.
pub async fn remember_scanned(db: &libsql::Database, records: &[PaymentRecord]) {
    let now = crate::auth::unix_now();
    let written = async {
        let connection = db.connect()?;
        for record in records {
            let mut rows: Vec<(String, String, String, u64)> = record
                .options
                .iter()
                .filter(|option| !option.payment_pda.is_empty())
                .map(|option| {
                    (
                        option.payment_pda.clone(),
                        option.currency.clone(),
                        option.mint.clone(),
                        option.amount,
                    )
                })
                .collect();
            if rows.is_empty() {
                rows.push((
                    record.id.clone(),
                    SOL_CURRENCY.to_string(),
                    String::new(),
                    record.amount_lamports,
                ));
            }
            for (id, currency, mint, amount) in &rows {
                connection
                    .execute(
                        "INSERT OR IGNORE INTO invoice_meta \
                            (id, seller, reference, cluster, endpoint, currency, mint, amount, \
                             memo, seller_user, name, description, items, created_at) \
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, '', '', '', ?10)",
                        libsql::params![
                            id.clone(),
                            record.seller.clone(),
                            record.reference.clone(),
                            record.cluster.clone(),
                            record.endpoint.clone(),
                            currency.clone(),
                            mint.clone(),
                            *amount as i64,
                            record.memo.clone(),
                            now
                        ],
                    )
                    .await?;
            }
        }
        Ok::<(), libsql::Error>(())
    }
    .await;

    if let Err(error) = written {
        tracing::info!(%error, "scanned invoices were not recorded");
    }
}

/// Turns the requested currencies into priced options, deriving the invoice
/// address for each one. Addresses are derived here rather than taken from the
/// request: the seeds (`seller`, `reference`, currency, amount) are the whole
/// safety story of a QR code, so the API computes them from the same facts it
/// validated instead of trusting an address a client sent.
pub fn build_options(request: &CreatePaymentRequest) -> ApiResult<Vec<PaymentOption>> {
    let program = program_id()?;
    let seller = parse_pubkey(&request.seller, "seller")?;
    let reference = parse_pubkey(&request.reference, "reference")?;

    // No options means an older client: one SOL price, as before.
    if request.options.is_empty() {
        let amount = request.amount_lamports;
        if amount == 0 {
            return Err(bad("amount_lamports must be greater than 0"));
        }
        let (pda, _) = Pubkey::find_program_address(
            &[
                PAYMENT_SEED,
                seller.as_ref(),
                reference.as_ref(),
                &amount.to_le_bytes(),
            ],
            &program,
        );
        return Ok(vec![PaymentOption {
            currency: SOL_CURRENCY.to_string(),
            mint: String::new(),
            decimals: SOL_DECIMALS,
            amount,
            payment_pda: pda.to_string(),
        }]);
    }

    if request.options.len() > MAX_OPTIONS {
        return Err(bad(format!(
            "an invoice can accept at most {MAX_OPTIONS} currencies"
        )));
    }

    let mut options: Vec<PaymentOption> = Vec::with_capacity(request.options.len());
    for requested in &request.options {
        let currency = requested.currency.trim().to_ascii_lowercase();
        if requested.amount == 0 {
            return Err(bad(format!("the {currency} amount must be greater than 0")));
        }
        if options.iter().any(|option| option.currency == currency) {
            return Err(bad(format!("{currency} is offered twice")));
        }

        if currency == SOL_CURRENCY {
            let (pda, _) = Pubkey::find_program_address(
                &[
                    PAYMENT_SEED,
                    seller.as_ref(),
                    reference.as_ref(),
                    &requested.amount.to_le_bytes(),
                ],
                &program,
            );
            options.push(PaymentOption {
                currency,
                mint: String::new(),
                decimals: SOL_DECIMALS,
                amount: requested.amount,
                payment_pda: pda.to_string(),
            });
            continue;
        }

        let mint = mint_for(&currency, &request.cluster).ok_or_else(|| {
            bad(format!(
                "{currency} cannot be charged on the {} network",
                request.cluster
            ))
        })?;
        let (pda, _) = Pubkey::find_program_address(
            &[
                TOKEN_PAYMENT_SEED,
                seller.as_ref(),
                reference.as_ref(),
                mint.as_ref(),
                &requested.amount.to_le_bytes(),
            ],
            &program,
        );
        options.push(PaymentOption {
            currency,
            mint: mint.to_string(),
            decimals: STABLE_DECIMALS,
            amount: requested.amount,
            payment_pda: pda.to_string(),
        });
    }

    Ok(options)
}

/// Cleans the receipt and makes it agree with the price. A code whose lines add
/// up to something other than what it charges is worse than no receipt at all,
/// so it is refused here rather than shown to a customer.
fn validate_items(
    items: Vec<PaymentItem>,
    options: &[PaymentOption],
) -> ApiResult<Vec<PaymentItem>> {
    if items.is_empty() {
        return Ok(items);
    }
    if items.len() > MAX_ITEMS {
        return Err(bad(format!(
            "A receipt can list at most {MAX_ITEMS} different products."
        )));
    }

    let mut cleaned = Vec::with_capacity(items.len());
    let mut total: u128 = 0;
    for item in items {
        let name = item.name.trim().to_string();
        if name.is_empty() {
            return Err(bad("Every receipt line needs a name."));
        }
        if name.chars().count() > MAX_ITEM_NAME_LEN {
            return Err(bad(format!(
                "A receipt line name must be at most {MAX_ITEM_NAME_LEN} characters."
            )));
        }
        if item.quantity == 0 || item.quantity > MAX_ITEM_QUANTITY {
            return Err(bad(format!(
                "A receipt line can be for 1 to {MAX_ITEM_QUANTITY} of an item."
            )));
        }
        if item.amount_lamports == 0 {
            return Err(bad("A receipt line needs a price greater than 0."));
        }
        total += u128::from(item.amount_lamports) * u128::from(item.quantity);
        cleaned.push(PaymentItem {
            name,
            quantity: item.quantity,
            amount_lamports: item.amount_lamports,
        });
    }

    // Only a SOL price can be checked here: an invoice priced purely in a
    // stablecoin has no lamport total to compare against.
    if let Some(sol) = options.iter().find(|option| option.is_sol()) {
        if total != u128::from(sol.amount) {
            return Err(bad(
                "The receipt lines do not add up to the invoice amount.",
            ));
        }
    }

    Ok(cleaned)
}

/// SOL, USDC and EURC — more than that makes the QR too dense to scan.
pub const MAX_OPTIONS: usize = 3;

/// `declare_id!` of the program, as a pubkey.
fn program_id() -> ApiResult<Pubkey> {
    Pubkey::from_str(PAYVAND_PROGRAM_ID)
        .map_err(|error| ApiError::Internal(format!("bad program id: {error}")))
}

/// Reads the payment account straight from the cluster. The handler is
/// stateless so it behaves the same locally and inside a serverless isolate.
pub async fn fetch_payment(
    payment_pda: &str,
    query: PaymentQuery,
    db: &libsql::Database,
) -> ApiResult<PaymentRecord> {
    let pubkey = parse_pubkey(payment_pda, "id")?;
    let cluster = query.cluster.clone().unwrap_or_else(default_cluster);
    let endpoint = resolve_endpoint(query.endpoint.as_deref(), &cluster)?;
    fetch_payment_at(&endpoint, &cluster, pubkey, db).await
}

/// The read itself, from an endpoint already checked against the allowlist —
/// which is what lets a test point it at a stand-in cluster and count what it
/// asked.
async fn fetch_payment_at(
    endpoint: &str,
    cluster: &str,
    pubkey: Pubkey,
    db: &libsql::Database,
) -> ApiResult<PaymentRecord> {
    // A page polls this address every five seconds, and the same invoice is
    // open in a second tab and on the seller's screen: one recent answer serves
    // all of them, and the cluster is asked once per address per window instead
    // of once per viewer per tick.
    if let Some(cached) = cached_pending(endpoint, &pubkey) {
        return Ok(cached);
    }

    let program = program_id()?;
    let receipt_address = receipt_pda(&program, &pubkey);

    // The invoice and its receipt are two different addresses, and only one of
    // them exists at a time, so ask for both in one round trip.
    let rpc = payment_rpc(endpoint.to_owned());
    let accounts = match rpc.get_multiple_accounts(&[pubkey, receipt_address]).await {
        Ok(accounts) => accounts,
        Err(error) => {
            tracing::info!(invoice = %pubkey, %error, "payment account not readable");
            // A cluster hiccup is not "unpaid": say so instead of inventing a
            // status, so the caller can retry and keep its last known state.
            return Err(ApiError::Internal(
                "The cluster did not answer. Try again.".into(),
            ));
        }
    };
    let invoice_data = accounts
        .first()
        .and_then(|account| account.as_ref())
        .map(|account| account.data.as_slice());
    let receipt_data = accounts
        .get(1)
        .and_then(|account| account.as_ref())
        .map(|account| account.data.as_slice());

    // One address is one invoice, so one history read is the whole need: a
    // settled invoice the cache has not seen yet is read once, here.
    let mut reader = InvoiceReader::new(&rpc, db, cluster.to_owned(), endpoint.to_owned(), 1);
    let record = reader.record(pubkey, invoice_data, receipt_data).await;
    // A failure above returned already, so what is remembered here is always a
    // status: settled goes to the cache that never expires, and anything else
    // gets the short window.
    remember_pending(endpoint, pubkey, &record);
    Ok(record)
}

/// The client every invoice read goes through.
///
/// `RpcClient::new` reads at `finalized`, which trails the `confirmed` a wallet
/// waits for by a dozen seconds on mainnet: a payment the buyer could already see
/// in their wallet came back `pending` from this API, and the checkout had to
/// read the chain itself to cover the gap. `confirmed` is the same level the send
/// settled at, so the record catches up within a slot or two.
pub(crate) fn payment_rpc(endpoint: String) -> RpcClient {
    RpcClient::new_with_commitment(endpoint, CommitmentConfig::confirmed())
}

/// The best record for one invoice address: the settlement behind its marker if
/// it was paid, the invoice account itself if it still exists, and otherwise the
/// placeholder a watcher reads as "not paid yet".
///
/// Everything one request's invoice reads share: the cluster to ask, the cache
/// behind the chain, and the history this request may still spend. `fetch_many`
/// holds one across a whole batch, so its budget covers the poll rather than
/// each address.
struct InvoiceReader<'a> {
    rpc: &'a RpcClient,
    db: &'a libsql::Database,
    cluster: String,
    endpoint: String,
    budget: HistoryBudget,
}

impl<'a> InvoiceReader<'a> {
    /// `wanted` is how many settlements this caller can show; the budget caps
    /// it, and a cache hit spends nothing.
    fn new(
        rpc: &'a RpcClient,
        db: &'a libsql::Database,
        cluster: String,
        endpoint: String,
        wanted: usize,
    ) -> Self {
        Self {
            rpc,
            db,
            cluster,
            endpoint,
            budget: HistoryBudget::new(wanted),
        }
    }

    /// The record for one settlement marker. A marker alone no longer describes
    /// a payment, so this needs the settle transaction — read at most once per
    /// invoice, and within this request's budget. When neither memory, the
    /// database nor history can answer, the marker's own answer (settled, and
    /// which invoice it closed) is still returned: the cluster settled it, and a
    /// flaky node must not turn that back into "pending".
    async fn settled(&mut self, marker: ReceiptMarker) -> PaymentRecord {
        settled_record(
            self.rpc,
            self.db,
            marker,
            self.cluster.clone(),
            self.endpoint.clone(),
            &mut self.budget,
        )
        .await
    }

    /// The best record for one invoice address: the settlement behind its marker
    /// if it was paid, the invoice account itself if it still exists, and
    /// otherwise the placeholder a watcher reads as "not paid yet".
    async fn record(
        &mut self,
        invoice: Pubkey,
        invoice_data: Option<&[u8]>,
        receipt_data: Option<&[u8]>,
    ) -> PaymentRecord {
        if let Some(marker) = receipt_data.and_then(decode_receipt_marker) {
            return self.settled(marker).await;
        }
        match invoice_data
            .and_then(|data| record_for(invoice, data, self.cluster.clone(), self.endpoint.clone()))
        {
            Some(record) => record,
            None => pending_record(invoice, self.cluster.clone(), self.endpoint.clone()),
        }
    }
}

/// The record for whatever kind of invoice lives at an address — SOL or token.
/// Anything else (a budget, a stranger's account) is not an invoice.
fn record_for(
    pubkey: Pubkey,
    data: &[u8],
    cluster: String,
    endpoint: String,
) -> Option<PaymentRecord> {
    if let Some(payment) = decode_payment(data) {
        return Some(record_from_chain(pubkey, payment, cluster, endpoint));
    }
    decode_token_payment(data)
        .map(|payment| token_record_from_chain(pubkey, payment, cluster, endpoint))
}

/// Placeholder returned for an address with no receipt and no invoice account —
/// to every watcher it is an invoice that has not been paid yet.
///
/// `Missing` is deliberately not used here: an address that derives an invoice
/// is a real invoice, whether or not anyone has paid it. Invalid input is a
/// rejected request, not a status.
fn pending_record(pubkey: Pubkey, cluster: String, endpoint: String) -> PaymentRecord {
    PaymentRecord {
        id: pubkey.to_string(),
        status: PaymentStatus::Pending,
        payment_pda: pubkey.to_string(),
        seller: String::new(),
        seller_user: None,
        reference: String::new(),
        amount_lamports: 0,
        options: Vec::new(),
        memo: String::new(),
        name: String::new(),
        description: String::new(),
        items: Vec::new(),
        discount: None,
        allow_discounts: false,
        cluster,
        endpoint,
        settled_by: None,
        created_at_unix: 0,
        settled_at_unix: 0,
    }
}

fn record_from_chain(
    pubkey: Pubkey,
    payment: OnchainPayment,
    cluster: String,
    endpoint: String,
) -> PaymentRecord {
    PaymentRecord {
        id: pubkey.to_string(),
        status: if payment.state == 1 {
            PaymentStatus::Settled
        } else {
            PaymentStatus::Pending
        },
        payment_pda: pubkey.to_string(),
        seller: payment.seller.to_string(),
        seller_user: None,
        reference: payment.reference.to_string(),
        amount_lamports: payment.amount,
        // A chain read describes one address, which is one option: the seller
        // screen knows the rest from the invoice's own code.
        options: vec![PaymentOption {
            currency: SOL_CURRENCY.to_string(),
            mint: String::new(),
            decimals: SOL_DECIMALS,
            amount: payment.amount,
            payment_pda: pubkey.to_string(),
        }],
        memo: payment.memo,
        name: String::new(),
        description: String::new(),
        items: Vec::new(),
        discount: None,
        allow_discounts: false,
        cluster,
        endpoint,
        settled_by: (payment.payer != Pubkey::default()).then(|| payment.payer.to_string()),
        created_at_unix: payment.created_at,
        settled_at_unix: payment.settled_at,
    }
}

/// `POST /api/payments/watch`: several payment addresses in one round trip.
#[derive(Debug, Deserialize)]
pub struct WatchRequest {
    pub pdas: Vec<String>,
    pub cluster: Option<String>,
    pub endpoint: Option<String>,
}

/// How many addresses one watch request may carry — the seller's screen only
/// ever watches the invoice plus one address per discount code.
pub const MAX_WATCH_PDAS: usize = 64;

/// Accounts one `getMultipleAccounts` request may name, a JSON-RPC limit. Each
/// watched invoice consumes two keys (invoice and receipt), so the watch read
/// batches at half of this.
const MAX_ACCOUNTS_PER_RPC: usize = 100;

/// Reads many payment accounts in a single RPC round trip so the seller's
/// screen can watch the invoice address and every discounted variant at once.
/// Addresses with no receipt and no invoice account come back as `Pending`,
/// the normal state of a QR nobody has paid yet. A cluster that does not answer
/// is an error, not a status, so the seller's screen keeps its last known one.
pub async fn fetch_many(
    request: WatchRequest,
    db: &libsql::Database,
) -> ApiResult<Vec<PaymentRecord>> {
    if request.pdas.is_empty() {
        return Ok(Vec::new());
    }
    if request.pdas.len() > MAX_WATCH_PDAS {
        return Err(bad(format!(
            "at most {MAX_WATCH_PDAS} addresses per watch request"
        )));
    }

    let cluster = request.cluster.clone().unwrap_or_else(default_cluster);
    let endpoint = resolve_endpoint(request.endpoint.as_deref(), &cluster)?;
    fetch_many_at(&endpoint, &cluster, &request.pdas, db).await
}

/// The batch itself, from an endpoint already checked against the allowlist.
async fn fetch_many_at(
    endpoint: &str,
    cluster: &str,
    pdas: &[String],
    db: &libsql::Database,
) -> ApiResult<Vec<PaymentRecord>> {
    let pubkeys = pdas
        .iter()
        .map(|pda| parse_pubkey(pda, "pdas"))
        .collect::<ApiResult<Vec<_>>>()?;

    let program = program_id()?;
    // `confirmed`, like every other invoice read: a feed that waited for
    // `finalized` would show a payment a dozen seconds after the buyer saw it.
    let rpc = payment_rpc(endpoint.to_owned());
    // The seller's screen watches the very addresses the buyer's page is polling,
    // so a recent answer is shared rather than asked for again; what is left is
    // batched into one round trip as before.
    let mut answered: HashMap<Pubkey, PaymentRecord> = HashMap::new();
    let mut unresolved = Vec::new();
    for pubkey in &pubkeys {
        match cached_pending(endpoint, pubkey) {
            Some(record) => {
                answered.insert(*pubkey, record);
            }
            None => unresolved.push(*pubkey),
        }
    }
    // A watched address is an invoice the caller is about to display, so it may
    // be resolved from history — but only once: the next poll is a cache hit.
    let mut reader = InvoiceReader::new(
        &rpc,
        db,
        cluster.to_owned(),
        endpoint.to_owned(),
        pubkeys.len(),
    );

    // Two accounts per invoice (the invoice and its receipt), and JSON-RPC caps
    // one `getMultipleAccounts` call at 100 keys, so batch the addresses.
    for batch in unresolved.chunks(MAX_WATCH_PDAS.min(MAX_ACCOUNTS_PER_RPC / 2)) {
        let mut keys = Vec::with_capacity(batch.len() * 2);
        for invoice in batch {
            keys.push(*invoice);
            keys.push(receipt_pda(&program, invoice));
        }

        let accounts = match rpc.get_multiple_accounts(&keys).await {
            Ok(accounts) => accounts,
            Err(error) => {
                tracing::info!(%error, "payment accounts not readable");
                return Err(ApiError::Internal(
                    "The cluster did not answer. Try again.".into(),
                ));
            }
        };

        for (index, invoice) in batch.iter().enumerate() {
            let invoice_data = accounts
                .get(index * 2)
                .and_then(|account| account.as_ref())
                .map(|account| account.data.as_slice());
            let receipt_data = accounts
                .get(index * 2 + 1)
                .and_then(|account| account.as_ref())
                .map(|account| account.data.as_slice());
            let record = reader.record(*invoice, invoice_data, receipt_data).await;
            remember_pending(endpoint, *invoice, &record);
            answered.insert(*invoice, record);
        }
    }

    // In the order the caller named them: a seller's screen pairs each address
    // with the discount code it belongs to by position.
    Ok(pubkeys
        .iter()
        .filter_map(|pubkey| answered.remove(pubkey))
        .collect())
}

/// `declare_id!` of the program in `apps/contracts/programs/payvand` —
/// the sole owner of payment and budget accounts.
pub const PAYVAND_PROGRAM_ID: &str = "4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J";

/// Every payment account the cluster holds for this program, undecodable
/// accounts skipped.
pub struct AllPayments {
    pub records: Vec<PaymentRecord>,
    pub cluster: String,
    pub endpoint: String,
}

/// The two account kinds the program owns that a scan must tell apart:
/// invoices, and the monthly budgets that can pay them.
pub struct ProgramScan {
    pub payments: Vec<PaymentRecord>,
    pub budgets: Vec<BudgetAccount>,
    pub cluster: String,
    pub endpoint: String,
}

/// The identity half of a `Budget` account — enough to know which session
/// key is authorised to spend from a buyer's allowance.
pub struct BudgetAccount {
    pub buyer: Pubkey,
    pub session_key: Pubkey,
}

/// Invoices live only on chain — the API keeps no copy — so listing them
/// means asking the program for its accounts (`get_program_accounts`).
/// That scan is fine while the platform is small; revisit if the account
/// count ever grows into the thousands.
///
/// `history_reads` is how many settled invoices this caller can actually show
/// (a page size): those are resolved from their settle transactions, newest
/// first, and every other marker is answered from itself. A cache hit costs
/// nothing, so a warm instance resolves none of them.
pub async fn fetch_program_scan(
    history_reads: usize,
    db: &libsql::Database,
) -> ApiResult<ProgramScan> {
    let cluster = default_cluster();
    let endpoint = resolve_endpoint(None, &cluster)?;
    program_scan_at(&endpoint, &cluster, history_reads, db).await
}

/// The scan itself, from an endpoint already checked against the allowlist.
async fn program_scan_at(
    endpoint: &str,
    cluster: &str,
    history_reads: usize,
    db: &libsql::Database,
) -> ApiResult<ProgramScan> {
    let program_id = Pubkey::from_str(PAYVAND_PROGRAM_ID)
        .map_err(|error| ApiError::Internal(format!("bad program id: {error}")))?;

    // A scan a moment ago answers this one: the fallback is the cold path, but
    // three tabs and the admin list still make a burst of the same read.
    let accounts = match cached_scan(endpoint, cluster) {
        Some(accounts) => accounts,
        None => {
            // `confirmed`, not the `finalized` a bare `RpcClient::new` reads at:
            // an invoice is created, and a receipt written, in transactions the
            // wallet has already seen confirmed, so a finalized scan left a
            // payment that had just gone through out of the history for another
            // dozen seconds.
            let rpc = payment_rpc(endpoint.to_owned());
            let read = match rpc.get_program_accounts(&program_id).await {
                Ok(accounts) => accounts,
                Err(error) => {
                    tracing::info!(%error, "program accounts not readable");
                    // A failure is not a scan: it is not remembered, so the next
                    // request asks again rather than serving an empty program.
                    return Err(ApiError::Internal(
                        "The cluster did not return the payment list. Try again.".into(),
                    ));
                }
            };
            let accounts: Vec<(Pubkey, Vec<u8>)> = read
                .into_iter()
                .map(|(pubkey, account)| (pubkey, account.data))
                .collect();
            remember_scan(endpoint, cluster, &accounts);
            accounts
        }
    };

    let mut payments = Vec::new();
    let mut budgets = Vec::new();
    // Markers are collected rather than resolved where they are found: the
    // budget has to go to the newest settlements, and the account order a scan
    // returns is arbitrary.
    let mut markers = Vec::new();
    for (pubkey, data) in accounts {
        match classify_program_account(pubkey, &data, cluster.to_owned(), endpoint.to_owned()) {
            Some(ScannedAccount::Payment(record)) => payments.push(*record),
            Some(ScannedAccount::Settled(marker)) => markers.push(marker),
            Some(ScannedAccount::Budget(budget)) => budgets.push(budget),
            None => {}
        }
    }

    markers.sort_by_key(|marker| std::cmp::Reverse(marker.settled_at));

    // Every marker in one database read, and the ones history has to answer for
    // a few at a time. Serial — one remote query per marker, then two paced RPC
    // calls behind each miss — a page of twenty settlements took half a minute.
    let stored = stored_settlements(db, endpoint, &markers).await;
    let mut budget = HistoryBudget::new(history_reads);
    let mut unresolved = Vec::new();
    for marker in markers {
        if let Some(cached) = cached_settlement(endpoint, &marker.invoice) {
            payments.push(receipt_record(
                cached,
                cluster.to_owned(),
                endpoint.to_owned(),
            ));
        } else if let Some(known) = stored.get(&marker.invoice) {
            remember_settlement(endpoint, marker.invoice, known);
            payments.push(receipt_record(
                known.clone(),
                cluster.to_owned(),
                endpoint.to_owned(),
            ));
        } else if budget.take() {
            unresolved.push(marker);
        } else {
            payments.push(marker_record(
                marker,
                cluster.to_owned(),
                endpoint.to_owned(),
            ));
        }
    }

    let invoices: Vec<Pubkey> = unresolved.iter().map(|marker| marker.invoice).collect();
    let resolved = settlements_from_history(endpoint, &invoices).await;
    let mut fresh = Vec::new();
    for marker in unresolved {
        match resolved.get(&marker.invoice) {
            Some(receipt) => {
                remember_settlement(endpoint, marker.invoice, receipt);
                fresh.push(receipt.clone());
            }
            None => payments.push(marker_record(
                marker,
                cluster.to_owned(),
                endpoint.to_owned(),
            )),
        }
    }
    store_settlements(db, endpoint, &fresh).await;
    payments.extend(
        fresh
            .into_iter()
            .map(|receipt| receipt_record(receipt, cluster.to_owned(), endpoint.to_owned())),
    );

    Ok(ProgramScan {
        payments,
        budgets,
        cluster: cluster.to_owned(),
        endpoint: endpoint.to_owned(),
    })
}

/// What one program-owned account turned out to be. The payment variant is
/// boxed because `PaymentRecord` carries every option, item and discount of an
/// invoice, and the scan builds one enum value per account.
enum ScannedAccount {
    Payment(Box<PaymentRecord>),
    /// A settlement's marker, unturned into a record. Resolving it is the
    /// caller's job, not this function's: it needs the cluster and the request's
    /// history budget, and this is a pure decoder.
    Settled(ReceiptMarker),
    Budget(BudgetAccount),
}

/// Sorts one account from `get_program_accounts` into what the scan keeps.
///
/// `None` is the normal answer for accounts this API has no use for — the fee
/// `Config` lives under the same program id — so an unknown or unreadable
/// account is skipped rather than failing the whole scan. A future account type
/// gets the same treatment for free.
fn classify_program_account(
    pubkey: Pubkey,
    data: &[u8],
    cluster: String,
    endpoint: String,
) -> Option<ScannedAccount> {
    if let Some(record) = record_for(pubkey, data, cluster, endpoint) {
        return Some(ScannedAccount::Payment(Box::new(record)));
    }
    if let Some(marker) = decode_receipt_marker(data) {
        // A settled invoice only exists as its marker, and the marker names the
        // invoice the scan addresses it by. Its payment facts are in the settle
        // transaction, which the scan reads within its history budget.
        return Some(ScannedAccount::Settled(marker));
    }
    decode_budget(data)
        .or_else(|| decode_token_budget(data))
        .map(ScannedAccount::Budget)
}

/// The payment half of a program scan (the admin feed needs nothing else).
pub async fn fetch_all_payments(
    history_reads: usize,
    db: &libsql::Database,
) -> ApiResult<AllPayments> {
    let scan = fetch_program_scan(history_reads, db).await?;
    Ok(AllPayments {
        records: scan.payments,
        cluster: scan.cluster,
        endpoint: scan.endpoint,
    })
}

/// Cluster used when a status request omits `?cluster=`: `SOLANA_CLUSTER`
/// from the shared `.env`, falling back to devnet.
pub(crate) fn default_cluster() -> String {
    std::env::var("SOLANA_CLUSTER").unwrap_or_else(|_| "devnet".to_string())
}

/// The reassuring half of a URL: scheme, host and port, with any path, query
/// or userinfo dropped. Comparing these is what makes the allowlist airtight —
/// `https://api.devnet.solana.com@evil.example` has a different origin than the
/// host it pretends to be.
fn origin_of(raw: &str) -> Option<String> {
    let parsed = url::Url::parse(raw.trim()).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    Some(parsed.origin().ascii_serialization())
}

/// Origins this API is willing to speak JSON-RPC to.
///
/// The cluster defaults, plus whatever the operator configured: `SOLANA_RPC_URL`
/// (the endpoint the API already trusts for its own reads) and
/// `ALLOWED_RPC_ENDPOINTS` (a comma-separated list, for a local validator on an
/// unusual port or a second provider). Nothing else.
fn allowed_origins() -> Vec<String> {
    let mut origins = vec![
        "https://api.devnet.solana.com".to_string(),
        "https://api.testnet.solana.com".to_string(),
        "https://api.mainnet-beta.solana.com".to_string(),
        "http://127.0.0.1:8899".to_string(),
    ];
    if let Ok(configured) = std::env::var("SOLANA_RPC_URL") {
        origins.extend(origin_of(&configured));
    }
    if let Ok(list) = std::env::var("ALLOWED_RPC_ENDPOINTS") {
        origins.extend(list.split(',').filter_map(origin_of));
    }
    origins
}

/// Explicit endpoint wins, then `SOLANA_RPC_URL`, then the cluster default.
///
/// The explicit one arrives inside a payment code written by a stranger, so it
/// is checked against the allowlist first: without that, `?endpoint=` would be
/// a request-forgery tool pointed at anything the function can reach — cloud
/// metadata, an internal admin port, a service on the same private network.
pub(crate) fn resolve_endpoint(endpoint: Option<&str>, cluster: &str) -> ApiResult<String> {
    if let Some(endpoint) = endpoint {
        let origin =
            origin_of(endpoint).ok_or_else(|| bad("endpoint must be an http(s) rpc url"))?;
        if !allowed_origins().contains(&origin) {
            return Err(bad(
                "This payment code points at an RPC server this API does not allow.",
            ));
        }
        return Ok(endpoint.trim().to_string());
    }
    if let Ok(url) = std::env::var("SOLANA_RPC_URL") {
        if !url.is_empty() {
            return Ok(url);
        }
    }
    match cluster {
        "devnet" => Ok("https://api.devnet.solana.com".to_string()),
        "testnet" => Ok("https://api.testnet.solana.com".to_string()),
        "mainnet" | "mainnet-beta" => Ok("https://api.mainnet-beta.solana.com".to_string()),
        "localnet" => Ok("http://127.0.0.1:8899".to_string()),
        other => Err(bad(format!(
            "unknown cluster {other:?}; add its RPC to ALLOWED_RPC_ENDPOINTS to use it"
        ))),
    }
}

struct OnchainPayment {
    seller: Pubkey,
    reference: Pubkey,
    amount: u64,
    memo: String,
    state: u8,
    payer: Pubkey,
    created_at: i64,
    settled_at: i64,
}

/// Borsh layout of the `Payment` account: 8 byte discriminator, then the
/// struct fields in declaration order.
fn decode_payment(data: &[u8]) -> Option<OnchainPayment> {
    let mut reader = Reader::new(data);

    if reader.take(8)? != account_discriminator() {
        return None;
    }

    Some(OnchainPayment {
        seller: reader.pubkey()?,
        reference: reader.pubkey()?,
        amount: reader.u64()?,
        memo: reader.string()?,
        state: reader.u8()?,
        payer: reader.pubkey()?,
        created_at: reader.i64()?,
        settled_at: reader.i64()?,
    })
}

struct OnchainTokenPayment {
    seller: Pubkey,
    reference: Pubkey,
    mint: Pubkey,
    amount: u64,
    memo: String,
    state: u8,
    payer: Pubkey,
    created_at: i64,
    settled_at: i64,
}

/// Borsh layout of the `TokenPayment` account: discriminator, then the fields
/// in declaration order — the mint sits between the reference and the amount.
fn decode_token_payment(data: &[u8]) -> Option<OnchainTokenPayment> {
    let mut reader = Reader::new(data);

    if reader.take(8)? != discriminator_for("TokenPayment") {
        return None;
    }

    Some(OnchainTokenPayment {
        seller: reader.pubkey()?,
        reference: reader.pubkey()?,
        mint: reader.pubkey()?,
        amount: reader.u64()?,
        memo: reader.string()?,
        state: reader.u8()?,
        payer: reader.pubkey()?,
        created_at: reader.i64()?,
        settled_at: reader.i64()?,
    })
}

/// The permanent `PaymentReceipt` account as it now exists: the invoice it
/// closed and the `settled_at` replay guard, nothing more. The payment's own
/// fields are in the `PaymentSettled` event the settle transaction logged, and
/// `settled_from_history` reads them from there.
#[derive(Clone, Copy)]
struct ReceiptMarker {
    invoice: Pubkey,
    settled_at: i64,
}

/// One settlement, assembled from its event. The field names are the
/// `PaymentReceipt` account's old ones on purpose: every consumer of a settled
/// invoice below already speaks this shape, and where the facts live is the
/// only thing that changed.
#[derive(Clone, Debug)]
struct OnchainReceipt {
    invoice: Pubkey,
    seller: Pubkey,
    reference: Pubkey,
    /// `Pubkey::default()` for a SOL invoice.
    mint: Pubkey,
    amount: u64,
    memo: String,
    payer: Pubkey,
    created_at: i64,
    settled_at: i64,
}

/// The marker's exact size: discriminator, invoice, `settled_at` — 48 bytes,
/// mirroring `PAYMENT_RECEIPT_SPACE` in the program.
const RECEIPT_MARKER_LEN: usize = 8 + 32 + 8;

/// Borsh layout of the `PaymentReceipt` account since revision 23:
/// discriminator, the invoice, and `settled_at`. It is a marker, not a record.
///
/// The length is checked rather than trusted. The pre-marker receipt carried the
/// same discriminator with the invoice at the same offset but a different store
/// of fields after it, so a 301-byte account read as a marker would report its
/// seller's first bytes as `settled_at`.
fn decode_receipt_marker(data: &[u8]) -> Option<ReceiptMarker> {
    if data.len() != RECEIPT_MARKER_LEN {
        return None;
    }
    let mut reader = Reader::new(data);

    if reader.take(8)? != discriminator_for("PaymentReceipt") {
        return None;
    }

    Some(ReceiptMarker {
        invoice: reader.pubkey()?,
        settled_at: reader.i64()?,
    })
}

/// Borsh body of the `PaymentSettled` event, as the program emits it:
/// `amount, fee, seller_proceeds, payer, seller, mint, reference, invoice, memo`.
fn decode_payment_settled(data: &[u8]) -> Option<OnchainReceipt> {
    let mut reader = Reader::new(data);

    if reader.take(8)? != event_discriminator_for("PaymentSettled") {
        return None;
    }

    let amount = reader.u64()?;
    reader.u64()?; // fee: the treasury's share, not part of a status record
    reader.u64()?; // seller_proceeds: what the seller keeps after the fee
    let payer = reader.pubkey()?;
    let seller = reader.pubkey()?;
    let mint = reader.pubkey()?;
    let reference = reader.pubkey()?;
    let invoice = reader.pubkey()?;
    let memo = reader.string()?;

    Some(OnchainReceipt {
        invoice,
        seller,
        reference,
        mint,
        amount,
        memo,
        payer,
        // Filled from the transaction's own block time by the caller.
        created_at: 0,
        settled_at: 0,
    })
}

/// One log line, if it is this program's `PaymentSettled` event. An event is a
/// single `Program data: <base64>` line; another program's logs and the
/// program's own `msg!` lines sit beside it.
fn settled_event_in_log(line: &str) -> Option<OnchainReceipt> {
    let body = line.strip_prefix("Program data: ")?;
    let bytes = STANDARD.decode(body).ok()?;
    decode_payment_settled(&bytes)
}

/// The settlement behind a receipt marker, read from the settle transaction's
/// own logs.
///
/// A program cannot read history, which is why the replay guard is an account.
/// The API can: the marker names the invoice, the invoice appears in the one
/// transaction that settled it, and that transaction logged `PaymentSettled`.
/// One `getSignaturesForAddress` and at most a few `getTransaction` calls per
/// settled invoice — the price of keeping the payment's 2,094,960 lamports of
/// rent off chain. `None` means the history was unreadable, and the caller falls
/// back to what the marker itself says.
/// Every settlement in at most two HTTP round trips: one batch asking each
/// invoice for its newest signature, one batch asking for those transactions.
///
/// Asking one invoice at a time is what made a cold history expensive: the
/// public cluster paces single calls hard, and the client retries a 429 with
/// backoff, so a page of twenty settlements spent tens of seconds waiting on a
/// rate limiter before answering — and often answered with nothing. A batch is
/// one request to a node that accepts it as such.
///
/// A missing entry means history did not answer for that invoice; the caller
/// falls back to the marker it already has.
async fn settlements_from_history(
    endpoint: &str,
    invoices: &[Pubkey],
) -> HashMap<Pubkey, OnchainReceipt> {
    let mut settled = HashMap::new();
    if invoices.is_empty() {
        return settled;
    }

    // The id is the invoice's own index, kept across both batches so a chunked
    // answer still says which invoice it belongs to.
    let calls = invoices
        .iter()
        .enumerate()
        .map(|(id, invoice)| {
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "getSignaturesForAddress",
                "params": [invoice.to_string(), {"limit": 5, "commitment": "confirmed"}],
            })
        })
        .collect();
    let mut signatures: HashMap<usize, (Pubkey, String)> = HashMap::new();
    for result in rpc_batch_all(endpoint, calls).await {
        let Some(id) = result.get("id").and_then(serde_json::Value::as_u64) else {
            continue;
        };
        let Some(invoice) = invoices.get(id as usize) else {
            continue;
        };
        let Some(signature) = result
            .get("result")
            .and_then(serde_json::Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("signature"))
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        signatures.insert(id as usize, (*invoice, signature.to_string()));
    }
    if signatures.is_empty() {
        return settled;
    }

    let mut ordered: Vec<(usize, (Pubkey, String))> = signatures.into_iter().collect();
    ordered.sort_by_key(|(id, _)| *id);

    let calls = ordered
        .iter()
        .map(|(id, (_, signature))| {
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "getTransaction",
                "params": [signature, {
                    "encoding": "json",
                    "commitment": "confirmed",
                    "maxSupportedTransactionVersion": 0,
                }],
            })
        })
        .collect();
    let results = rpc_batch_all(endpoint, calls).await;

    for result in results {
        let Some(id) = result.get("id").and_then(serde_json::Value::as_u64) else {
            continue;
        };
        let Some(invoice) = invoices.get(id as usize) else {
            continue;
        };
        let Some(transaction) = result.get("result").filter(|value| !value.is_null()) else {
            continue;
        };
        let Some(logs) = transaction
            .pointer("/meta/logMessages")
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };
        for line in logs.iter().filter_map(serde_json::Value::as_str) {
            let Some(receipt) = settled_event_in_log(line) else {
                continue;
            };
            if receipt.invoice != *invoice {
                continue;
            }
            // A transaction's block time is the only clock the event has; the
            // marker's own `settled_at` is written from it.
            let at = transaction
                .get("blockTime")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);
            settled.insert(
                *invoice,
                OnchainReceipt {
                    created_at: at,
                    settled_at: at,
                    ..receipt
                },
            );
            break;
        }
    }

    settled
}

/// One HTTP client for the batched history reads; building one per request
/// would pay the TLS handshake for every page.
static RPC_HTTP: OnceLock<Option<reqwest::Client>> = OnceLock::new();

fn rpc_http() -> Option<&'static reqwest::Client> {
    RPC_HTTP
        .get_or_init(|| {
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                // HTTP/1.1 on purpose: the public cluster answers a batch sent
                // over HTTP/2 with 429 while the same batch over HTTP/1.1 is
                // served, and single requests are unaffected either way.
                .http1_only()
                .build()
                .ok()
        })
        .as_ref()
}

/// Requests one batch may carry. The public cluster answers a bigger batch with
/// 429 rather than an error, so this is the size that keeps a page of history
/// from being refused outright.
const RPC_BATCH_LIMIT: usize = 10;

/// Posts every call, in batches the node accepts. A batch that fails costs only
/// its own invoices their detail; the rest still resolve.
async fn rpc_batch_all(endpoint: &str, calls: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    let mut results = Vec::with_capacity(calls.len());
    for batch in calls.chunks(RPC_BATCH_LIMIT) {
        if let Some(mut answered) = rpc_batch(endpoint, batch.to_vec()).await {
            results.append(&mut answered);
        }
    }
    results
}

/// Posts a JSON-RPC batch and returns its entries. One retry, because the
/// public cluster answers a batch it is pacing with 429 and a retry usually
/// lands after the window; anything else is a miss the caller already handles.
async fn rpc_batch(
    endpoint: &str,
    calls: Vec<serde_json::Value>,
) -> Option<Vec<serde_json::Value>> {
    let client = rpc_http()?;
    let body = serde_json::to_vec(&serde_json::Value::Array(calls)).ok()?;

    for attempt in 0..2 {
        let response = client
            .post(endpoint)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body.clone())
            .send()
            .await;
        let Ok(response) = response else {
            continue;
        };
        if response.status().is_success() {
            let text = response.text().await.ok()?;
            return serde_json::from_str::<serde_json::Value>(&text)
                .ok()?
                .as_array()
                .cloned();
        }
        tracing::info!(status = %response.status(), attempt, "settlement history batch was refused");
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    None
}

async fn settled_from_history(rpc: &RpcClient, invoice: Pubkey) -> Option<OnchainReceipt> {
    let signatures = rpc
        .get_signatures_for_address_with_config(
            &invoice,
            GetConfirmedSignaturesForAddress2Config {
                commitment: Some(CommitmentConfig::confirmed()),
                limit: Some(5),
                ..Default::default()
            },
        )
        .await
        .ok()?;

    for entry in signatures {
        let Ok(signature) = entry.signature.parse::<Signature>() else {
            continue;
        };
        let Ok(transaction) = rpc
            .get_transaction_with_config(
                &signature,
                RpcTransactionConfig {
                    encoding: Some(UiTransactionEncoding::Json),
                    commitment: Some(CommitmentConfig::confirmed()),
                    max_supported_transaction_version: Some(0),
                },
            )
            .await
        else {
            continue;
        };

        let logs = transaction
            .transaction
            .meta
            .and_then(|meta| Option::<Vec<String>>::from(meta.log_messages))
            .unwrap_or_default();
        for line in logs {
            let Some(settled) = settled_event_in_log(&line) else {
                continue;
            };
            if settled.invoice != invoice {
                continue;
            }
            let at = transaction.block_time.unwrap_or(0);
            return Some(OnchainReceipt {
                created_at: at,
                settled_at: at,
                ..settled
            });
        }
    }

    None
}

/// Most settle transactions one request may read from history.
///
/// History is the only expensive read this API does — two round trips per
/// settled invoice — so a request that finds many unknown settlements resolves
/// the newest few and answers the rest from their markers. The cache makes the
/// next poll cheaper instead of re-paying for the same invoices, so a large
/// cold programme fills in over a couple of polls rather than in one burst.
pub const MAX_HISTORY_READS: usize = 32;

/// How many more history reads a request may spend. Cache hits are free and
/// never consume it, so a warm isolate answers a whole page without reading
/// history at all.
pub struct HistoryBudget {
    remaining: usize,
}

impl HistoryBudget {
    /// `wanted` is what the caller can actually show (a page size, the number
    /// of watched addresses); the cap keeps one request bounded anyway.
    pub fn new(wanted: usize) -> Self {
        Self {
            remaining: wanted.min(MAX_HISTORY_READS),
        }
    }

    /// Claims one read, or says there are none left.
    fn take(&mut self) -> bool {
        if self.remaining == 0 {
            return false;
        }
        self.remaining -= 1;
        true
    }
}

/// Settlements are immutable: a marker and the transaction behind it are
/// written once, and nothing rewrites a settled invoice. So an isolate can keep
/// what it resolved and never ask again while it lives — which is what turns a
/// poll from "read history for every settled invoice" into "read history for
/// the one that was just paid".
///
/// Bounded by [`SETTLED_CACHE_LIMIT`] with oldest-first eviction: an isolate
/// that has seen a very long programme forgets its oldest settlements, and pays
/// for them again only if they are shown again.
const SETTLED_CACHE_LIMIT: usize = 4_096;

struct SettledCache {
    inner: Mutex<SettledCacheInner>,
}

struct SettledCacheInner {
    /// `(endpoint, invoice)` — the same invoice on two clusters is two
    /// settlements, so the endpoint is part of the key.
    entries: HashMap<(String, Pubkey), OnchainReceipt>,
    /// Insertion order, so the oldest entry is the one evicted.
    order: VecDeque<(String, Pubkey)>,
}

static SETTLED_CACHE: OnceLock<SettledCache> = OnceLock::new();

fn settled_cache() -> &'static SettledCache {
    SETTLED_CACHE.get_or_init(|| SettledCache {
        inner: Mutex::new(SettledCacheInner {
            entries: HashMap::new(),
            order: VecDeque::new(),
        }),
    })
}

/// A lock poisoned by a panicking request is still a usable map: recover the
/// data rather than turning one failed thread into a failing endpoint.
fn cached_settlement(endpoint: &str, invoice: &Pubkey) -> Option<OnchainReceipt> {
    let inner = settled_cache()
        .inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    inner.entries.get(&(endpoint.to_owned(), *invoice)).cloned()
}

/// Only successful reads are remembered: a failed one must be retried, or one
/// bad node would pin an invoice to its marker for the isolate's lifetime.
fn remember_settlement(endpoint: &str, invoice: Pubkey, settled: &OnchainReceipt) {
    let mut inner = settled_cache()
        .inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    remember_in(&mut inner, endpoint, invoice, settled);
}

/// The insertion itself, taking the map rather than reaching for the static
/// one, so the bound can be tested without filling the process-wide cache.
fn remember_in(
    inner: &mut SettledCacheInner,
    endpoint: &str,
    invoice: Pubkey,
    settled: &OnchainReceipt,
) {
    let key = (endpoint.to_owned(), invoice);
    if inner.entries.insert(key.clone(), settled.clone()).is_none() {
        inner.order.push_back(key);
    }
    while inner.entries.len() > SETTLED_CACHE_LIMIT {
        match inner.order.pop_front() {
            Some(oldest) => {
                inner.entries.remove(&oldest);
            }
            None => break,
        }
    }
}

/// How long an unsettled answer may be reused.
///
/// A pending invoice is the one status that changes, and it changes once. The
/// polling clients ask about the same address from several places at the same
/// time — the buyer's page, a second tab, the seller's screen watching the code
/// it printed — and each of those reads used to be its own request to the
/// cluster. Eight seconds spans a poll tick (`STATUS_POLL_MS` is five), so a
/// payment shows up within two polls at most — the checkout's own receipt read
/// covers the gap in between — and a single viewer's polls, not just concurrent
/// viewers of one invoice, share a cluster read.
const PENDING_TTL: Duration = Duration::from_secs(8);

/// Addresses one instance remembers an unsettled answer for, oldest-first: a
/// very busy day forgets what stopped being asked.
const PENDING_CACHE_LIMIT: usize = 4_096;

/// The last unsettled answer per `(endpoint, invoice)`, with when it was read.
///
/// Only unsettled records are kept. A settled one is final and lives in the
/// settled cache, which never expires; an answer that failed to read is not a
/// status at all and is never stored.
struct PendingCache {
    inner: Mutex<PendingCacheInner>,
}

#[derive(Default)]
struct PendingCacheInner {
    entries: HashMap<(String, Pubkey), (Instant, PaymentRecord)>,
    /// Insertion order, so the oldest entry is the one evicted.
    order: VecDeque<(String, Pubkey)>,
}

static PENDING_CACHE: OnceLock<PendingCache> = OnceLock::new();

fn pending_cache() -> &'static PendingCache {
    PENDING_CACHE.get_or_init(|| PendingCache {
        inner: Mutex::new(PendingCacheInner::default()),
    })
}

/// A recent unsettled answer, if one is still inside [`PENDING_TTL`].
fn cached_pending(endpoint: &str, invoice: &Pubkey) -> Option<PaymentRecord> {
    let mut inner = pending_cache()
        .inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cached_pending_in(&mut inner, endpoint, invoice, Instant::now())
}

/// The lookup itself, taking the map and the clock rather than reaching for the
/// statics, so the expiry can be tested without waiting for it.
fn cached_pending_in(
    inner: &mut PendingCacheInner,
    endpoint: &str,
    invoice: &Pubkey,
    now: Instant,
) -> Option<PaymentRecord> {
    let key = (endpoint.to_owned(), *invoice);
    let (read_at, record) = inner.entries.get(&key)?;
    if now.duration_since(*read_at) >= PENDING_TTL {
        inner.entries.remove(&key);
        return None;
    }
    Some(record.clone())
}

/// Remembers an unsettled answer. A settled record is refused here on purpose:
/// it belongs to the cache that never expires, and a copy with a deadline would
/// only make the next read slower.
fn remember_pending(endpoint: &str, invoice: Pubkey, record: &PaymentRecord) {
    let mut inner = pending_cache()
        .inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    remember_pending_in(&mut inner, endpoint, invoice, record, Instant::now());
}

fn remember_pending_in(
    inner: &mut PendingCacheInner,
    endpoint: &str,
    invoice: Pubkey,
    record: &PaymentRecord,
    now: Instant,
) {
    if record.status == PaymentStatus::Settled {
        return;
    }
    let key = (endpoint.to_owned(), invoice);
    if inner
        .entries
        .insert(key.clone(), (now, record.clone()))
        .is_none()
    {
        inner.order.push_back(key);
    }
    while inner.entries.len() > PENDING_CACHE_LIMIT {
        match inner.order.pop_front() {
            Some(oldest) => {
                inner.entries.remove(&oldest);
            }
            None => break,
        }
    }
}

/// How long a program scan may be reused.
///
/// The scan is one `getProgramAccounts` over every account the program owns, and
/// it runs when the feed has to fall back to the chain. That is the cold path,
/// but a page open in three tabs still asks three times in the same second, and
/// the admin list asks beside it. Eight seconds is short enough that an invoice
/// made just now is in the next scan and long enough that a burst is one read.
const SCAN_TTL: Duration = Duration::from_secs(8);

/// Scans one instance keeps: `(endpoint, cluster)`. One entry per cluster in
/// practice, bounded anyway.
const SCAN_CACHE_LIMIT: usize = 8;

/// One scan's accounts, with when they were read.
type ScannedEntry = (Instant, Vec<(Pubkey, Vec<u8>)>);

/// `(endpoint, cluster)` → the scan of it a moment ago.
type ScannedAccountsMap = HashMap<(String, String), ScannedEntry>;

/// The accounts a scan last returned, per `(endpoint, cluster)`.
///
/// Accounts, not records: the caller resolves settlements against the database
/// with its own history budget, and that must not be frozen for the window.
/// A failed scan is never stored — an error is not an answer.
struct ScannedAccounts {
    inner: Mutex<ScannedAccountsMap>,
}

static SCAN_CACHE: OnceLock<ScannedAccounts> = OnceLock::new();

fn scan_cache() -> &'static ScannedAccounts {
    SCAN_CACHE.get_or_init(|| ScannedAccounts {
        inner: Mutex::new(HashMap::new()),
    })
}

fn cached_scan(endpoint: &str, cluster: &str) -> Option<Vec<(Pubkey, Vec<u8>)>> {
    let mut inner = scan_cache()
        .inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cached_scan_in(&mut inner, endpoint, cluster, Instant::now())
}

fn cached_scan_in(
    inner: &mut ScannedAccountsMap,
    endpoint: &str,
    cluster: &str,
    now: Instant,
) -> Option<Vec<(Pubkey, Vec<u8>)>> {
    let key = (endpoint.to_owned(), cluster.to_owned());
    let (read_at, accounts) = inner.get(&key)?;
    if now.duration_since(*read_at) >= SCAN_TTL {
        inner.remove(&key);
        return None;
    }
    Some(accounts.clone())
}

fn remember_scan(endpoint: &str, cluster: &str, accounts: &[(Pubkey, Vec<u8>)]) {
    let mut inner = scan_cache()
        .inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    remember_scan_in(&mut inner, endpoint, cluster, accounts, Instant::now());
}

fn remember_scan_in(
    inner: &mut ScannedAccountsMap,
    endpoint: &str,
    cluster: &str,
    accounts: &[(Pubkey, Vec<u8>)],
    now: Instant,
) {
    // Expired entries are dropped here rather than swept on a timer: the map is
    // one entry per cluster, and this is the only place it grows.
    inner.retain(|_, (read_at, _)| now.duration_since(*read_at) < SCAN_TTL);
    inner.insert(
        (endpoint.to_owned(), cluster.to_owned()),
        (now, accounts.to_vec()),
    );
    while inner.len() > SCAN_CACHE_LIMIT {
        let Some(oldest) = inner
            .iter()
            .min_by_key(|(_, (read_at, _))| *read_at)
            .map(|(key, _)| key.clone())
        else {
            break;
        };
        inner.remove(&oldest);
    }
}

/// The settlement as the database remembers it, for an instance that has never
/// seen this invoice.
///
/// `None` covers both "not stored yet" and "the database did not answer": this
/// is a cache in front of the chain, so a miss or a hiccup falls through to
/// history rather than failing a status read.
async fn stored_settlement(
    db: &libsql::Database,
    endpoint: &str,
    invoice: &Pubkey,
) -> Option<OnchainReceipt> {
    let marker = ReceiptMarker {
        invoice: *invoice,
        settled_at: 0,
    };
    stored_settlements(db, endpoint, std::slice::from_ref(&marker))
        .await
        .remove(invoice)
}

/// The settlements the database already knows, for every marker in one read.
///
/// A scan used to ask the database once per settlement, which on a remote
/// database is a round trip each: with the chain reads behind them, that is
/// what made a cold history slow.
///
/// An empty map covers both "none stored yet" and "the database did not
/// answer": this is a cache in front of the chain, so a miss falls through to
/// history rather than failing the scan.
async fn stored_settlements(
    db: &libsql::Database,
    endpoint: &str,
    markers: &[ReceiptMarker],
) -> HashMap<Pubkey, OnchainReceipt> {
    let mut found = HashMap::new();
    if markers.is_empty() {
        return found;
    }
    let Ok(connection) = db.connect() else {
        return found;
    };

    let placeholders = (2..=markers.len() + 1)
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut params: Vec<libsql::Value> = Vec::with_capacity(markers.len() + 1);
    params.push(libsql::Value::Text(endpoint.to_owned()));
    params.extend(
        markers
            .iter()
            .map(|marker| libsql::Value::Text(marker.invoice.to_string())),
    );

    let read = connection
        .query(
            &format!(
                "SELECT invoice, seller, reference, mint, amount, memo, payer, created_at, settled_at \
                 FROM settled_payments WHERE endpoint = ?1 AND invoice IN ({placeholders})"
            ),
            params,
        )
        .await;

    let Ok(mut rows) = read else {
        return found;
    };
    while let Ok(Some(row)) = rows.next().await {
        let Ok(invoice) = row.get::<String>(0) else {
            continue;
        };
        let Ok(invoice) = Pubkey::from_str(&invoice) else {
            continue;
        };
        if let Some(receipt) = receipt_from_row(&row, invoice) {
            found.insert(invoice, receipt);
        }
    }
    found
}

/// One `settled_payments` row as the receipt it was written from.
fn receipt_from_row(row: &libsql::Row, invoice: Pubkey) -> Option<OnchainReceipt> {
    Some(OnchainReceipt {
        invoice,
        seller: Pubkey::from_str(&row.get::<String>(1).ok()?).ok()?,
        reference: Pubkey::from_str(&row.get::<String>(2).ok()?).ok()?,
        mint: Pubkey::from_str(&row.get::<String>(3).ok()?).ok()?,
        amount: u64::try_from(row.get::<i64>(4).ok()?).ok()?,
        memo: row.get::<String>(5).ok()?,
        payer: Pubkey::from_str(&row.get::<String>(6).ok()?).ok()?,
        created_at: row.get::<i64>(7).ok()?,
        settled_at: row.get::<i64>(8).ok()?,
    })
}

/// Remembers one settlement in the database, so the history read that produced
/// it happens once per invoice per cluster instead of once per cold instance.
///
/// Best effort on purpose: the settlement is already known and the caller has
/// its record. Failing the request because a cache write failed would be worse
/// than reading history one more time on a later poll.
async fn store_settlement(db: &libsql::Database, endpoint: &str, settled: &OnchainReceipt) {
    store_settlements(db, endpoint, std::slice::from_ref(settled)).await;
}

/// Remembers a batch of settlements in one statement, for the scan that reads
/// many at once. Same best-effort rule as one row at a time.
async fn store_settlements(db: &libsql::Database, endpoint: &str, settled: &[OnchainReceipt]) {
    if settled.is_empty() {
        return;
    }
    let Ok(connection) = db.connect() else {
        return;
    };

    let mut rows = Vec::with_capacity(settled.len());
    let mut params: Vec<libsql::Value> = Vec::with_capacity(settled.len() * 10);
    for (index, receipt) in settled.iter().enumerate() {
        let at = index * 10;
        rows.push(format!(
            "(?{}, ?{}, ?{}, ?{}, ?{}, ?{}, ?{}, ?{}, ?{}, ?{})",
            at + 1,
            at + 2,
            at + 3,
            at + 4,
            at + 5,
            at + 6,
            at + 7,
            at + 8,
            at + 9,
            at + 10
        ));
        params.extend([
            libsql::Value::Text(receipt.invoice.to_string()),
            libsql::Value::Text(endpoint.to_owned()),
            libsql::Value::Text(receipt.seller.to_string()),
            libsql::Value::Text(receipt.reference.to_string()),
            libsql::Value::Text(receipt.mint.to_string()),
            libsql::Value::Integer(receipt.amount as i64),
            libsql::Value::Text(receipt.memo.clone()),
            libsql::Value::Text(receipt.payer.to_string()),
            libsql::Value::Integer(receipt.created_at),
            libsql::Value::Integer(receipt.settled_at),
        ]);
    }

    let written = connection
        .execute(
            &format!(
                "INSERT INTO settled_payments \
                    (invoice, endpoint, seller, reference, mint, amount, memo, payer, created_at, settled_at) \
                 VALUES {} \
                 ON CONFLICT(invoice, endpoint) DO UPDATE SET \
                    seller = excluded.seller, reference = excluded.reference, \
                    mint = excluded.mint, amount = excluded.amount, \
                    memo = excluded.memo, payer = excluded.payer, \
                    created_at = excluded.created_at, settled_at = excluded.settled_at",
                rows.join(", ")
            ),
            params,
        )
        .await;
    if let Err(error) = written {
        tracing::info!(%error, "settlements were not cached");
    }
}

/// The record for a settled invoice: from this instance's memory, then from the
/// database that outlives it, then from history when this request still has a
/// read to spend, and otherwise from the marker alone.
async fn settled_record(
    rpc: &RpcClient,
    db: &libsql::Database,
    marker: ReceiptMarker,
    cluster: String,
    endpoint: String,
    budget: &mut HistoryBudget,
) -> PaymentRecord {
    if let Some(cached) = cached_settlement(&endpoint, &marker.invoice) {
        return receipt_record(cached, cluster, endpoint);
    }
    // The database row was written from the settle transaction once, so a
    // brand-new instance answers without touching history at all.
    if let Some(stored) = stored_settlement(db, &endpoint, &marker.invoice).await {
        remember_settlement(&endpoint, marker.invoice, &stored);
        return receipt_record(stored, cluster, endpoint);
    }
    if budget.take() {
        if let Some(settled) = settled_from_history(rpc, marker.invoice).await {
            store_settlement(db, &endpoint, &settled).await;
            remember_settlement(&endpoint, marker.invoice, &settled);
            return receipt_record(settled, cluster, endpoint);
        }
    }
    marker_record(marker, cluster, endpoint)
}

/// The record for a settled invoice whose settlement could not be read back:
/// the marker says "settled" and who it belongs to, which is the part the
/// cluster cannot lose. A status read still answers, it just cannot name the
/// payer or the price until the next attempt.
fn marker_record(marker: ReceiptMarker, cluster: String, endpoint: String) -> PaymentRecord {
    PaymentRecord {
        id: marker.invoice.to_string(),
        status: PaymentStatus::Settled,
        payment_pda: marker.invoice.to_string(),
        seller: String::new(),
        seller_user: None,
        reference: String::new(),
        amount_lamports: 0,
        options: Vec::new(),
        memo: String::new(),
        name: String::new(),
        description: String::new(),
        items: Vec::new(),
        discount: None,
        allow_discounts: false,
        cluster,
        endpoint,
        settled_by: None,
        created_at_unix: 0,
        settled_at_unix: marker.settled_at,
    }
}

/// The record for a settled invoice, built from its event. The marker outlives
/// the invoice account, so this is what a watcher sees after payment.
fn receipt_record(receipt: OnchainReceipt, cluster: String, endpoint: String) -> PaymentRecord {
    let invoice = receipt.invoice.to_string();
    let (currency, mint, decimals, amount_lamports) = if receipt.mint == Pubkey::default() {
        (
            SOL_CURRENCY.to_string(),
            String::new(),
            SOL_DECIMALS,
            receipt.amount,
        )
    } else {
        (
            currency_for_mint(&receipt.mint, &cluster)
                .unwrap_or("token")
                .to_string(),
            receipt.mint.to_string(),
            STABLE_DECIMALS,
            0,
        )
    };

    PaymentRecord {
        id: invoice.clone(),
        status: PaymentStatus::Settled,
        payment_pda: invoice.clone(),
        seller: receipt.seller.to_string(),
        seller_user: None,
        reference: receipt.reference.to_string(),
        amount_lamports,
        options: vec![PaymentOption {
            currency,
            mint,
            decimals,
            amount: receipt.amount,
            payment_pda: invoice,
        }],
        memo: receipt.memo,
        name: String::new(),
        description: String::new(),
        items: Vec::new(),
        discount: None,
        allow_discounts: false,
        cluster,
        endpoint,
        settled_by: (receipt.payer != Pubkey::default()).then(|| receipt.payer.to_string()),
        created_at_unix: receipt.created_at,
        settled_at_unix: receipt.settled_at,
    }
}

fn token_record_from_chain(
    pubkey: Pubkey,
    payment: OnchainTokenPayment,
    cluster: String,
    endpoint: String,
) -> PaymentRecord {
    let currency = currency_for_mint(&payment.mint, &cluster)
        .unwrap_or("token")
        .to_string();
    PaymentRecord {
        id: pubkey.to_string(),
        status: if payment.state == 1 {
            PaymentStatus::Settled
        } else {
            PaymentStatus::Pending
        },
        payment_pda: pubkey.to_string(),
        seller: payment.seller.to_string(),
        seller_user: None,
        reference: payment.reference.to_string(),
        // A token invoice charges no SOL; the option below carries the price.
        amount_lamports: 0,
        options: vec![PaymentOption {
            currency,
            mint: payment.mint.to_string(),
            decimals: STABLE_DECIMALS,
            amount: payment.amount,
            payment_pda: pubkey.to_string(),
        }],
        memo: payment.memo,
        name: String::new(),
        description: String::new(),
        items: Vec::new(),
        discount: None,
        allow_discounts: false,
        cluster,
        endpoint,
        settled_by: (payment.payer != Pubkey::default()).then(|| payment.payer.to_string()),
        created_at_unix: payment.created_at,
        settled_at_unix: payment.settled_at,
    }
}

/// `sha256("account:<Name>")[..8]`, the discriminator Anchor prefixes to
/// every `#[account]`.
fn account_discriminator() -> [u8; 8] {
    discriminator_for("Payment")
}

/// Discriminator of any Anchor account, for telling the program's account
/// kinds apart in a scan.
fn discriminator_for(name: &str) -> [u8; 8] {
    let digest = Sha256::digest(format!("account:{name}").as_bytes());
    digest[..8].try_into().expect("8 byte slice")
}

/// `sha256("event:<Name>")[..8]`, the prefix Anchor puts on an event's
/// `Program data:` line.
///
/// Events and accounts of the same name hash differently, and an event read
/// with the account prefix never matches: every settlement would decode to
/// `None`, which left settled invoices with no payer, price or seller — so a
/// fresh payment vanished from the history that had to name its owner.
fn event_discriminator_for(name: &str) -> [u8; 8] {
    let digest = Sha256::digest(format!("event:{name}").as_bytes());
    digest[..8].try_into().expect("8 byte slice")
}

/// Borsh layout of the `Budget` account: discriminator, buyer, session key,
/// then the allowance fields the chain enforces. Only the identities matter
/// to the API — the amounts are read from the chain by the client.
fn decode_budget(data: &[u8]) -> Option<BudgetAccount> {
    let mut reader = Reader::new(data);
    if reader.take(8)? != discriminator_for("Budget") {
        return None;
    }
    Some(BudgetAccount {
        buyer: reader.pubkey()?,
        session_key: reader.pubkey()?,
    })
}

/// A token allowance is a budget too: it holds the same buyer and session key at
/// the same offsets, with the mint after them. A payment made silently from a
/// USDC or EURC pot is signed by that session key, so a history that read only
/// the SOL pots could not show it at all.
fn decode_token_budget(data: &[u8]) -> Option<BudgetAccount> {
    let mut reader = Reader::new(data);
    if reader.take(8)? != discriminator_for("TokenBudget") {
        return None;
    }
    Some(BudgetAccount {
        buyer: reader.pubkey()?,
        session_key: reader.pubkey()?,
    })
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn take(&mut self, len: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(len)?;
        let slice = self.data.get(self.pos..end)?;
        self.pos = end;
        Some(slice)
    }

    fn pubkey(&mut self) -> Option<Pubkey> {
        let bytes: [u8; 32] = self.take(32)?.try_into().ok()?;
        Some(Pubkey::from(bytes))
    }

    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }

    fn i64(&mut self) -> Option<i64> {
        Some(i64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }

    fn string(&mut self) -> Option<String> {
        let len = usize::try_from(self.u32()?).ok()?;
        String::from_utf8(self.take(len)?.to_vec()).ok()
    }
}

pub(crate) fn parse_pubkey(value: &str, field: &str) -> ApiResult<Pubkey> {
    Pubkey::from_str(value).map_err(|_| bad(format!("{field} is not a valid solana address")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The endpoint travels inside a payment code, which is the shape a request
    /// forgery attempt takes: a stranger's QR pointing the API at whatever it
    /// can reach.
    #[test]
    fn refuses_an_endpoint_outside_the_allowlist() {
        for hostile in [
            "http://169.254.169.254/latest/meta-data",
            "http://127.0.0.1:5432",
            "https://api.devnet.solana.com@evil.example",
            "https://evil.example/",
            "ftp://api.devnet.solana.com",
            "file:///etc/passwd",
        ] {
            assert!(
                resolve_endpoint(Some(hostile), "devnet").is_err(),
                "{hostile} should not be fetched"
            );
        }
    }

    /// The checkout reads invoice state, not settlement history: `finalized`
    /// trails the `confirmed` a wallet waits for by a dozen seconds, and a paid
    /// invoice came back `pending` for exactly that long.
    #[test]
    fn invoice_reads_use_confirmed_commitment() {
        let rpc = payment_rpc("https://api.devnet.solana.com".into());
        assert_eq!(rpc.commitment(), CommitmentConfig::confirmed());
    }

    #[test]
    fn accepts_the_endpoints_the_operator_already_trusts() {
        assert_eq!(
            resolve_endpoint(Some("https://api.devnet.solana.com"), "devnet").unwrap(),
            "https://api.devnet.solana.com"
        );
        // A path and a port on the same origin are still that origin.
        assert!(resolve_endpoint(Some("http://127.0.0.1:8899/"), "localnet").is_ok());
        // With no endpoint in the code, the cluster decides.
        assert_eq!(
            resolve_endpoint(None, "devnet").unwrap(),
            "https://api.devnet.solana.com"
        );
    }

    /// An internal failure names a table, a query or a file. The caller gets a
    /// sentence instead; the detail goes to the log.
    #[tokio::test]
    async fn an_internal_error_keeps_its_detail_to_the_log() {
        use axum::response::IntoResponse;
        let response = ApiError::Internal("no such table: secret_ledger".into()).into_response();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .expect("body");
        let text = String::from_utf8(body.to_vec()).expect("utf8");
        assert!(!text.contains("secret_ledger"), "leaked: {text}");
        assert!(text.contains("Something went wrong"));
    }

    fn sample_request() -> CreatePaymentRequest {
        CreatePaymentRequest {
            payment_pda: "4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J".to_string(),
            seller: "4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J".to_string(),
            reference: "4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J".to_string(),
            amount_lamports: 1_000_000,
            options: Vec::new(),
            memo: "coffee".to_string(),
            name: None,
            description: None,
            items: Vec::new(),
            discount_code: None,
            allow_discounts: false,
            cluster: "devnet".to_string(),
            endpoint: None,
        }
    }

    #[test]
    fn accepts_a_valid_request() {
        let record = create_payment(sample_request()).expect("valid request");
        assert_eq!(record.status, PaymentStatus::Pending);
        assert_eq!(record.endpoint, "https://api.devnet.solana.com");
    }

    #[test]
    fn rejects_zero_amount() {
        let request = CreatePaymentRequest {
            amount_lamports: 0,
            ..sample_request()
        };
        assert!(create_payment(request).is_err());
    }

    #[test]
    fn keeps_a_name_and_description_for_the_buyer() {
        let request = CreatePaymentRequest {
            name: Some("  Coffee for two  ".to_string()),
            description: Some("Two flat whites at the corner shop".to_string()),
            ..sample_request()
        };
        let record = create_payment(request).expect("valid request");
        assert_eq!(record.name, "Coffee for two");
        assert_eq!(record.description, "Two flat whites at the corner shop");

        let payload: serde_json::Value =
            serde_json::from_str(&record.qr_payload()).expect("payload is json");
        assert_eq!(payload["name"], "Coffee for two");
        assert_eq!(payload["description"], "Two flat whites at the corner shop");
    }

    #[test]
    fn still_accepts_requests_without_a_name_or_description() {
        let request: CreatePaymentRequest = serde_json::from_value(serde_json::json!({
            "payment_pda": "4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J",
            "seller": "4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J",
            "reference": "4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J",
            "amount_lamports": 1_000_000,
            "memo": "coffee",
            "cluster": "devnet"
        }))
        .expect("an older client's request still deserializes");

        let record = create_payment(request).expect("valid request");
        assert!(record.name.is_empty());
        assert!(record.description.is_empty());
    }

    #[test]
    fn omits_an_empty_name_and_description_from_the_payload() {
        let record = create_payment(sample_request()).expect("valid request");
        let payload: serde_json::Value =
            serde_json::from_str(&record.qr_payload()).expect("payload is json");
        assert!(payload.get("name").is_none());
        assert!(payload.get("description").is_none());
        assert!(payload.get("discount").is_none());
    }

    #[test]
    fn carries_a_receipt_that_adds_up_to_the_price() {
        let request = CreatePaymentRequest {
            amount_lamports: 1_000_000,
            items: vec![
                PaymentItem {
                    name: " Flat white ".into(),
                    quantity: 2,
                    amount_lamports: 400_000,
                },
                PaymentItem {
                    name: "Croissant".into(),
                    quantity: 1,
                    amount_lamports: 200_000,
                },
            ],
            ..sample_request()
        };
        let record = create_payment(request).expect("valid receipt");
        assert_eq!(record.items.len(), 2);
        assert_eq!(record.items[0].name, "Flat white");

        let payload: serde_json::Value =
            serde_json::from_str(&record.qr_payload()).expect("payload is json");
        assert_eq!(payload["items"][0]["quantity"], 2);
        assert_eq!(payload["items"][1]["amount_lamports"], 200_000);
    }

    #[test]
    fn refuses_a_receipt_that_does_not_add_up() {
        // 2 × 0.004 SOL is not the 0.01 SOL the invoice charges.
        let request = CreatePaymentRequest {
            items: vec![PaymentItem {
                name: "Flat white".into(),
                quantity: 2,
                amount_lamports: 400_000,
            }],
            ..sample_request()
        };
        assert!(create_payment(request).is_err());
    }

    #[test]
    fn refuses_a_receipt_line_that_makes_no_sense() {
        let free = CreatePaymentRequest {
            items: vec![PaymentItem {
                name: "Coffee".into(),
                quantity: 0,
                amount_lamports: 1_000_000,
            }],
            ..sample_request()
        };
        assert!(create_payment(free).is_err());

        // One more line than a code can carry.
        let crowded = CreatePaymentRequest {
            amount_lamports: u64::from(MAX_ITEMS as u32 + 1),
            items: (0..=MAX_ITEMS)
                .map(|_| PaymentItem {
                    name: "x".into(),
                    quantity: 1,
                    amount_lamports: 1,
                })
                .collect(),
            ..sample_request()
        };
        assert!(create_payment(crowded).is_err());
    }

    #[test]
    fn carries_an_attached_discount_into_the_payload() {
        let mut record = create_payment(sample_request()).expect("valid request");
        record.discount = Some(PaymentDiscount {
            code: "SAVE10".to_string(),
            percent_off: 10,
        });

        let payload: serde_json::Value =
            serde_json::from_str(&record.qr_payload()).expect("payload is json");
        assert_eq!(payload["discount"]["code"], "SAVE10");
        assert_eq!(payload["discount"]["percent_off"], 10);
        // The invoice keeps its full price: the discount prices it, it does
        // not rewrite the amount the address was derived from.
        assert_eq!(payload["amount_lamports"], 1_000_000);
    }

    #[test]
    fn rejects_an_overlong_name_or_description() {
        let long_name = CreatePaymentRequest {
            name: Some("n".repeat(MAX_NAME_LEN + 1)),
            ..sample_request()
        };
        assert!(create_payment(long_name).is_err());

        let long_description = CreatePaymentRequest {
            description: Some("d".repeat(MAX_DESCRIPTION_LEN + 1)),
            ..sample_request()
        };
        assert!(create_payment(long_description).is_err());
    }

    #[test]
    fn rejects_unknown_cluster_without_endpoint() {
        let request = CreatePaymentRequest {
            cluster: "moon".to_string(),
            ..sample_request()
        };
        assert!(create_payment(request).is_err());
    }

    #[test]
    fn decodes_a_payment_account() {
        // discriminator + seller + reference + amount + memo + state + payer
        // + created_at + settled_at + bump
        let mut data = account_discriminator().to_vec();
        data.extend_from_slice(&[7u8; 32]); // seller
        data.extend_from_slice(&[9u8; 32]); // reference
        data.extend_from_slice(&42u64.to_le_bytes());
        data.extend_from_slice(&3u32.to_le_bytes());
        data.extend_from_slice(b"tip");
        data.push(0); // pending
        data.extend_from_slice(&[0u8; 32]); // payer
        data.extend_from_slice(&100i64.to_le_bytes());
        data.extend_from_slice(&0i64.to_le_bytes());

        let payment = decode_payment(&data).expect("decodes");
        assert_eq!(payment.amount, 42);
        assert_eq!(payment.memo, "tip");
        assert_eq!(payment.state, 0);
        assert_eq!(payment.created_at, 100);
    }

    #[test]
    fn rejects_a_foreign_account() {
        assert!(decode_payment(&[0u8; 100]).is_none());
    }

    #[test]
    fn prices_every_currency_the_seller_accepts() {
        let request = CreatePaymentRequest {
            amount_lamports: 0,
            options: vec![
                CreateOption {
                    currency: "sol".into(),
                    amount: 50_000_000,
                },
                CreateOption {
                    currency: "usdc".into(),
                    amount: 5_000_000,
                },
                CreateOption {
                    currency: "eurc".into(),
                    amount: 4_500_000,
                },
            ],
            ..sample_request()
        };
        let record = create_payment(request).expect("valid request");

        assert_eq!(record.options.len(), 3);
        assert_eq!(record.amount_lamports, 50_000_000);
        // SOL leads the payload: that is the shape older readers understand.
        assert_eq!(record.options[0].currency, "sol");
        assert_eq!(record.options[0].payment_pda, record.id);
        // Each currency gets its own address, so paying one cannot settle
        // another.
        let mut addresses = record
            .options
            .iter()
            .map(|option| option.payment_pda.clone())
            .collect::<Vec<_>>();
        addresses.sort();
        addresses.dedup();
        assert_eq!(addresses.len(), 3);

        let payload: serde_json::Value =
            serde_json::from_str(&record.qr_payload()).expect("payload is json");
        assert_eq!(payload["options"].as_array().unwrap().len(), 3);
        assert_eq!(payload["options"][1]["currency"], "usdc");
        assert_eq!(payload["options"][1]["decimals"], 6);
        assert_eq!(
            payload["options"][1]["mint"],
            "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU"
        );
    }

    #[test]
    fn a_sol_only_invoice_keeps_the_old_payload() {
        let record = create_payment(sample_request()).expect("valid request");
        assert_eq!(record.options.len(), 1);
        let payload: serde_json::Value =
            serde_json::from_str(&record.qr_payload()).expect("payload is json");
        assert!(payload.get("options").is_none());
        assert_eq!(payload["amount_lamports"], 1_000_000);
    }

    #[test]
    fn rejects_a_currency_twice_and_an_empty_invoice() {
        let twice = CreatePaymentRequest {
            amount_lamports: 0,
            options: vec![
                CreateOption {
                    currency: "usdc".into(),
                    amount: 1,
                },
                CreateOption {
                    currency: "USDC".into(),
                    amount: 2,
                },
            ],
            ..sample_request()
        };
        assert!(create_payment(twice).is_err());

        let nothing = CreatePaymentRequest {
            amount_lamports: 0,
            options: vec![],
            ..sample_request()
        };
        assert!(create_payment(nothing).is_err());
    }

    #[test]
    fn rejects_a_token_that_has_no_mint_on_this_cluster() {
        let request = CreatePaymentRequest {
            amount_lamports: 0,
            options: vec![CreateOption {
                currency: "dai".into(),
                amount: 1_000_000,
            }],
            ..sample_request()
        };
        assert!(create_payment(request).is_err());
    }

    /// The fee policy is a program-owned account shaped nothing like a
    /// payment. `get_program_accounts` returns it alongside invoices, so the
    /// scan has to skip it — an error here would take the whole payment list
    /// down the moment `Config` is initialized.
    fn config_data(session_fee_float: u64) -> Vec<u8> {
        let mut data = discriminator_for("Config").to_vec();
        data.extend_from_slice(&[6u8; 32]); // admin
        data.extend_from_slice(&100u16.to_le_bytes()); // fee_bps_below
        data.extend_from_slice(&50u16.to_le_bytes()); // fee_bps_above
        data.extend_from_slice(&100_000_000u64.to_le_bytes()); // sol_tier_lamports
        data.extend_from_slice(&10_000_000u64.to_le_bytes()); // token_tier_base_units
        data.extend_from_slice(&62_782u64.to_le_bytes()); // flat_usdc_base_units
        data.extend_from_slice(&57_346u64.to_le_bytes()); // flat_eurc_base_units
        data.extend_from_slice(&[4u8; 32]); // usdc_mint
        data.extend_from_slice(&[5u8; 32]); // eurc_mint
        data.extend_from_slice(&5_000u64.to_le_bytes()); // network_fee_lamports
        data.extend_from_slice(&351u64.to_le_bytes()); // network_fee_usdc_base_units
        data.extend_from_slice(&320u64.to_le_bytes()); // network_fee_eurc_base_units
        data.push(0); // fee_mode: SellerPays
        data.extend_from_slice(&session_fee_float.to_le_bytes());
        data.extend_from_slice(&1_760_000_000i64.to_le_bytes()); // flats_updated_at
        data.push(251); // bump
        data.push(250); // treasury_bump
        data.push(4); // version
        data
    }

    #[test]
    fn the_program_scan_skips_the_config_account() {
        let data = config_data(30_000_000);
        // 184 bytes, the shape the program allocates.
        assert_eq!(data.len(), 184);

        // No decoder mistakes it for something the scan keeps.
        assert!(decode_payment(&data).is_none());
        assert!(decode_token_payment(&data).is_none());
        assert!(decode_receipt_marker(&data).is_none());
        assert!(decode_budget(&data).is_none());

        // And classification returns “nothing to keep” rather than panicking.
        let classified = classify_program_account(
            Pubkey::from([8u8; 32]),
            &data,
            "devnet".into(),
            "https://api.devnet.solana.com".into(),
        );
        assert!(
            classified.is_none(),
            "Config must not become a payment or budget"
        );
    }

    /// The skip is not a blanket “ignore everything”: markers and budgets in
    /// the same scan still classify.
    #[test]
    fn the_program_scan_still_classifies_receipts_and_budgets() {
        let invoice = Pubkey::from([4u8; 32]);
        let marker = classify_program_account(
            receipt_pda(&Pubkey::from_str(PAYVAND_PROGRAM_ID).unwrap(), &invoice),
            &receipt_data(invoice, 200),
            "devnet".into(),
            "https://example".into(),
        );
        assert!(
            matches!(
                marker,
                Some(ScannedAccount::Settled(found))
                    if found.invoice == invoice && found.settled_at == 200
            ),
            "a marker must classify as a settlement of the invoice it names, keeping its own time"
        );

        let mut budget = discriminator_for("Budget").to_vec();
        budget.extend_from_slice(&[1u8; 32]); // buyer
        budget.extend_from_slice(&[2u8; 32]); // session key
        budget.extend_from_slice(&10u64.to_le_bytes());
        budget.extend_from_slice(&0u64.to_le_bytes());
        budget.extend_from_slice(&1u64.to_le_bytes());
        budget.push(255);
        let classified = classify_program_account(
            Pubkey::from([3u8; 32]),
            &budget,
            "devnet".into(),
            "https://example".into(),
        );
        assert!(matches!(classified, Some(ScannedAccount::Budget(_))));
    }

    /// A token allowance classifies too: a payment made silently from a USDC or
    /// EURC pot is signed by that session key, so without it the payment would
    /// belong to nobody the history can name.
    #[test]
    fn the_program_scan_classifies_token_budgets() {
        let mut budget = discriminator_for("TokenBudget").to_vec();
        budget.extend_from_slice(&[1u8; 32]); // buyer
        budget.extend_from_slice(&[2u8; 32]); // session key
        budget.extend_from_slice(&[3u8; 32]); // mint
        budget.extend_from_slice(&10u64.to_le_bytes());
        budget.extend_from_slice(&0u64.to_le_bytes());
        budget.extend_from_slice(&1u64.to_le_bytes());
        budget.push(255);

        match classify_program_account(
            Pubkey::from([3u8; 32]),
            &budget,
            "devnet".into(),
            "https://example".into(),
        ) {
            Some(ScannedAccount::Budget(found)) => {
                assert_eq!(found.buyer, Pubkey::from([1u8; 32]));
                assert_eq!(found.session_key, Pubkey::from([2u8; 32]));
            }
            _ => panic!("a token budget must classify as a budget"),
        }
    }

    #[test]
    fn decodes_a_token_payment_account() {
        let mut data = discriminator_for("TokenPayment").to_vec();
        data.extend_from_slice(&[7u8; 32]); // seller
        data.extend_from_slice(&[9u8; 32]); // reference
        data.extend_from_slice(&[3u8; 32]); // mint
        data.extend_from_slice(&42u64.to_le_bytes());
        data.extend_from_slice(&2u32.to_le_bytes());
        data.extend_from_slice(b"hi");
        data.push(1); // settled
        data.extend_from_slice(&[5u8; 32]); // payer
        data.extend_from_slice(&100i64.to_le_bytes());
        data.extend_from_slice(&200i64.to_le_bytes());

        let payment = decode_token_payment(&data).expect("decodes");
        assert_eq!(payment.amount, 42);
        assert_eq!(payment.state, 1);
        assert_eq!(payment.settled_at, 200);

        // A SOL invoice must not decode as a token invoice and vice versa.
        assert!(decode_payment(&data).is_none());
    }

    /// The pre-marker `PaymentReceipt` account — 301 bytes with a full-length
    /// memo. Same discriminator and same invoice offset as the marker, then
    /// seller, reference, mint, amount, memo, payer and three timestamps, so its
    /// `settled_at` is nowhere near the offset the marker decoder reads.
    fn old_receipt_data(invoice: Pubkey) -> Vec<u8> {
        let mut data = discriminator_for("PaymentReceipt").to_vec();
        data.extend_from_slice(invoice.as_ref());
        data.extend_from_slice(&[7u8; 32]); // seller
        data.extend_from_slice(&[9u8; 32]); // reference
        data.extend_from_slice(&[0u8; 32]); // mint: SOL
        data.extend_from_slice(&42u64.to_le_bytes()); // amount
        let memo = [b'm'; 96];
        data.extend_from_slice(&(memo.len() as u32).to_le_bytes());
        data.extend_from_slice(&memo);
        data.extend_from_slice(&[5u8; 32]); // payer
        data.extend_from_slice(&100i64.to_le_bytes()); // created_at
        data.extend_from_slice(&200i64.to_le_bytes()); // settled_at
        data.extend_from_slice(&300i64.to_le_bytes()); // closed_at
        data.push(255); // bump
        data
    }

    /// The marker as the program writes it: discriminator, invoice, `settled_at`.
    fn receipt_data(invoice: Pubkey, settled_at: i64) -> Vec<u8> {
        let mut data = discriminator_for("PaymentReceipt").to_vec();
        data.extend_from_slice(invoice.as_ref());
        data.extend_from_slice(&settled_at.to_le_bytes());
        data
    }

    /// The `PaymentSettled` log line a settle transaction carries, built in the
    /// program's field order: amount, fee, seller_proceeds, payer, seller, mint,
    /// reference, invoice, memo.
    fn settled_log_line(invoice: Pubkey, mint: Pubkey, amount: u64, memo: &str) -> String {
        let mut body = event_discriminator_for("PaymentSettled").to_vec();
        body.extend_from_slice(&amount.to_le_bytes());
        body.extend_from_slice(&10u64.to_le_bytes()); // fee
        body.extend_from_slice(&(amount - 10).to_le_bytes()); // seller_proceeds
        body.extend_from_slice(&[5u8; 32]); // payer
        body.extend_from_slice(&[7u8; 32]); // seller
        body.extend_from_slice(mint.as_ref());
        body.extend_from_slice(&[9u8; 32]); // reference
        body.extend_from_slice(invoice.as_ref());
        body.extend_from_slice(&(memo.len() as u32).to_le_bytes());
        body.extend_from_slice(memo.as_bytes());
        format!("Program data: {}", STANDARD.encode(body))
    }

    #[test]
    fn the_marker_is_only_the_guard_and_the_invoice() {
        let invoice = Pubkey::from([4u8; 32]);
        let data = receipt_data(invoice, 200);

        // 48 bytes is the whole account: the program's `PAYMENT_RECEIPT_SPACE`.
        assert_eq!(data.len(), 48);
        let marker = decode_receipt_marker(&data).expect("decodes");
        assert_eq!(marker.invoice, invoice);
        assert_eq!(marker.settled_at, 200);

        // The guard has to be able to say "already settled" on its own, because
        // that is the fallback when the history read fails.
        let record = marker_record(marker, "devnet".into(), "https://example".into());
        assert_eq!(record.status, PaymentStatus::Settled);
        assert_eq!(record.id, invoice.to_string());
        assert_eq!(record.settled_at_unix, 200);
        assert!(record.settled_by.is_none());
    }

    /// The IDL publishes the settle event's discriminator; this pins the bytes
    /// the cluster actually logs, so a prefix mix-up cannot hide every payment
    /// again.
    #[test]
    fn the_settle_event_discriminator_matches_the_idl() {
        assert_eq!(
            event_discriminator_for("PaymentSettled"),
            [158, 182, 152, 76, 105, 23, 232, 135]
        );
        assert_ne!(
            event_discriminator_for("PaymentSettled"),
            discriminator_for("PaymentSettled"),
            "an event and an account of the same name are prefixed differently"
        );
    }

    #[test]
    fn a_sol_settle_event_reads_back_as_a_settled_invoice() {
        let invoice = Pubkey::from([4u8; 32]);
        let line = settled_log_line(invoice, Pubkey::default(), 42, "tip");

        let settled = settled_event_in_log(&line).expect("decodes");
        let record = receipt_record(settled, "devnet".into(), "https://example".into());
        assert_eq!(record.status, PaymentStatus::Settled);
        // The invoice address is the identity, not the marker's own address.
        assert_eq!(record.id, invoice.to_string());
        assert_eq!(record.amount_lamports, 42);
        assert_eq!(record.options[0].currency, "sol");
        assert!(record.options[0].mint.is_empty());
        assert_eq!(record.options[0].amount, 42);
        assert_eq!(record.memo, "tip");
        assert_eq!(record.settled_by, Some(Pubkey::from([5u8; 32]).to_string()));
    }

    #[test]
    fn a_token_settle_event_reads_back_in_its_mint() {
        let invoice = Pubkey::from([4u8; 32]);
        let usdc = mint_for("usdc", "devnet").expect("usdc on devnet");
        let line = settled_log_line(invoice, usdc, 5_000_000, "flat white");

        let settled = settled_event_in_log(&line).expect("decodes");
        let record = receipt_record(settled, "devnet".into(), "https://example".into());
        assert_eq!(record.status, PaymentStatus::Settled);
        assert_eq!(record.amount_lamports, 0);
        assert_eq!(record.options[0].currency, "usdc");
        assert_eq!(record.options[0].amount, 5_000_000);
        assert_eq!(record.options[0].decimals, STABLE_DECIMALS);
        assert_eq!(record.options[0].mint, usdc.to_string());
    }

    /// A database on disk, so two connections see the same rows: an in-memory
    /// SQLite belongs to one connection, which is not what a cold instance and
    /// the instance that filled the cache before it look like.
    async fn temp_db() -> (libsql::Database, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "payvand-settlement-{}.db",
            crate::auth::random_id()
        ));
        let db = libsql::Builder::new_local(&path)
            .build()
            .await
            .expect("temp db");
        crate::db::migrate(&db).await.expect("schema");
        (db, path)
    }

    /// An invoice this API made is listed from its own row, with no cluster read
    /// at all: the seller's feed should not have to scan the program to show
    /// what it created.
    #[tokio::test]
    async fn an_invoice_we_created_is_listed_from_our_rows() {
        let (db, path) = temp_db().await;
        let invoice = Pubkey::from([0x51u8; 32]);
        let seller = Pubkey::from([0x52u8; 32]);
        let mut record = pending_record(invoice, "devnet".into(), "https://example".into());
        record.seller = seller.to_string();
        record.reference = Pubkey::from([0x53u8; 32]).to_string();
        record.memo = "flat white".into();
        record.name = "Invoice".into();
        record.options = vec![PaymentOption {
            currency: SOL_CURRENCY.to_string(),
            mint: String::new(),
            decimals: SOL_DECIMALS,
            amount: 4_500_000,
            payment_pda: invoice.to_string(),
        }];

        remember_invoice(&db, &record, Some("user-1")).await;

        let listed = recorded_invoices(&db, &record.seller, "devnet", 10).await;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, invoice.to_string());
        assert_eq!(listed[0].seller, record.seller);
        assert_eq!(listed[0].options[0].amount, 4_500_000);
        assert_eq!(listed[0].memo, "flat white");
        assert!(
            recorded_invoices(&db, "someone-else", "devnet", 10)
                .await
                .is_empty(),
            "another wallet's feed is not served this row"
        );
        assert!(
            recorded_invoices(&db, &record.seller, "mainnet", 10)
                .await
                .is_empty(),
            "a devnet invoice is not a mainnet one"
        );

        // The row carries the account that made it, for the pages that ask by
        // account rather than by wallet.
        let connection = db.connect().expect("connect");
        let mut rows = connection
            .query(
                "SELECT seller_user, amount FROM invoice_meta WHERE id = ?1",
                libsql::params![invoice.to_string()],
            )
            .await
            .expect("read");
        let row = rows.next().await.expect("row").expect("invoice");
        assert_eq!(row.get::<String>(0).expect("account"), "user-1");
        assert_eq!(row.get::<i64>(1).expect("amount"), 4_500_000);

        let _ = std::fs::remove_file(path);
    }

    /// A settled invoice this wallet paid is listed from the stored settlement:
    /// the payer is only knowable from the settle transaction, and it is already
    /// in the row history wrote.
    #[tokio::test]
    async fn settlements_this_wallet_paid_are_listed_from_the_db() {
        let (db, path) = temp_db().await;
        let buyer = Pubkey::from([0x61u8; 32]);
        let receipt = OnchainReceipt {
            invoice: Pubkey::from([0x62u8; 32]),
            seller: Pubkey::from([0x63u8; 32]),
            reference: Pubkey::from([0x64u8; 32]),
            mint: Pubkey::default(),
            amount: 2_000_000,
            memo: "coffee".into(),
            payer: buyer,
            created_at: 10,
            settled_at: 20,
        };
        let endpoint = "https://feed-test.invalid";
        store_settlement(&db, endpoint, &receipt).await;

        let paid = settlements_paid_by(&db, endpoint, "devnet", &buyer.to_string(), 10).await;
        assert_eq!(paid.len(), 1);
        assert_eq!(paid[0].id, receipt.invoice.to_string());
        assert_eq!(paid[0].status, PaymentStatus::Settled);
        assert_eq!(paid[0].amount_lamports, 2_000_000);
        assert_eq!(
            paid[0].settled_by.as_deref(),
            Some(buyer.to_string().as_str())
        );

        let stranger = Pubkey::from([0x65u8; 32]);
        assert!(
            settlements_paid_by(&db, endpoint, "devnet", &stranger.to_string(), 10)
                .await
                .is_empty(),
            "another wallet's payments are not in this feed"
        );
        assert!(
            settlements_paid_by(
                &db,
                "https://other.invalid",
                "devnet",
                &buyer.to_string(),
                10
            )
            .await
            .is_empty(),
            "a settlement belongs to the cluster it settled on"
        );

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn an_unpaid_invoice_is_pending_not_missing() {
        let (db, path) = temp_db().await;
        let invoice = Pubkey::from([4u8; 32]);
        let rpc = RpcClient::new("https://example.invalid".into());
        let mut reader =
            InvoiceReader::new(&rpc, &db, "devnet".into(), "https://example".into(), 1);
        let record = reader.record(invoice, None, None).await;
        assert_eq!(record.status, PaymentStatus::Pending);
        assert_eq!(record.payment_pda, invoice.to_string());
        assert_eq!(record.amount_lamports, 0);
        assert!(record.settled_by.is_none());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_marker_wins_over_a_leftover_invoice_account() {
        let invoice = Pubkey::from([4u8; 32]);

        // An old-program invoice account still sitting on chain. A marker in the
        // same read is the answer, whichever transaction left the invoice there:
        // the read branches on the marker before it looks at invoice data.
        let mut stale = account_discriminator().to_vec();
        stale.extend_from_slice(&[7u8; 32]); // seller
        stale.extend_from_slice(&[9u8; 32]); // reference
        stale.extend_from_slice(&42u64.to_le_bytes());
        stale.extend_from_slice(&3u32.to_le_bytes());
        stale.extend_from_slice(b"tip");
        stale.push(0); // pending
        stale.extend_from_slice(&[0u8; 32]); // payer
        stale.extend_from_slice(&100i64.to_le_bytes());
        stale.extend_from_slice(&0i64.to_le_bytes());
        assert!(record_for(invoice, &stale, "devnet".into(), "https://example".into()).is_some());

        let marker = decode_receipt_marker(&receipt_data(invoice, 200)).expect("decodes");
        let record = marker_record(marker, "devnet".into(), "https://example".into());
        assert_eq!(record.status, PaymentStatus::Settled);
        assert_eq!(record.settled_at_unix, 200);
    }

    /// A settled record reports no split, and that is a contract, not an
    /// omission: `0002` revision 15 records why. The event now read here carries
    /// `fee` and `seller_proceeds`, but publishing them would be a new field with
    /// a new meaning for every consumer of this record — a decision of its own,
    /// not a side effect of where the settlement is stored. A test forbids them
    /// appearing by accident, and pins the fields the record does promise.
    #[test]
    fn a_settled_record_claims_no_split() {
        let invoice = Pubkey::from([4u8; 32]);
        let line = settled_log_line(invoice, Pubkey::default(), 100_000_000, "tip");
        let settled = settled_event_in_log(&line).expect("decodes");
        let record = receipt_record(settled, "devnet".into(), "https://example".into());

        let json = serde_json::to_value(&record).expect("serializes");
        for absent in [
            "fee",
            "fee_lamports",
            "seller_proceeds",
            "receipt_rent_lamports",
        ] {
            assert!(
                json.get(absent).is_none(),
                "a settled record must not report {absent}"
            );
        }
        // What it does promise: the price it was settled at, and who signed.
        assert_eq!(json["amount_lamports"], 100_000_000);
        assert_eq!(json["status"], "settled");
        assert_eq!(json["settled_by"], Pubkey::from([5u8; 32]).to_string());
    }

    /// The whole point of the cache: a settled invoice already resolved is
    /// answered without touching history, so a poll repeated once a second is
    /// one account read and nothing else. The cluster here is unreachable, and
    /// the budget is empty, so only the cache can produce this record.
    #[tokio::test]
    async fn a_resolved_settlement_is_served_from_memory() {
        let (db, path) = temp_db().await;
        let endpoint = "https://cache-test-resolved.invalid";
        let invoice = Pubkey::from([0xB1u8; 32]);
        let line = settled_log_line(invoice, Pubkey::default(), 42, "tip");
        let settled = settled_event_in_log(&line).expect("decodes");
        remember_settlement(endpoint, invoice, &settled);

        let rpc = RpcClient::new("https://example.invalid".into());
        let mut budget = HistoryBudget::new(0);
        let record = settled_record(
            &rpc,
            &db,
            ReceiptMarker {
                invoice,
                settled_at: 200,
            },
            "devnet".into(),
            endpoint.into(),
            &mut budget,
        )
        .await;

        assert_eq!(record.status, PaymentStatus::Settled);
        assert_eq!(record.amount_lamports, 42);
        assert_eq!(record.memo, "tip");
        assert_eq!(record.settled_by, Some(Pubkey::from([5u8; 32]).to_string()));
        let _ = std::fs::remove_file(path);
    }

    /// When a request has no history read left, the marker still answers —
    /// settled, with the time it settled at — instead of the read stalling the
    /// whole page.
    #[tokio::test]
    async fn an_exhausted_history_budget_falls_back_to_the_marker() {
        let (db, path) = temp_db().await;
        let invoice = Pubkey::from([0xB2u8; 32]);
        let rpc = RpcClient::new("https://example.invalid".into());
        let mut budget = HistoryBudget::new(0);

        let record = settled_record(
            &rpc,
            &db,
            ReceiptMarker {
                invoice,
                settled_at: 200,
            },
            "devnet".into(),
            "https://cache-test-budget.invalid".into(),
            &mut budget,
        )
        .await;

        assert_eq!(record.status, PaymentStatus::Settled);
        assert_eq!(record.id, invoice.to_string());
        // The marker carries the settlement's time; a marker answer is not a
        // blank one.
        assert_eq!(record.settled_at_unix, 200);
        let _ = std::fs::remove_file(path);
    }

    /// The reason the cache moved into the database: a fresh instance has an
    /// empty memory, and the settlement it stored earlier still answers — with
    /// an unreachable cluster and no history read left, only that row can.
    #[tokio::test]
    async fn a_settlement_read_once_is_answered_from_the_database() {
        let (db, path) = temp_db().await;
        let endpoint = "https://settled-db-test.invalid";
        let invoice = Pubkey::from([0xC1u8; 32]);
        let line = settled_log_line(invoice, Pubkey::default(), 42, "tip");
        // What a successful history read hands on: the event plus the block time
        // the settle transaction carried.
        let settled = OnchainReceipt {
            created_at: 190,
            settled_at: 200,
            ..settled_event_in_log(&line).expect("decodes")
        };
        store_settlement(&db, endpoint, &settled).await;

        let rpc = RpcClient::new("https://example.invalid".into());
        let mut budget = HistoryBudget::new(0);
        let record = settled_record(
            &rpc,
            &db,
            ReceiptMarker {
                invoice,
                settled_at: 200,
            },
            "devnet".into(),
            endpoint.into(),
            &mut budget,
        )
        .await;

        assert_eq!(record.status, PaymentStatus::Settled);
        assert_eq!(record.amount_lamports, 42);
        assert_eq!(record.memo, "tip");
        assert_eq!(record.settled_by, Some(Pubkey::from([5u8; 32]).to_string()));
        assert_eq!(record.created_at_unix, 190);
        assert_eq!(record.settled_at_unix, 200);

        let _ = std::fs::remove_file(path);
    }

    /// A stand-in cluster: answers every JSON-RPC batch from `answers`, and
    /// counts the HTTP requests it received so a test can see the batching.
    async fn stub_rpc(
        answers: impl Fn(&serde_json::Value) -> serde_json::Value + Send + Sync + 'static,
    ) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = hits.clone();
        let answers = std::sync::Arc::new(answers);
        let app = axum::Router::new().route(
            "/",
            axum::routing::post(move |axum::Json(body): axum::Json<serde_json::Value>| {
                let answers = answers.clone();
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    // A batch is answered in kind; a single request — what
                    // `RpcClient` posts — gets its own object back, not an array.
                    let batch = body.as_array().cloned();
                    let calls = batch.clone().unwrap_or_else(|| vec![body]);
                    let answered: Vec<serde_json::Value> =
                        calls.iter().map(|call| (answers)(call)).collect();
                    axum::Json(match batch {
                        Some(_) => serde_json::Value::Array(answered),
                        None => answered
                            .into_iter()
                            .next()
                            .unwrap_or(serde_json::Value::Null),
                    })
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("address");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        (format!("http://{addr}/"), hits)
    }

    /// The batched read has to carry the invoice's own id through both requests
    /// and read the settle event out of the transaction's logs.
    #[tokio::test]
    async fn a_batch_history_read_returns_the_settle_event() {
        let invoice = Pubkey::from([0xB7u8; 32]);
        let line = settled_log_line(invoice, Pubkey::default(), 42, "tip");
        let signature = "5vxXkMhTVpHt4QkHVgcFj8itGev4a7trxRkj5Z3xs37".to_string();

        let (endpoint, _) = stub_rpc(move |call| {
            let id = call.get("id").cloned().unwrap_or_default();
            match call.get("method").and_then(serde_json::Value::as_str) {
                Some("getSignaturesForAddress") => serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": [{"signature": signature.clone()}],
                }),
                Some("getTransaction") => serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {"blockTime": 700, "meta": {"logMessages": [line.clone()]}},
                }),
                _ => serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {"code": -32601, "message": "no such method"},
                }),
            }
        })
        .await;

        let settled = settlements_from_history(&endpoint, &[invoice]).await;
        let receipt = settled
            .get(&invoice)
            .expect("the settle event was read back");
        assert_eq!(receipt.amount, 42);
        assert_eq!(receipt.memo, "tip");
        assert_eq!(receipt.payer, Pubkey::from([5u8; 32]));
        assert_eq!(receipt.settled_at, 700, "the transaction's block time");
    }

    /// The cluster refuses a batch bigger than ten, so twelve invoices cost two
    /// signature requests, and only the invoice that answered is asked about
    /// again — a per-item 429 must not fail the rest of the page.
    #[tokio::test]
    async fn history_batches_are_chunked_and_a_refused_item_does_not_fail_the_page() {
        let invoices: Vec<Pubkey> = (0..12u8)
            .map(|index| Pubkey::from([index + 1; 32]))
            .collect();

        let (endpoint, hits) = stub_rpc(|call| {
            let id = call.get("id").cloned().unwrap_or_default();
            let first = call.get("id").and_then(serde_json::Value::as_u64) == Some(0);
            match call.get("method").and_then(serde_json::Value::as_str) {
                Some("getSignaturesForAddress") if first => serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": [{"signature": "sig-0"}],
                }),
                Some("getSignaturesForAddress") => serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {"code": 429, "message": "Too many requests for a specific RPC call"},
                }),
                _ => serde_json::json!({"jsonrpc": "2.0", "id": id, "result": null}),
            }
        })
        .await;

        let settled = settlements_from_history(&endpoint, &invoices).await;
        assert!(settled.is_empty(), "no transaction came back");
        // Two signature chunks, then one transaction request for the one
        // invoice that answered.
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    /// The scan writes every settlement it resolved in one statement and reads
    /// them back in one, which is what keeps a cold history off a round trip per
    /// marker.
    #[tokio::test]
    async fn settlements_are_stored_and_read_in_one_batch() {
        let (db, path) = temp_db().await;
        let endpoint = "https://settled-batch.invalid";
        let first = settled_event_in_log(&settled_log_line(
            Pubkey::from([0xA1u8; 32]),
            Pubkey::default(),
            42,
            "tip",
        ))
        .expect("decodes");
        let second = settled_event_in_log(&settled_log_line(
            Pubkey::from([0xA2u8; 32]),
            Pubkey::default(),
            77,
            "flat white",
        ))
        .expect("decodes");

        store_settlements(&db, endpoint, &[first.clone(), second.clone()]).await;

        let markers = [
            ReceiptMarker {
                invoice: first.invoice,
                settled_at: 1,
            },
            ReceiptMarker {
                invoice: second.invoice,
                settled_at: 2,
            },
        ];
        let stored = stored_settlements(&db, endpoint, &markers).await;
        assert_eq!(stored.len(), 2);
        assert_eq!(stored[&first.invoice].amount, 42);
        assert_eq!(stored[&second.invoice].memo, "flat white");

        // A cluster the rows were not written for has nothing to answer with.
        assert!(stored_settlements(&db, "https://other.invalid", &markers)
            .await
            .is_empty());
        let _ = std::fs::remove_file(path);
    }

    /// Storing the same invoice twice updates the row instead of failing: a
    /// settlement is read once, but an operator re-reading one must not run into
    /// a primary-key error.
    #[tokio::test]
    async fn storing_a_settlement_twice_updates_it() {
        let (db, path) = temp_db().await;
        let endpoint = "https://settled-db-upsert.invalid";
        let invoice = Pubkey::from([0xC2u8; 32]);
        let line = settled_log_line(invoice, Pubkey::default(), 42, "tip");
        let settled = settled_event_in_log(&line).expect("decodes");
        store_settlement(&db, endpoint, &settled).await;
        store_settlement(
            &db,
            endpoint,
            &OnchainReceipt {
                memo: "flat white".into(),
                ..settled.clone()
            },
        )
        .await;

        let stored = stored_settlement(&db, endpoint, &invoice)
            .await
            .expect("stored");
        assert_eq!(stored.memo, "flat white");
        assert_eq!(stored.amount, 42);

        // Another cluster is another settlement, not the same row.
        assert!(stored_settlement(&db, "https://other.invalid", &invoice)
            .await
            .is_none());
        let _ = std::fs::remove_file(path);
    }

    /// A bounded cache, so a long-running instance cannot grow one entry per
    /// settlement ever made.
    #[test]
    fn the_settled_cache_evicts_its_oldest_entry() {
        let endpoint = "https://cache-test-bound.invalid";
        let mut inner = SettledCacheInner {
            entries: HashMap::new(),
            order: VecDeque::new(),
        };

        let mut invoices = Vec::new();
        for index in 0..(SETTLED_CACHE_LIMIT + 8) {
            let mut bytes = [0u8; 32];
            bytes[..8].copy_from_slice(&(index as u64).to_le_bytes());
            let invoice = Pubkey::from(bytes);
            let line = settled_log_line(invoice, Pubkey::default(), 1_000, "x");
            remember_in(
                &mut inner,
                endpoint,
                invoice,
                &settled_event_in_log(&line).expect("decodes"),
            );
            invoices.push(invoice);
        }

        let newest = *invoices.last().expect("inserted at least one");
        assert!(inner.entries.contains_key(&(endpoint.to_string(), newest)));
        assert!(
            !inner
                .entries
                .contains_key(&(endpoint.to_string(), invoices[0])),
            "the oldest settlement should have been evicted"
        );
        assert_eq!(inner.entries.len(), SETTLED_CACHE_LIMIT);
    }

    /// An unsettled answer is shared for one window and then asked for again.
    /// The length is the point: long enough that the buyer's page, a second tab
    /// and the seller's screen share one cluster read, short enough that a
    /// payment still appears within a poll or two.
    #[test]
    fn an_unsettled_answer_is_shared_until_its_window_ends() {
        let endpoint = "https://pending-test.invalid";
        let invoice = Pubkey::from([7u8; 32]);
        let record = pending_record(invoice, "devnet".into(), endpoint.into());
        let mut inner = PendingCacheInner::default();
        let start = Instant::now();

        assert!(
            cached_pending_in(&mut inner, endpoint, &invoice, start).is_none(),
            "nothing has been read yet"
        );

        remember_pending_in(&mut inner, endpoint, invoice, &record, start);
        let shared = cached_pending_in(
            &mut inner,
            endpoint,
            &invoice,
            start + Duration::from_secs(3),
        )
        .expect("a second viewer reads the first one's answer");
        assert_eq!(shared.id, invoice.to_string());
        assert!(
            cached_pending_in(&mut inner, endpoint, &invoice, start + PENDING_TTL).is_none(),
            "an expired answer is a miss, not a stale status"
        );
        assert!(
            inner.entries.is_empty(),
            "the expired entry is dropped where it is found"
        );
    }

    /// A settled record belongs to the cache that never expires; a copy with a
    /// deadline here would only make the next read slower.
    #[test]
    fn a_settled_record_is_not_kept_as_pending() {
        let endpoint = "https://pending-test-settled.invalid";
        let invoice = Pubkey::from([8u8; 32]);
        let mut record = pending_record(invoice, "devnet".into(), endpoint.into());
        record.status = PaymentStatus::Settled;
        let mut inner = PendingCacheInner::default();
        remember_pending_in(&mut inner, endpoint, invoice, &record, Instant::now());
        assert!(inner.entries.is_empty());
    }

    /// A cluster keeps its own answers: two networks can hold different states
    /// at the same address, so one must never answer for the other.
    #[test]
    fn the_pending_memo_is_keyed_by_endpoint() {
        let endpoint = "https://pending-test-a.invalid";
        let other = "https://pending-test-b.invalid";
        let invoice = Pubkey::from([9u8; 32]);
        let record = pending_record(invoice, "devnet".into(), endpoint.into());
        let mut inner = PendingCacheInner::default();
        let start = Instant::now();
        remember_pending_in(&mut inner, endpoint, invoice, &record, start);
        assert!(cached_pending_in(&mut inner, endpoint, &invoice, start).is_some());
        assert!(
            cached_pending_in(&mut inner, other, &invoice, start).is_none(),
            "another endpoint has not been asked yet"
        );
    }

    /// A `null` answer is a status — "not paid yet" — and is worth keeping for
    /// the window like any other unsettled one.
    #[test]
    fn an_unpaid_answer_is_kept_for_its_window() {
        let endpoint = "https://pending-test-null.invalid";
        let invoice = Pubkey::from([0xAu8; 32]);
        let record = pending_record(invoice, "devnet".into(), endpoint.into());
        assert_eq!(record.status, PaymentStatus::Pending);
        let mut inner = PendingCacheInner::default();
        let start = Instant::now();
        remember_pending_in(&mut inner, endpoint, invoice, &record, start);
        assert!(cached_pending_in(&mut inner, endpoint, &invoice, start).is_some());
    }

    /// The memo through the real read: the buyer's page and a second viewer of
    /// the same invoice cost one cluster request, not two.
    #[tokio::test]
    async fn a_second_viewer_is_answered_from_the_first_read() {
        let (db, path) = temp_db().await;
        let invoice = Pubkey::from([0x31u8; 32]);
        let (endpoint, hits) = stub_rpc(|call| {
            let id = call.get("id").cloned().unwrap_or_default();
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {"context": {"slot": 1}, "value": [null, null]},
            })
        })
        .await;

        let first = fetch_payment_at(&endpoint, "devnet", invoice, &db)
            .await
            .expect("first read");
        let second = fetch_payment_at(&endpoint, "devnet", invoice, &db)
            .await
            .expect("second read");
        assert_eq!(first.status, PaymentStatus::Pending);
        assert_eq!(second.id, first.id);
        assert_eq!(
            hits.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "the window answered the second viewer"
        );
        let _ = std::fs::remove_file(path);
    }

    /// A cluster that does not answer is an error, not "unpaid": nothing is
    /// remembered, and the next poll asks again rather than serving the failure
    /// as a status.
    #[tokio::test]
    async fn a_failed_read_is_never_remembered() {
        let (db, path) = temp_db().await;
        let invoice = Pubkey::from([0x32u8; 32]);
        let (endpoint, hits) = stub_rpc(|call| {
            let id = call.get("id").cloned().unwrap_or_default();
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": -32000, "message": "node is behind"},
            })
        })
        .await;

        assert!(fetch_payment_at(&endpoint, "devnet", invoice, &db)
            .await
            .is_err());
        assert!(fetch_payment_at(&endpoint, "devnet", invoice, &db)
            .await
            .is_err());
        assert!(
            hits.load(std::sync::atomic::Ordering::SeqCst) >= 2,
            "each attempt asked the cluster"
        );
        assert!(
            cached_pending(&endpoint, &invoice).is_none(),
            "a failure is not an answer to keep"
        );
        let _ = std::fs::remove_file(path);
    }

    /// A seller watching two addresses is one batch, and the next tick inside
    /// the window is no request at all.
    #[tokio::test]
    async fn a_watch_request_is_one_cluster_read_inside_the_window() {
        let (db, path) = temp_db().await;
        let pdas = vec![
            Pubkey::from([0x33u8; 32]).to_string(),
            Pubkey::from([0x34u8; 32]).to_string(),
        ];
        let (endpoint, hits) = stub_rpc(|call| {
            let id = call.get("id").cloned().unwrap_or_default();
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {"context": {"slot": 1}, "value": [null, null, null, null]},
            })
        })
        .await;

        let first = fetch_many_at(&endpoint, "devnet", &pdas, &db)
            .await
            .expect("first watch");
        let second = fetch_many_at(&endpoint, "devnet", &pdas, &db)
            .await
            .expect("second watch");
        assert_eq!(first.len(), 2);
        assert_eq!(second.len(), 2);
        assert_eq!(
            hits.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "two invoices over two polls: one batched read"
        );
        let _ = std::fs::remove_file(path);
    }

    /// A row from before amounts were kept is completed from its own account,
    /// once, and the seller's own words are left exactly as they were.
    #[tokio::test]
    async fn a_legacy_row_is_filled_from_the_chain_and_asked_about_once() {
        let (db, path) = temp_db().await;
        let invoice = Pubkey::from([0x71u8; 32]);
        let seller = Pubkey::from([0x72u8; 32]);
        let reference = Pubkey::from([0x73u8; 32]);

        let mut data = account_discriminator().to_vec();
        data.extend_from_slice(seller.as_ref());
        data.extend_from_slice(reference.as_ref());
        data.extend_from_slice(&4_500_000u64.to_le_bytes());
        data.extend_from_slice(&10u32.to_le_bytes());
        data.extend_from_slice(b"flat white");
        data.push(0); // pending
        data.extend_from_slice(&[0u8; 32]);
        data.extend_from_slice(&100i64.to_le_bytes());
        data.extend_from_slice(&0i64.to_le_bytes());
        let account = serde_json::json!({
            "data": [STANDARD.encode(&data), "base64"],
            "executable": false,
            "lamports": 1000,
            "owner": PAYVAND_PROGRAM_ID,
            "rentEpoch": 0,
        });
        let (endpoint, hits) = stub_rpc(move |call| {
            let id = call.get("id").cloned().unwrap_or_default();
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {"context": {"slot": 1}, "value": [account.clone(), null]},
            })
        })
        .await;

        let connection = db.connect().expect("connect");
        connection
            .execute(
                "INSERT INTO invoice_meta (id, seller, name, description, created_at) \
                 VALUES (?1, ?2, 'Flat white', 'Cardamom bun', 100)",
                libsql::params![invoice.to_string(), seller.to_string()],
            )
            .await
            .expect("a row from before amounts were kept");

        backfill_invoice_rows(&db, &seller.to_string(), "devnet", &endpoint).await;

        let mut rows = connection
            .query(
                "SELECT amount, currency, reference, cluster, endpoint, name, description \
                 FROM invoice_meta WHERE id = ?1",
                libsql::params![invoice.to_string()],
            )
            .await
            .expect("read");
        let row = rows.next().await.expect("row").expect("the row");
        assert_eq!(row.get::<i64>(0).expect("amount"), 4_500_000);
        assert_eq!(row.get::<String>(1).expect("currency"), "sol");
        assert_eq!(
            row.get::<String>(2).expect("reference"),
            reference.to_string()
        );
        assert_eq!(row.get::<String>(3).expect("cluster"), "devnet");
        assert_eq!(row.get::<String>(4).expect("endpoint"), endpoint);
        assert_eq!(
            row.get::<String>(5).expect("name"),
            "Flat white",
            "the seller's own words are not overwritten"
        );
        assert_eq!(row.get::<String>(6).expect("description"), "Cardamom bun");

        // And the row is now something the feed can list.
        let listed = recorded_invoices(&db, &seller.to_string(), "devnet", 10).await;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].amount_lamports, 4_500_000);

        let asked = hits.load(std::sync::atomic::Ordering::SeqCst);
        assert_eq!(asked, 1, "one batched read filled the page");
        backfill_invoice_rows(&db, &seller.to_string(), "devnet", &endpoint).await;
        assert_eq!(
            hits.load(std::sync::atomic::Ordering::SeqCst),
            asked,
            "a filled row is not asked about again"
        );
        let _ = std::fs::remove_file(path);
    }

    /// A row the chain knows nothing about — an invoice from another cluster, or
    /// one whose account is long gone — is asked about once and then left alone.
    #[tokio::test]
    async fn a_legacy_row_the_chain_does_not_know_is_not_asked_again() {
        let (db, path) = temp_db().await;
        let invoice = Pubkey::from([0x74u8; 32]);
        let seller = Pubkey::from([0x75u8; 32]);
        let (endpoint, hits) = stub_rpc(|call| {
            let id = call.get("id").cloned().unwrap_or_default();
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {"context": {"slot": 1}, "value": [null, null]},
            })
        })
        .await;

        let connection = db.connect().expect("connect");
        connection
            .execute(
                "INSERT INTO invoice_meta (id, seller, name, description, created_at) \
                 VALUES (?1, ?2, 'Flat white', '', 100)",
                libsql::params![invoice.to_string(), seller.to_string()],
            )
            .await
            .expect("legacy row");

        backfill_invoice_rows(&db, &seller.to_string(), "devnet", &endpoint).await;
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(
            recorded_invoices(&db, &seller.to_string(), "devnet", 10)
                .await
                .is_empty(),
            "nothing to list until the chain says something"
        );

        backfill_invoice_rows(&db, &seller.to_string(), "devnet", &endpoint).await;
        assert_eq!(
            hits.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "the next load does not ask about it again"
        );
        let _ = std::fs::remove_file(path);
    }

    /// A cluster that did not answer is not an answer: nothing is remembered as
    /// tried, so the next load asks again instead of leaving the row behind.
    #[tokio::test]
    async fn a_backfill_that_failed_is_tried_again() {
        let (db, path) = temp_db().await;
        let invoice = Pubkey::from([0x76u8; 32]);
        let seller = Pubkey::from([0x77u8; 32]);
        let (endpoint, hits) = stub_rpc(|call| {
            let id = call.get("id").cloned().unwrap_or_default();
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": -32005, "message": "node is unhealthy"},
            })
        })
        .await;

        let connection = db.connect().expect("connect");
        connection
            .execute(
                "INSERT INTO invoice_meta (id, seller, name, description, created_at) \
                 VALUES (?1, ?2, 'Flat white', '', 100)",
                libsql::params![invoice.to_string(), seller.to_string()],
            )
            .await
            .expect("legacy row");

        backfill_invoice_rows(&db, &seller.to_string(), "devnet", &endpoint).await;
        backfill_invoice_rows(&db, &seller.to_string(), "devnet", &endpoint).await;
        assert!(
            hits.load(std::sync::atomic::Ordering::SeqCst) >= 2,
            "each load asks again until the cluster answers"
        );

        let mut rows = connection
            .query("SELECT count(*) FROM invoice_backfill", ())
            .await
            .expect("read");
        let row = rows.next().await.expect("row").expect("count");
        assert_eq!(
            row.get::<i64>(0).expect("count"),
            0,
            "a failure is not remembered as an answer"
        );
        let _ = std::fs::remove_file(path);
    }

    /// A session key is kept against the wallet it spends for, and no other
    /// wallet, cluster or endpoint reads it back.
    #[tokio::test]
    async fn a_session_key_is_kept_for_the_wallet_it_spends_for() {
        let (db, path) = temp_db().await;
        let wallet = "wallet-a";
        let key = Pubkey::from([0x61u8; 32]);
        let endpoint = "https://allowance-a.invalid";
        assert!(
            session_keys_for(&db, wallet, "devnet", endpoint)
                .await
                .is_empty(),
            "nothing has been read yet"
        );

        remember_session_key(&db, &key.to_string(), wallet, "devnet", endpoint).await;
        assert_eq!(
            session_keys_for(&db, wallet, "devnet", endpoint).await,
            vec![key]
        );
        assert!(
            session_keys_for(&db, "wallet-b", "devnet", endpoint)
                .await
                .is_empty(),
            "another wallet has no key"
        );
        assert!(
            session_keys_for(&db, wallet, "mainnet-beta", endpoint)
                .await
                .is_empty(),
            "another cluster is another program"
        );
        let _ = std::fs::remove_file(path);
    }

    /// A payment signed by a session key is the wallet's once the key is known:
    /// the settle row is keyed by the payer, so the read asks for every key that
    /// stands for the buyer.
    #[tokio::test]
    async fn settlements_a_session_key_paid_are_listed_for_its_wallet() {
        let (db, path) = temp_db().await;
        let endpoint = "https://allowance-b.invalid";
        let wallet = Pubkey::from([0x62u8; 32]);
        let session_key = Pubkey::from([0x63u8; 32]);
        let receipt =
            |invoice: [u8; 32], payer: Pubkey, settled_at: i64, memo: &str| OnchainReceipt {
                invoice: Pubkey::from(invoice),
                seller: Pubkey::from([0x69u8; 32]),
                reference: Pubkey::from([0x6Au8; 32]),
                mint: Pubkey::default(),
                amount: 4_500_000,
                memo: memo.to_string(),
                payer,
                created_at: settled_at - 60,
                settled_at,
            };
        let direct = receipt([0x64u8; 32], wallet, 100, "direct");
        let silent = receipt([0x65u8; 32], session_key, 200, "silent");
        store_settlements(&db, endpoint, &[direct, silent]).await;

        let direct_only =
            settlements_paid_by(&db, endpoint, "devnet", &wallet.to_string(), 10).await;
        assert_eq!(direct_only.len(), 1, "the wallet's own payment");
        assert_eq!(direct_only[0].memo, "direct");

        let both = settlements_paid_by_any(
            &db,
            endpoint,
            "devnet",
            &[wallet.to_string(), session_key.to_string()],
            10,
        )
        .await;
        assert_eq!(both.len(), 2, "the allowance payment is the buyer's too");
        assert!(
            both.iter().any(|record| record.memo == "silent"),
            "the silent payment is in the page"
        );
        let _ = std::fs::remove_file(path);
    }

    /// One bounded read of the addresses a buyer's budgets live at is enough to
    /// learn its session key — the program is not scanned.
    #[tokio::test]
    async fn an_allowance_is_read_from_its_own_addresses() {
        let (db, path) = temp_db().await;
        let buyer = Pubkey::from([0x66u8; 32]);
        let session_key = Pubkey::from([0x67u8; 32]);
        let mut budget = discriminator_for("Budget").to_vec();
        budget.extend_from_slice(buyer.as_ref());
        budget.extend_from_slice(session_key.as_ref());
        budget.extend_from_slice(&10u64.to_le_bytes());
        budget.extend_from_slice(&0u64.to_le_bytes());
        budget.extend_from_slice(&1u64.to_le_bytes());
        budget.push(255);
        let account = serde_json::json!({
            "data": [STANDARD.encode(&budget), "base64"],
            "executable": false,
            "lamports": 1000,
            "owner": PAYVAND_PROGRAM_ID,
            "rentEpoch": 0,
        });
        let (endpoint, hits) = stub_rpc(move |call| {
            let id = call.get("id").cloned().unwrap_or_default();
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {"context": {"slot": 1}, "value": [account.clone(), null, null]},
            })
        })
        .await;

        let found = detect_allowance(&db, &endpoint, "devnet", &buyer)
            .await
            .expect("the cluster answered");
        assert_eq!(found, vec![session_key]);
        assert_eq!(
            hits.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "one read, whatever the buyer has"
        );
        assert_eq!(
            session_keys_for(&db, &buyer.to_string(), "devnet", &endpoint).await,
            vec![session_key],
            "and it is kept for the next load"
        );
        let _ = std::fs::remove_file(path);
    }

    /// A cluster that does not answer is not an answer: nothing is remembered,
    /// and the wallet can be looked up again.
    #[tokio::test]
    async fn an_allowance_lookup_that_failed_learns_nothing() {
        let (db, path) = temp_db().await;
        let buyer = Pubkey::from([0x68u8; 32]);
        let (endpoint, _) = stub_rpc(|call| {
            let id = call.get("id").cloned().unwrap_or_default();
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": -32005, "message": "node is unhealthy"},
            })
        })
        .await;

        assert!(detect_allowance(&db, &endpoint, "devnet", &buyer)
            .await
            .is_none());
        assert!(
            session_keys_for(&db, &buyer.to_string(), "devnet", &endpoint)
                .await
                .is_empty()
        );
        let _ = std::fs::remove_file(path);
    }

    /// The lookup is once a window, not once a load: a wallet with no allowance
    /// is remembered as checked, and looked at again when the window turns.
    #[tokio::test]
    async fn a_checked_wallet_is_not_looked_up_again_inside_the_window() {
        let (db, path) = temp_db().await;
        let wallet = "wallet-c";
        let endpoint = "https://allowance-c.invalid";
        assert!(
            allowance_check_due(&db, wallet, "devnet", endpoint).await,
            "never looked up"
        );
        mark_allowance_checked(&db, wallet, "devnet", endpoint).await;
        assert!(!allowance_check_due(&db, wallet, "devnet", endpoint).await);

        let connection = db.connect().expect("connect");
        connection
            .execute(
                "UPDATE allowance_checks SET checked_at = ?1 WHERE wallet = ?2",
                libsql::params![
                    crate::auth::unix_now() - ALLOWANCE_CHECK_SECS - 1,
                    wallet.to_string()
                ],
            )
            .await
            .expect("backdate");
        assert!(
            allowance_check_due(&db, wallet, "devnet", endpoint).await,
            "an old look is due again"
        );
        let _ = std::fs::remove_file(path);
    }

    /// The scan's window end to end: the second request in it is answered from
    /// the accounts the first one read.
    #[tokio::test]
    async fn a_scan_answers_the_next_request_without_asking_again() {
        let (db, path) = temp_db().await;
        let (endpoint, hits) = stub_rpc(|call| {
            let id = call.get("id").cloned().unwrap_or_default();
            serde_json::json!({"jsonrpc": "2.0", "id": id, "result": []})
        })
        .await;

        let first = program_scan_at(&endpoint, "devnet", 0, &db)
            .await
            .expect("first scan");
        let second = program_scan_at(&endpoint, "devnet", 0, &db)
            .await
            .expect("second scan");
        assert!(first.payments.is_empty());
        assert_eq!(second.cluster, "devnet");
        assert_eq!(
            hits.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "the window answered the second scan"
        );
        let _ = std::fs::remove_file(path);
    }

    /// A scan that failed is not a scan of an empty program: it is not kept, so
    /// the next request asks again instead of serving ``no payments'' for the
    /// window.
    #[tokio::test]
    async fn a_failed_scan_is_never_remembered() {
        let (db, path) = temp_db().await;
        let (endpoint, hits) = stub_rpc(|call| {
            let id = call.get("id").cloned().unwrap_or_default();
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": -32005, "message": "node is unhealthy"},
            })
        })
        .await;

        assert!(program_scan_at(&endpoint, "devnet", 0, &db).await.is_err());
        assert!(program_scan_at(&endpoint, "devnet", 0, &db).await.is_err());
        assert!(
            hits.load(std::sync::atomic::Ordering::SeqCst) >= 2,
            "each attempt asked the cluster"
        );
        assert!(
            cached_scan(&endpoint, "devnet").is_none(),
            "a failure is not a scan to keep"
        );
        let _ = std::fs::remove_file(path);
    }

    /// The scan's window in the map: shared inside it, dropped at its end, and
    /// never shared across a cluster or an endpoint.
    #[test]
    fn a_scan_is_shared_until_its_window_ends() {
        let mut inner = HashMap::new();
        let accounts = vec![(Pubkey::from([0x41u8; 32]), vec![1u8, 2, 3])];
        let start = Instant::now();
        assert!(
            cached_scan_in(&mut inner, "https://scan-test.invalid", "devnet", start).is_none(),
            "nothing has been scanned yet"
        );
        remember_scan_in(
            &mut inner,
            "https://scan-test.invalid",
            "devnet",
            &accounts,
            start,
        );
        let shared = cached_scan_in(
            &mut inner,
            "https://scan-test.invalid",
            "devnet",
            start + Duration::from_secs(3),
        )
        .expect("inside the window");
        assert_eq!(shared.len(), 1);
        assert!(
            cached_scan_in(
                &mut inner,
                "https://scan-test.invalid",
                "devnet",
                start + SCAN_TTL
            )
            .is_none(),
            "an expired scan is a miss"
        );
        assert!(
            inner.is_empty(),
            "the expired entry is dropped where it is found"
        );
    }

    #[test]
    fn scans_are_kept_per_cluster_and_endpoint() {
        let mut inner = HashMap::new();
        let accounts = vec![(Pubkey::from([0x42u8; 32]), vec![1u8])];
        let start = Instant::now();
        remember_scan_in(
            &mut inner,
            "https://scan-a.invalid",
            "devnet",
            &accounts,
            start,
        );
        assert!(
            cached_scan_in(&mut inner, "https://scan-a.invalid", "mainnet-beta", start).is_none(),
            "another cluster is another program state"
        );
        assert!(
            cached_scan_in(&mut inner, "https://scan-b.invalid", "devnet", start).is_none(),
            "another endpoint has not been asked yet"
        );
        assert_eq!(inner.len(), 1);
    }

    #[test]
    fn the_scan_cache_stays_bounded() {
        let mut inner = HashMap::new();
        let accounts = vec![(Pubkey::from([0x43u8; 32]), vec![1u8])];
        let start = Instant::now();
        for index in 0..(SCAN_CACHE_LIMIT + 3) {
            remember_scan_in(
                &mut inner,
                &format!("https://scan-{index}.invalid"),
                "devnet",
                &accounts,
                start + Duration::from_millis(index as u64),
            );
        }
        assert_eq!(inner.len(), SCAN_CACHE_LIMIT);
        assert!(
            cached_scan_in(&mut inner, "https://scan-0.invalid", "devnet", start).is_none(),
            "the oldest fell out"
        );
    }

    #[test]
    fn refuses_to_read_a_receipt_out_of_a_foreign_account() {
        assert!(decode_receipt_marker(&[0u8; 48]).is_none());
        // An invoice account is not a receipt either.
        let mut invoice = account_discriminator().to_vec();
        invoice.extend_from_slice(&[0u8; 48]);
        assert!(decode_receipt_marker(&invoice).is_none());
        // A receipt from the old program is a different layout under the same
        // discriminator: read as a marker, its seller would land in
        // `settled_at`. The length check is what refuses it.
        let old = old_receipt_data(Pubkey::from([4u8; 32]));
        assert_eq!(old.len(), 301);
        assert!(decode_receipt_marker(&old).is_none());

        // A truncated marker does not panic and does not decode.
        assert!(decode_receipt_marker(&receipt_data(Pubkey::from([4u8; 32]), 1)[..20]).is_none());
        // Neither does a longer account, which is what the exact length means.
        let mut longer = receipt_data(Pubkey::from([4u8; 32]), 1);
        longer.push(0);
        assert!(decode_receipt_marker(&longer).is_none());
    }
}
