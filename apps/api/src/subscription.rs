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
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use crate::auth;
use crate::payments::{
    self, bad, ApiError, ApiResult, CreateOption, CreatePaymentRequest, PaymentQuery, PaymentStatus,
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
/// The keyless feed the payment page already prices from: one call for one SOL's
/// price in both currencies (`EUR` and `USD`), read by the web app's own
/// `/api/rates` route as well, so the plan's quote and the fiat hints in the
/// checkout come from the same numbers. It is first, and verified answering from
/// Vercel's network, rather than the other way round.
const COINBASE_RATES_URL: &str = "https://api.coinbase.com/v2/exchange-rates?currency=SOL";
/// The second keyless feed, read only when Coinbase cannot answer. Its
/// simple-price shape carries both numbers in one object too; it is second
/// because its public endpoint rate-limits shared hosts, which a serverless
/// egress is. A quote needs only one of the two to answer.
const COINGECKO_RATES_URL: &str =
    "https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=eur,usd";
/// How long one feed read may take before it counts as failed. The page waits on
/// this call and the function's own budget is shorter than a hung socket, so the
/// timeout is the read's, not the request's.
const RATES_TIMEOUT_SECS: u64 = 8;
/// How much of an unexpected feed answer goes into the log. Long enough to see
/// the shape or the error, short enough that the fallback's full currency table
/// never lands in a log line.
const RATES_BODY_LOG_CHARS: usize = 200;

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

/// The feeds to try, in order: an operator's own URL if one is configured,
/// otherwise the payment page's feed and the second one behind it. A configured
/// URL is used alone — pointing the app at a feed of your own should not have it
/// second-guessed.
fn rates_urls() -> Vec<String> {
    match std::env::var("SUBSCRIPTION_RATES_URL") {
        Ok(url) if !url.trim().is_empty() => vec![url],
        _ => vec![
            COINBASE_RATES_URL.to_string(),
            COINGECKO_RATES_URL.to_string(),
        ],
    }
}

fn sane_pair(eur_per_sol: f64, usd_per_eur: f64) -> Option<(f64, f64)> {
    (eur_per_sol.is_finite() && usd_per_eur.is_finite() && eur_per_sol > 0.0 && usd_per_eur > 0.0)
        .then_some((eur_per_sol, usd_per_eur))
}

/// The two numbers, read from whichever keyless feed answered.
///
/// CoinGecko's `{"solana":{"eur":…,"usd":…}}` carries both prices directly;
/// Coinbase's `{"data":{"rates":{"EUR":"…","USD":"…"}}}` carries each
/// currency's price of one SOL as a decimal string, so the euro→dollar rate is
/// their quotient. A zero or missing price is a broken read, not a free rate.
fn parse_rates(body: &str) -> Option<(f64, f64)> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    if let Some(solana) = value.get("solana") {
        let eur = solana.get("eur")?.as_f64()?;
        let usd = solana.get("usd")?.as_f64()?;
        return sane_pair(eur, usd / eur);
    }
    let rates = value.get("data")?.get("rates")?;
    let price_of_sol = |code: &str| rates.get(code)?.as_str()?.trim().parse::<f64>().ok();
    let eur = price_of_sol("EUR")?;
    let usd = price_of_sol("USD")?;
    sane_pair(eur, usd / eur)
}

/// The first [`RATES_BODY_LOG_CHARS`] characters of a feed answer, for a log
/// line that stays readable whether the body is an error or a currency table.
fn body_head(body: &str) -> &str {
    match body.char_indices().nth(RATES_BODY_LOG_CHARS) {
        Some((end, _)) => &body[..end],
        None => body,
    }
}

