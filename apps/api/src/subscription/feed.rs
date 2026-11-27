//! The live exchange rate, read from a keyless price feed and cached.

use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use crate::auth;
use crate::payments::{bad, ApiResult};

use super::pricing::Rates;

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

#[cfg(test)]
mod tests;
