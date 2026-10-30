//! Payment request validation, QR payload formatting and on-chain status reads.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use solana_pubkey::Pubkey;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use std::str::FromStr;
use thiserror::Error;

/// Mirrors `PAYMENT_MAX_MEMO` in `apps/contracts/programs/payvand`.
pub const MAX_MEMO_LEN: usize = 96;

/// Version tag carried in the QR payload so scanners can evolve the format.
pub const QR_PAYLOAD_VERSION: u8 = 1;

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
    pub amount_lamports: u64,
    pub memo: String,
    pub cluster: String,
    pub endpoint: String,
    pub settled_by: Option<String>,
    pub created_at_unix: i64,
    pub settled_at_unix: i64,
}

impl PaymentRecord {
    /// Compact, self-describing blob the seller encodes into the QR code.
    pub fn qr_payload(&self) -> String {
        serde_json::to_string(&QrPayload {
            v: QR_PAYLOAD_VERSION,
            id: &self.id,
            seller: &self.seller,
            // Lets the buyer's screen ask the API for this seller's codes.
            seller_user: self.seller_user.as_deref(),
            reference: &self.reference,
            amount_lamports: self.amount_lamports,
            memo: &self.memo,
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
    cluster: &'a str,
    endpoint: &'a str,
}

/// What the seller posts after its wallet has created the payment account.
#[derive(Debug, Deserialize)]
pub struct CreatePaymentRequest {
    pub payment_pda: String,
    pub seller: String,
    pub reference: String,
    pub amount_lamports: u64,
    pub memo: String,
    pub cluster: String,
    pub endpoint: Option<String>,
}

/// `?cluster=devnet` / `?endpoint=...` on `GET /api/payments/{id}`.
#[derive(Debug, Default, Deserialize)]
pub struct PaymentQuery {
    pub cluster: Option<String>,
    pub endpoint: Option<String>,
}

/// Validates an intent and returns the canonical record plus the QR payload.
pub fn create_payment(request: CreatePaymentRequest) -> ApiResult<PaymentRecord> {
    parse_pubkey(&request.payment_pda, "payment_pda")?;
    parse_pubkey(&request.seller, "seller")?;
    parse_pubkey(&request.reference, "reference")?;

    if request.amount_lamports == 0 {
        return Err(bad("amount_lamports must be greater than 0"));
    }
    if request.memo.len() > MAX_MEMO_LEN {
        return Err(bad(format!("memo must be at most {MAX_MEMO_LEN} bytes")));
    }

    let endpoint = resolve_endpoint(request.endpoint.as_deref(), &request.cluster)?;

    Ok(PaymentRecord {
        id: request.payment_pda.clone(),
        status: PaymentStatus::Missing,
        payment_pda: request.payment_pda,
        seller: request.seller,
        seller_user: None,
        reference: request.reference,
        amount_lamports: request.amount_lamports,
        memo: request.memo,
        cluster: request.cluster,
        endpoint,
        settled_by: None,
        created_at_unix: 0,
        settled_at_unix: 0,
    })
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

    let Some(payment) = decode_payment(&account.data) else {
        return Err(bad(format!(
            "{payment_pda} is not a payvand payment account"
        )));
    };

    Ok(record_from_chain(pubkey, payment, cluster, endpoint))
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
        memo: String::new(),
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
        memo: payment.memo,
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
        .map(|(pubkey, account)| match account.and_then(|account| decode_payment(&account.data)) {
            Some(payment) => record_from_chain(*pubkey, payment, cluster.clone(), endpoint.clone()),
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
        if let Some(payment) = decode_payment(&account.data) {
            payments.push(record_from_chain(
                pubkey,
                payment,
                cluster.clone(),
                endpoint.clone(),
            ));
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
fn resolve_endpoint(endpoint: Option<&str>, cluster: &str) -> ApiResult<String> {
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
            memo: "coffee".to_string(),
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
}
