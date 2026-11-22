//! The shops subscription: €1.70 for 30 days, payable in SOL, USDC or EURC.
//!
//! The price is a euro price, so only EURC can be charged at par. The other two
//! currencies are converted at a live rate fetched from a public price API and
//! cached for [`RATE_TTL_SECS`]: a quote is the amount the seller sees, and the
//! invoice created from it is the amount that must settle, so a rate that moves
//! between the two cannot change what was agreed.
//!
//! The payment itself is an ordinary invoice from the platform account, settled
//! by the deployed program exactly like a seller's invoice. Nothing is trusted
//! from the client: [`confirm`] asks the cluster whether the invoice was paid and
//! only then records the period.

use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use solana_pubkey::Pubkey;
use std::str::FromStr;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use crate::auth;
use crate::payments::{
    self, bad, ApiResult, CreateOption, CreatePaymentRequest, PaymentQuery, PaymentStatus,
};

/// €1.70, in cents: the whole price is kept in cents so no rounding creeps in.
pub const PLAN_PRICE_EUR_CENTS: u64 = 170;
/// A paid period. 30 days from the moment the payment settles, so a payment
/// never has to care what month it landed in.
pub const PERIOD_SECS: i64 = 30 * 24 * 60 * 60;
/// Six decimals on both stablecoins.
const TOKEN_DECIMALS: u32 = 6;
const LAMPORTS_PER_SOL: f64 = 1_000_000_000.0;
/// How long one rate read answers quotes. Long enough that a page of three
/// currencies asks once, short enough that a moved market shows up quickly.
pub const RATE_TTL_SECS: u64 = 300;
/// The public, keyless price feed.
const DEFAULT_RATES_URL: &str =
    "https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=eur,usd";

/// The currencies the plan accepts, in display order.
pub const CURRENCIES: [&str; 3] = ["sol", "usdc", "eurc"];

/// € per SOL and $ per €, as one read of the price feed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rates {
    /// EUR per 1 SOL.
    pub eur_per_sol: f64,
    /// USD per 1 EUR.
    pub usd_per_eur: f64,
    /// When this read happened, as a unix timestamp.
    pub fetched_at: i64,
}

impl Rates {
    fn sane(&self) -> bool {
        self.eur_per_sol.is_finite()
            && self.usd_per_eur.is_finite()
            && self.eur_per_sol > 0.0
            && self.usd_per_eur > 0.0
    }
}

/// One currency's price for the plan.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Quote {
    pub currency: String,
    /// Base units: lamports for SOL, the mint's units for a stablecoin.
    pub amount: u64,
    /// The mint the amount is paid in, or null for SOL.
    pub mint: Option<String>,
}

/// Rounds up: a subscription that underpayed by one base unit would not settle.
fn ceil_units(value: f64) -> u64 {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    let rounded = value.ceil();
    if rounded >= u64::MAX as f64 {
        u64::MAX
    } else {
        rounded as u64
    }
}

/// The three prices for one rate read.
///
/// EURC is the plan's own currency and pays at par. USDC is a dollar price, so it
/// takes the euro→dollar rate. SOL takes the euro→SOL rate, and both round up so
/// the euro price is covered rather than shaved by a base unit.
pub fn quotes_for(rates: &Rates, cluster: &str) -> Vec<Quote> {
    let eur = PLAN_PRICE_EUR_CENTS as f64 / 100.0;
    let token_scale = 10f64.powi(TOKEN_DECIMALS as i32);
    let usdc = ceil_units(eur * rates.usd_per_eur * token_scale);
    let eurc = ceil_units(eur * token_scale);
    let sol = ceil_units(eur / rates.eur_per_sol * LAMPORTS_PER_SOL);

    CURRENCIES
        .iter()
        .map(|&currency| Quote {
            currency: currency.to_string(),
            amount: match currency {
                "sol" => sol,
                "usdc" => usdc,
                _ => eurc,
            },
            mint: payments::mint_for(currency, cluster).map(|mint| mint.to_string()),
        })
        .collect()
}

fn rates_cache() -> &'static Mutex<Option<(Instant, Rates)>> {
    static CACHE: OnceLock<Mutex<Option<(Instant, Rates)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

fn rates_url() -> String {
    std::env::var("SUBSCRIPTION_RATES_URL")
        .ok()
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| DEFAULT_RATES_URL.to_string())
}

/// The feed's two numbers, read from its `{"solana":{"eur":…,"usd":…}}` answer.
fn parse_rates(body: &str) -> Option<(f64, f64)> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let solana = value.get("solana")?;
    let eur = solana.get("eur")?.as_f64()?;
    let usd = solana.get("usd")?.as_f64()?;
    if eur <= 0.0 || usd <= 0.0 {
        return None;
    }
    Some((eur, usd / eur))
}

