//! Payment request validation, QR payload formatting and on-chain status reads.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use solana_pubkey::Pubkey;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use std::str::FromStr;
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
    Internal(String),
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = match &self {
            Self::BadRequest(_) => axum::http::StatusCode::BAD_REQUEST,
            Self::Unauthorized => axum::http::StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => axum::http::StatusCode::FORBIDDEN,
            Self::NotFound(_) => axum::http::StatusCode::NOT_FOUND,
            Self::Internal(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        };
        let body = axum::Json(serde_json::json!({ "error": self.to_string() }));
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
            options: if many { &self.options } else { &[] },
            cluster: &self.cluster,
            endpoint: &self.endpoint,
        })
        .expect("qr payload is serializable")
    }
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
        status: PaymentStatus::Missing,
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
        cluster: request.cluster,
        endpoint,
        settled_by: None,
        created_at_unix: 0,
        settled_at_unix: 0,
    })
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
            return Err(bad(format!(
                "the {currency} amount must be greater than 0"
            )));
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
            return Err(bad("The receipt lines do not add up to the invoice amount."));
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
pub async fn fetch_payment(payment_pda: &str, query: PaymentQuery) -> ApiResult<PaymentRecord> {
    let pubkey = parse_pubkey(payment_pda, "id")?;
    let cluster = query.cluster.clone().unwrap_or_else(default_cluster);
    let endpoint = resolve_endpoint(query.endpoint.as_deref(), &cluster)?;

    let rpc = RpcClient::new(endpoint.clone());
    let account = match rpc.get_account(&pubkey).await {
        Ok(account) => account,
        Err(error) => {
            tracing::info!(%payment_pda, %error, "payment account not readable");
            return Ok(empty_record(pubkey, cluster, endpoint));
        }
    };

    let Some(record) = record_for(pubkey, &account.data, cluster, endpoint) else {
        return Err(bad(format!(
            "{payment_pda} is not a payvand payment account"
        )));
    };

    Ok(record)
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

/// Placeholder returned while no payment account is readable for an address —
/// to every watcher it is an invoice that has not been paid yet.
fn empty_record(pubkey: Pubkey, cluster: String, endpoint: String) -> PaymentRecord {
    PaymentRecord {
        id: pubkey.to_string(),
        status: PaymentStatus::Missing,
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
        cluster,
        endpoint,
        settled_by: None,
        created_at_unix: 0,
        settled_at_unix: 0,
    }
}

/// The record for an address whose account decoded successfully.
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

/// Reads many payment accounts in a single RPC round trip so the seller's
/// screen can watch the invoice address and every discounted variant at once.
/// Addresses that are absent or unreadable come back as `Missing`, matching
/// `fetch_payment`'s tolerance: a cluster hiccup must not blank the seller's
/// screen.
pub async fn fetch_many(request: WatchRequest) -> ApiResult<Vec<PaymentRecord>> {
    if request.pdas.is_empty() {
        return Ok(Vec::new());
    }
    if request.pdas.len() > MAX_WATCH_PDAS {
        return Err(bad(format!(
            "at most {MAX_WATCH_PDAS} addresses per watch request"
        )));
    }

    let cluster = request.cluster.unwrap_or_else(default_cluster);
    let endpoint = resolve_endpoint(request.endpoint.as_deref(), &cluster)?;
    let pubkeys = request
        .pdas
        .iter()
        .map(|pda| parse_pubkey(pda, "pdas"))
        .collect::<ApiResult<Vec<_>>>()?;

    let rpc = RpcClient::new(endpoint.clone());
    let accounts = match rpc.get_multiple_accounts(&pubkeys).await {
        Ok(accounts) => accounts,
        Err(error) => {
            tracing::info!(%error, "payment accounts not readable");
            return Ok(pubkeys
                .iter()
                .map(|pubkey| empty_record(*pubkey, cluster.clone(), endpoint.clone()))
                .collect());
        }
    };

    Ok(pubkeys
        .iter()
        .zip(accounts)
        .map(|(pubkey, account)| match account
            .and_then(|account| record_for(*pubkey, &account.data, cluster.clone(), endpoint.clone()))
        {
            Some(record) => record,
            None => empty_record(*pubkey, cluster.clone(), endpoint.clone()),
        })
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
pub async fn fetch_program_scan() -> ApiResult<ProgramScan> {
    let cluster = default_cluster();
    let endpoint = resolve_endpoint(None, &cluster)?;
    let program_id = Pubkey::from_str(PAYVAND_PROGRAM_ID)
        .map_err(|error| ApiError::Internal(format!("bad program id: {error}")))?;

    let rpc = RpcClient::new(endpoint.clone());
    let accounts = match rpc.get_program_accounts(&program_id).await {
        Ok(accounts) => accounts,
        Err(error) => {
            tracing::info!(%error, "program accounts not readable");
            return Err(ApiError::Internal(
                "The cluster did not return the payment list. Try again.".into(),
            ));
        }
    };

    let mut payments = Vec::new();
    let mut budgets = Vec::new();
    for (pubkey, account) in accounts {
        if let Some(record) = record_for(pubkey, &account.data, cluster.clone(), endpoint.clone()) {
            payments.push(record);
        } else if let Some(budget) = decode_budget(&account.data) {
            budgets.push(budget);
        }
    }

    Ok(ProgramScan {
        payments,
        budgets,
        cluster,
        endpoint,
    })
}

/// The payment half of a program scan (the admin feed needs nothing else).
pub async fn fetch_all_payments() -> ApiResult<AllPayments> {
    let scan = fetch_program_scan().await?;
    Ok(AllPayments {
        records: scan.payments,
        cluster: scan.cluster,
        endpoint: scan.endpoint,
    })
}

/// Cluster used when a status request omits `?cluster=`: `SOLANA_CLUSTER`
/// from the shared `.env`, falling back to devnet.
fn default_cluster() -> String {
    std::env::var("SOLANA_CLUSTER").unwrap_or_else(|_| "devnet".to_string())
}

/// Explicit endpoint wins, then `SOLANA_RPC_URL`, then the cluster default.
pub(crate) fn resolve_endpoint(endpoint: Option<&str>, cluster: &str) -> ApiResult<String> {
    if let Some(endpoint) = endpoint {
        if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
            return Ok(endpoint.to_string());
        }
        return Err(bad("endpoint must be an http(s) rpc url"));
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
            "unknown cluster {other:?}; pass ?endpoint=https://... instead"
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

/// The record for an address whose token invoice decoded successfully.
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
            cluster: "devnet".to_string(),
            endpoint: None,
        }
    }

    #[test]
    fn accepts_a_valid_request() {
        let record = create_payment(sample_request()).expect("valid request");
        assert_eq!(record.status, PaymentStatus::Missing);
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
}