/// One feed read, with the reason it failed logged. `None` is every failure: a
/// refused connection, a non-2xx status, an unreadable body, or a price that
/// does not make sense. The caller tries the next feed rather than guessing.
async fn read_rates(url: &str) -> Option<Rates> {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(RATES_TIMEOUT_SECS))
        // A named client, because a bare default one is what a shared-address
        // host gets challenged for.
        .user_agent(concat!("payvand/", env!("CARGO_PKG_VERSION")))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            tracing::error!(%error, %url, "subscription rate client unavailable");
            return None;
        }
    };

    let response = match client.get(url).send().await {
        Ok(response) => response,
        Err(error) => {
            tracing::error!(%error, %url, "subscription rate read failed");
            return None;
        }
    };
    let status = response.status();
    let body = match response.text().await {
        Ok(body) => body,
        Err(error) => {
            tracing::error!(%error, %url, "subscription rate body unreadable");
            return None;
        }
    };
    if !status.is_success() {
        tracing::error!(
            %url,
            %status,
            body = %body_head(&body),
            "subscription rate feed refused the read"
        );
        return None;
    }
    let (eur_per_sol, usd_per_eur) = match parse_rates(&body) {
        Some(pair) => pair,
        None => {
            tracing::error!(
                %url,
                body = %body_head(&body),
                "subscription rate response in an unknown shape"
            );
            return None;
        }
    };
    Some(Rates {
        eur_per_sol,
        usd_per_eur,
        fetched_at: auth::unix_now(),
    })
}

/// The live rate, cached for [`RATE_TTL_SECS`], read from the first feed that
/// answers. A failed read never invents a price: when every feed is down the
/// quote is refused — `status` still answers without one, and `create_invoice`
/// tells the caller to retry.
pub async fn rates() -> ApiResult<Rates> {
    let mut guard = rates_cache().lock().await;
    if let Some((read_at, rates)) = *guard {
        if read_at.elapsed() < Duration::from_secs(RATE_TTL_SECS) {
            return Ok(rates);
        }
    }

    for url in rates_urls() {
        if let Some(rates) = read_rates(&url).await {
            *guard = Some((Instant::now(), rates));
            return Ok(rates);
        }
    }
    Err(bad(
        "The exchange rate is unavailable right now. Try again in a moment.",
    ))
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
    /// Empty when no rate could be read.
    pub quotes: Vec<Quote>,
    /// When the rate behind `quotes` was read; 0 when there is no quote.
    pub rated_at_unix: i64,
    /// False when the exchange rate could not be read. The period and the plan
    /// are still answered — only the prices are missing, so the card can still
    /// say where the subscription stands while the buttons wait.
    pub rates_available: bool,
}