/// The live rate, cached for [`RATE_TTL_SECS`]. A failed read never invents a
/// price: the quote is refused and the caller can retry.
pub async fn rates() -> ApiResult<Rates> {
    let mut guard = rates_cache().lock().await;
    if let Some((read_at, rates)) = *guard {
        if read_at.elapsed() < Duration::from_secs(RATE_TTL_SECS) {
            return Ok(rates);
        }
    }

    let response = reqwest::get(rates_url()).await.map_err(|error| {
        tracing::error!(%error, "subscription rate read failed");
        bad("The exchange rate is unavailable right now. Try again in a moment.")
    })?;
    if !response.status().is_success() {
        return Err(bad(
            "The exchange rate is unavailable right now. Try again in a moment.",
        ));
    }
    let body = response.text().await.map_err(|error| {
        tracing::error!(%error, "subscription rate body unreadable");
        bad("The exchange rate is unavailable right now. Try again in a moment.")
    })?;
    let (eur_per_sol, usd_per_eur) = parse_rates(&body).ok_or_else(|| {
        tracing::error!(%body, "subscription rate response in an unknown shape");
        bad("The exchange rate is unavailable right now. Try again in a moment.")
    })?;

    let rates = Rates {
        eur_per_sol,
        usd_per_eur,
        fetched_at: auth::unix_now(),
    };
    if !rates.sane() {
        return Err(bad(
            "The exchange rate is unavailable right now. Try again in a moment.",
        ));
    }
    *guard = Some((Instant::now(), rates));
    Ok(rates)
}

fn cluster() -> String {
    std::env::var("SOLANA_CLUSTER").unwrap_or_else(|_| "devnet".to_string())
}

/// The account that sells subscriptions: the platform's own wallet, which
/// receives the price through the program like any seller.
pub fn platform_seller() -> ApiResult<Pubkey> {
    let raw = std::env::var("PLATFORM_SELLER").map_err(|_| {
        bad("Subscriptions are not configured on this server yet (PLATFORM_SELLER).")
    })?;
    Pubkey::from_str(raw.trim())
        .map_err(|_| bad("Subscriptions are not configured correctly (PLATFORM_SELLER)."))
}

/// A reference for one subscription invoice: 32 fresh bytes, so two payments for
/// the same account never collide on their seeds.
fn new_reference(user_id: &str) -> Pubkey {
    let mut hasher = Sha256::new();
    hasher.update(user_id.as_bytes());
    hasher.update(auth::random_id().as_bytes());
    hasher.update(auth::unix_now().to_le_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&digest);
    Pubkey::new_from_array(bytes)
}

/// The subscription as one account sees it.
#[derive(Debug, Serialize)]
pub struct SubscriptionStatus {
    pub active: bool,
    /// Unix timestamp of the end of the paid period; 0 when none was ever paid.
    pub expires_at_unix: i64,
    pub currency: Option<String>,
    pub amount: Option<u64>,
    pub price_eur_cents: u64,
    pub period_days: i64,
    /// The plan's price in each accepted currency, converted at `rated_at_unix`.
    pub quotes: Vec<Quote>,
    pub rated_at_unix: i64,
}

async fn latest(
    state: &crate::AppState,
    user_id: &str,
) -> ApiResult<(bool, i64, Option<String>, Option<u64>)> {
    let connection = state.db.connect().map_err(auth::internal)?;
    let mut rows = connection
        .query(
            "SELECT currency, amount, expires_at FROM subscriptions \
             WHERE user_id = ?1 ORDER BY expires_at DESC LIMIT 1",
            [user_id.to_string()],
        )
        .await
        .map_err(auth::internal)?;
    match rows.next().await.map_err(auth::internal)? {
        Some(row) => {
            let currency: String = row.get(0).map_err(auth::internal)?;
            let amount: i64 = row.get(1).map_err(auth::internal)?;
            let expires_at: i64 = row.get(2).map_err(auth::internal)?;
            Ok((
                expires_at > auth::unix_now(),
                expires_at,
                Some(currency),
                u64::try_from(amount).ok(),
            ))
        }
        None => Ok((false, 0, None, None)),
    }
}

/// `GET /api/subscription` — the account's period and the plan's three prices.
pub async fn status(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<SubscriptionStatus>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let (active, expires_at_unix, currency, amount) = latest(&state, &user.id).await?;
    let cluster = cluster();
    let rates = rates().await?;
    Ok(Json(SubscriptionStatus {
        active,
        expires_at_unix,
        currency,
        amount,
        price_eur_cents: PLAN_PRICE_EUR_CENTS,
        period_days: PERIOD_SECS / (24 * 60 * 60),
        quotes: quotes_for(&rates, &cluster),
        rated_at_unix: rates.fetched_at,
    }))
}

/// Mutations a lapsed subscription refuses.
pub async fn require_active(state: &crate::AppState, user_id: &str) -> ApiResult<()> {
    let (active, _, _, _) = latest(state, user_id).await?;
    if active {
        return Ok(());
    }
    Err(crate::payments::ApiError::Forbidden(
        "The shops feature needs an active subscription (€1.70 for 30 days).".into(),
    ))
}

#[derive(Debug, Deserialize)]
pub struct InvoiceRequest {
    pub currency: String,
}

/// What the seller's screen needs to pay: the invoice code and the id to confirm.
#[derive(Debug, Serialize)]
pub struct InvoiceResponse {
    pub payment_id: String,
    pub payload: String,
    pub currency: String,
    pub amount: u64,
    pub expires_at_unix: i64,
}

/// `POST /api/subscription/invoice` — one invoice for the plan, in the currency
/// the seller picked, priced at the quote of the moment. The row it writes is
/// what `confirm` checks the payment against, so a settled payment for another
/// amount cannot buy a period.
pub async fn create_invoice(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Json(request): Json<InvoiceRequest>,
) -> ApiResult<Json<InvoiceResponse>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let cluster = cluster();
    let seller = platform_seller()?;
    let rates = rates().await?;
    let quote = quotes_for(&rates, &cluster)
        .into_iter()
        .find(|quote| quote.currency == request.currency.to_ascii_lowercase())
        .ok_or_else(|| bad("Pick one of sol, usdc or eurc."))?;
    if quote.amount == 0 {
        return Err(bad("That currency has no price right now. Try another."));
    }

    let record = payments::create_payment(CreatePaymentRequest {
        payment_pda: String::new(),
        seller: seller.to_string(),
        reference: new_reference(&user.id).to_string(),
        amount_lamports: 0,
        options: vec![CreateOption {
            currency: quote.currency.clone(),
            amount: quote.amount,
        }],
        memo: "Shops subscription, 30 days".to_string(),
        name: Some("Shops subscription".to_string()),
        description: Some("€1.70 for 30 days of shops and products.".to_string()),
        items: Vec::new(),
        discount_code: None,
        allow_discounts: false,
        cluster: cluster.clone(),
        endpoint: None,
    })?;

    let connection = state.db.connect().map_err(auth::internal)?;
    connection
        .execute(
            "INSERT INTO subscription_invoices (payment_id, user_id, currency, amount, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (
                record.id.clone(),
                user.id.clone(),
                quote.currency.clone(),
                i64::try_from(quote.amount).map_err(|_| bad("That price is too large."))?,
                auth::unix_now(),
            ),
        )
        .await
        .map_err(auth::internal)?;

    // Built before `id` moves out of the record.
    let payload = record.qr_payload();
    Ok(Json(InvoiceResponse {
        payment_id: record.id,
        payload,
        currency: quote.currency,
        amount: quote.amount,
        expires_at_unix: auth::unix_now() + PERIOD_SECS,
    }))
}

#[derive(Debug, Deserialize)]
pub struct ConfirmRequest {
    pub payment_id: String,
}

/// `POST /api/subscription/confirm` — records the period once the invoice has
/// settled. The cluster is the source of truth: a payment id that never settled
/// buys nothing, and confirming twice is a no-op.
pub async fn confirm(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Json(request): Json<ConfirmRequest>,
) -> ApiResult<Json<SubscriptionStatus>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;

    let mut rows = connection
        .query(
            "SELECT currency, amount FROM subscription_invoices \
             WHERE payment_id = ?1 AND user_id = ?2",
            (request.payment_id.clone(), user.id.clone()),
        )
        .await
        .map_err(auth::internal)?;
    let (currency, amount): (String, i64) = match rows.next().await.map_err(auth::internal)? {
        Some(row) => (
            row.get(0).map_err(auth::internal)?,
            row.get(1).map_err(auth::internal)?,
        ),
        None => return Err(bad("That subscription invoice was not created here.")),
    };
    drop(rows);

    // Already credited: answering again is harmless and keeps a reload from
    // extending a period the buyer did not pay for.
    let mut paid = connection
        .query(
            "SELECT 1 FROM subscriptions WHERE user_id = ?1 AND payment_id = ?2",
            (user.id.clone(), request.payment_id.clone()),
        )
        .await
        .map_err(auth::internal)?;
    if paid.next().await.map_err(auth::internal)?.is_some() {
        return status(State(state), headers).await;
    }
    drop(paid);

    let record = payments::fetch_payment(
        &request.payment_id,
        PaymentQuery {
            cluster: Some(cluster()),
            endpoint: None,
        },
        &state.db,
    )
    .await?;

    if record.status != PaymentStatus::Settled {
        return Err(bad("That payment has not settled yet."));
    }
    // The invoice is ours only if it paid the platform the amount we quoted.
    let seller = platform_seller()?;
    if record.seller != seller.to_string() {
        return Err(bad("That payment did not go to the platform account."));
    }
    let paid_option = record
        .options
        .iter()
        .find(|option| option.currency == currency)
        .ok_or_else(|| bad("That payment is not in the invoice's currency."))?;
    if paid_option.amount != u64::try_from(amount).unwrap_or(u64::MAX) {
        return Err(bad("That payment is not the invoiced amount."));
    }

    let now = auth::unix_now();
    let (_, current_expiry, _, _) = latest(&state, &user.id).await?;
    // A renewal after a lapsed period starts now; one bought early adds to the
    // period already paid for, so nobody loses days by paying on time.
    let start = current_expiry.max(now);
    connection
        .execute(
            "INSERT INTO subscriptions \
             (id, user_id, currency, amount, payment_id, paid_at, expires_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                auth::random_id(),
                user.id,
                currency,
                amount,
                request.payment_id,
                now,
                start + PERIOD_SECS,
            ),
        )
        .await
        .map_err(auth::internal)?;

    status(State(state), headers).await
}