/// The status fields that depend on the rate read.
///
/// An outage is not the account's problem: a missing rate drops the prices and
/// the timestamp rather than the whole answer.
fn quoted(rates: Option<&Rates>, cluster: &str) -> (Vec<Quote>, i64, bool) {
    match rates {
        Some(rates) => (quotes_for(rates, cluster), rates.fetched_at, true),
        None => (Vec::new(), 0, false),
    }
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

/// How far back a status read looks for a paid-but-unconfirmed invoice. Three
/// days covers a buyer who paid on Friday and came back on Monday; older than
/// that the invoice is abandoned rather than pending, and is not re-read
/// forever.
const REPAIR_WINDOW_SECS: i64 = 3 * 24 * 60 * 60;
/// How many such invoices one read may check — the newest is the one that paid.
const REPAIR_INVOICES: usize = 3;
/// Repairs for one account are spaced out: opening a page is not a chain poll.
const REPAIR_INTERVAL_SECS: u64 = 60;

/// When each account was last checked for an unconfirmed payment.
fn repair_cache() -> &'static Mutex<HashMap<String, Instant>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Credits the invoices this account was quoted recently and never confirmed.
///
/// A payment can settle with nobody left to say so: the tab closed while the
/// wallet was approving, the confirm was refused, or the browser forgot the
/// invoice id. The money is on chain either way, so the month it bought is not
/// something to lose — the next ask about the subscription is where it is
/// noticed, and therefore where it is repaired.
///
async fn credit_unconfirmed(state: &crate::AppState, user_id: &str) -> usize {
    {
        let mut seen = repair_cache().lock().await;
        let now = Instant::now();
        let interval = Duration::from_secs(REPAIR_INTERVAL_SECS);
        if seen
            .get(user_id)
            .is_some_and(|at| now.duration_since(*at) < interval)
        {
            return 0;
        }
        // The map is one string per account that has opened the shops page; an
        // hour of it is enough, and keeps a long-lived instance bounded.
        seen.retain(|_, at| now.duration_since(*at) < interval * 60);
        seen.insert(user_id.to_string(), now);
    }

    let Some(ids) = unconfirmed_invoices(state, user_id).await else {
        return 0;
    };
    let mut written = 0;
    for id in ids {
        match credit_invoice(state, user_id, &id).await {
            Ok(true) => written += 1,
            Ok(false) => {}
            // An invoice nobody has paid yet answers "not settled": that is the
            // ordinary case for a link that was opened and left, not a fault.
            Err(ApiError::BadRequest(_)) => {}
            Err(error) => {
                tracing::info!(%user_id, %id, %error, "subscription repair skipped")
            }
        }
    }
    written
}

/// The account's recent invoices that no period was recorded for.
async fn unconfirmed_invoices(state: &crate::AppState, user_id: &str) -> Option<Vec<String>> {
    let connection = state.db.connect().ok()?;
    let mut rows = connection
        .query(
            "SELECT i.payment_id FROM subscription_invoices i \
             WHERE i.user_id = ?1 AND i.created_at >= ?2 \
             AND NOT EXISTS (SELECT 1 FROM subscriptions s \
                             WHERE s.user_id = i.user_id AND s.payment_id = i.payment_id) \
             ORDER BY i.created_at DESC LIMIT ?3",
            (
                user_id.to_string(),
                auth::unix_now() - REPAIR_WINDOW_SECS,
                REPAIR_INVOICES as i64,
            ),
        )
        .await
        .ok()?;
    let mut ids = Vec::new();
    while let Some(row) = rows.next().await.ok()? {
        ids.push(row.get::<String>(0).ok()?);
    }
    Some(ids)
}

/// `GET /api/subscription` — the account's period and the plan's three prices.
pub async fn status(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<SubscriptionStatus>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let (mut active, mut expires_at_unix, mut currency, mut amount) =
        latest(&state, &user.id).await?;
    if !active && credit_unconfirmed(&state, &user.id).await > 0 {
        (active, expires_at_unix, currency, amount) = latest(&state, &user.id).await?;
    }
    let cluster = cluster();
    // Every feed being down costs the prices, not the answer: `rates()` has
    // already logged why, and an active period must still be visible.
    let rates = rates().await.ok();
    let (quotes, rated_at_unix, rates_available) = quoted(rates.as_ref(), &cluster);
    Ok(Json(SubscriptionStatus {
        active,
        expires_at_unix,
        currency,
        amount,
        price_eur_cents: PLAN_PRICE_EUR_CENTS,
        period_days: PERIOD_SECS / (24 * 60 * 60),
        quotes,
        rated_at_unix,
        rates_available,
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
    credit_invoice(&state, &user.id, &request.payment_id).await?;
    status(State(state), headers).await
}

/// Verifies one invoice against the cluster and records the period it buys.
///
/// Both the browser saying "I paid" and a status read repairing a payment
/// nobody confirmed go through this, so the checks live in one place: the
/// invoice must have been quoted to this account, it must have settled, it must
/// have paid the platform, and it must have paid the amount it was quoted at.
///
/// Answers whether a period was written. An invoice that was already credited
/// writes nothing — that is what a reload after paying looks like, and it must
/// not buy a second month.
async fn credit_invoice(
    state: &crate::AppState,
    user_id: &str,
    payment_id: &str,
) -> ApiResult<bool> {
    let connection = state.db.connect().map_err(auth::internal)?;

    let mut rows = connection
        .query(
            "SELECT currency, amount FROM subscription_invoices \
             WHERE payment_id = ?1 AND user_id = ?2",
            (payment_id.to_string(), user_id.to_string()),
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

    let mut paid = connection
        .query(
            "SELECT 1 FROM subscriptions WHERE user_id = ?1 AND payment_id = ?2",
            (user_id.to_string(), payment_id.to_string()),
        )
        .await
        .map_err(auth::internal)?;
    if paid.next().await.map_err(auth::internal)?.is_some() {
        return Ok(false);
    }
    drop(paid);

    let record = payments::fetch_payment(
        payment_id,
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
    let (_, current_expiry, _, _) = latest(state, user_id).await?;
    // A renewal after a lapsed period starts now; one bought early adds to the
    // period already paid for, so nobody loses days by paying on time.
    let start = current_expiry.max(now);
    connection
        .execute(
            // `OR IGNORE` because two requests can reach this per invoice (the
            // browser's confirm and a status read's repair): the period is one,
            // and the row's uniqueness is what says so.
            "INSERT OR IGNORE INTO subscriptions \
             (id, user_id, currency, amount, payment_id, paid_at, expires_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                auth::random_id(),
                user_id.to_string(),
                currency,
                amount,
                payment_id.to_string(),
                now,
                start + PERIOD_SECS,
            ),
        )
        .await
        .map_err(auth::internal)?;

    Ok(true)
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

    /// The real feeds over the network, through the production read path:
    /// `cargo test --lib live_rates -- --ignored --nocapture`. Ignored by
    /// default because it needs the internet, and because a feed's own outage
    /// should not fail the suite — every feed that answers is checked, and at
    /// least one has to.
    #[tokio::test]
    #[ignore = "reads the live rate feeds"]
    async fn live_rates_price_the_plan() {
        let mut answered = 0;
        for url in rates_urls() {
            let Some(read) = read_rates(&url).await else {
                println!("no answer from {url}");
                continue;
            };
            answered += 1;
            println!(
                "{url} → {:.2} EUR/SOL, {:.4} USD/EUR",
                read.eur_per_sol, read.usd_per_eur
            );
            let quotes = quotes_for(&read, "devnet");
            let amount = |currency: &str| {
                quotes
                    .iter()
                    .find(|quote| quote.currency == currency)
                    .map(|quote| quote.amount)
                    .expect("the plan has a price in every currency")
            };
            // EURC is the plan's own currency, so it pays at par, exactly.
            assert_eq!(amount("eurc"), 1_700_000);
            // €1.70 at a sane euro-dollar rate is one to two dollars.
            assert!((1_500_000..=2_500_000).contains(&amount("usdc")));
            // And at any real SOL price it is a fraction of a SOL.
            assert!(amount("sol") > 5_000_000 && amount("sol") < 200_000_000);
        }
        assert!(answered > 0, "no rate feed answered");
    }

    #[test]
    fn an_unread_rate_costs_the_prices_only() {
        let (quotes, rated_at, available) = quoted(None, "devnet");
        assert!(quotes.is_empty());
        assert_eq!(rated_at, 0);
        assert!(!available);

        let read = rates(125.0, 1.08);
        let (quotes, rated_at, available) = quoted(Some(&read), "devnet");
        assert_eq!(quotes.len(), CURRENCIES.len());
        assert_eq!(rated_at, read.fetched_at);
        assert!(available);
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
    fn reads_the_fallback_feed_in_the_same_currency_pair() {
        // Coinbase's shape: decimal strings, one SOL's price in each currency.
        let body = r#"{"data":{"currency":"SOL","rates":{"EUR":"66.03","USD":"72.41"}}}"#;
        let (eur_per_sol, usd_per_eur) = parse_rates(body).expect("parsed");
        assert_eq!(eur_per_sol, 66.03);
        assert!((usd_per_eur - 72.41 / 66.03).abs() < 1e-12);

        // A zero, a missing price or a word where a number belongs is a broken
        // read rather than a free rate.
        assert!(parse_rates(r#"{"data":{"rates":{"EUR":"0","USD":"72.41"}}}"#).is_none());
        assert!(parse_rates(r#"{"data":{"rates":{"EUR":"66.03"}}}"#).is_none());
        assert!(parse_rates(r#"{"data":{"rates":{"EUR":"sixty","USD":"72.41"}}}"#).is_none());
    }

    #[test]
    fn logs_a_bounded_slice_of_an_unexpected_answer() {
        assert_eq!(body_head("short"), "short");
        let long = "x".repeat(RATES_BODY_LOG_CHARS * 2);
        assert_eq!(body_head(&long).chars().count(), RATES_BODY_LOG_CHARS);
        // Counted in characters, so a multi-byte answer cannot split a code point.
        let unicode = "€".repeat(RATES_BODY_LOG_CHARS * 2);
        assert_eq!(body_head(&unicode).chars().count(), RATES_BODY_LOG_CHARS);
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