/// Grants one paid period, for the suites that need a subscribed seller without
/// a cluster to settle against.
#[cfg(test)]
pub(crate) async fn activate_for_tests(db: &libsql::Database, user_id: &str) {
    let connection = db.connect().expect("connection");
    connection
        .execute(
            "INSERT INTO subscriptions \
             (id, user_id, currency, amount, payment_id, paid_at, expires_at) \
             VALUES (?1, ?2, 'eurc', 1700000, ?3, ?4, ?5)",
            (
                auth::random_id(),
                user_id.to_string(),
                auth::random_id(),
                auth::unix_now(),
                auth::unix_now() + PERIOD_SECS,
            ),
        )
        .await
        .expect("grant subscription");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rates(eur_per_sol: f64, usd_per_eur: f64) -> Rates {
        Rates {
            eur_per_sol,
            usd_per_eur,
            fetched_at: 1_760_000_000,
        }
    }

    #[test]
    fn reads_the_feed_in_both_directions() {
        let (eur_per_sol, usd_per_eur) =
            parse_rates(r#"{"solana":{"eur":125.0,"usd":135.0}}"#).expect("parsed");
        assert_eq!(eur_per_sol, 125.0);
        assert!((usd_per_eur - 1.08).abs() < 1e-9);
        assert!(parse_rates(r#"{"solana":{"eur":0,"usd":135.0}}"#).is_none());
        assert!(parse_rates("not json").is_none());
    }

    #[test]
    fn prices_euro_at_par_and_the_others_by_rate() {
        // €1.70 at 1.08 USD per EUR, and at 125 EUR per SOL.
        let quotes = quotes_for(&rates(125.0, 1.08), "devnet");
        let by_currency = |name: &str| {
            quotes
                .iter()
                .find(|quote| quote.currency == name)
                .expect("currency")
                .amount
        };
        assert_eq!(by_currency("eurc"), 1_700_000);
        assert_eq!(by_currency("usdc"), 1_836_000);
        // 1.70 / 125 = 0.0136 SOL, rounded up to the lamport.
        assert_eq!(by_currency("sol"), 13_600_000);
        // The stablecoins carry the cluster's mint, SOL carries none.
        assert!(quotes
            .iter()
            .find(|q| q.currency == "sol")
            .unwrap()
            .mint
            .is_none());
        assert!(quotes
            .iter()
            .find(|q| q.currency == "usdc")
            .unwrap()
            .mint
            .is_some());
    }

    #[test]
    fn rounds_up_so_the_euro_price_is_always_covered() {
        // 1.70 EUR at 130.333… EUR/SOL is 0.013043478… SOL — 13,043,478.26
        // lamports, so the ceiling is 13,043,479 and never the shaved 478.
        let quotes = quotes_for(&rates(130.333_333_33, 1.0), "devnet");
        let sol = quotes.iter().find(|q| q.currency == "sol").unwrap().amount;
        assert_eq!(sol, 13_043_479);
        assert!(sol as f64 >= 1.7 / 130.333_333_33 * LAMPORTS_PER_SOL);

        let quotes = quotes_for(&rates(1.0, 1.0), "devnet");
        assert_eq!(
            quotes.iter().find(|q| q.currency == "usdc").unwrap().amount,
            1_700_000
        );
    }

    #[test]
    fn a_breakdown_never_quotes_zero() {
        let quotes = quotes_for(&rates(1e12, 1.0), "devnet");
        let sol = quotes.iter().find(|q| q.currency == "sol").unwrap().amount;
        assert_eq!(sol, 1);
    }
}
