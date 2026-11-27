//! The plan's price, and what it comes to in each accepted currency.

use serde::Serialize;

use crate::payments;

/// €1.70, in cents: the whole price is kept in cents so no rounding creeps in.
pub const PLAN_PRICE_EUR_CENTS: u64 = 170;
/// A paid period. 30 days from the moment the payment settles, so a payment
/// never has to care what month it landed in.
pub const PERIOD_SECS: i64 = 30 * 24 * 60 * 60;
/// Six decimals on both stablecoins.
const TOKEN_DECIMALS: u32 = 6;
const LAMPORTS_PER_SOL: f64 = 1_000_000_000.0;

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

/// The status fields that depend on the rate read.
///
/// An outage is not the account's problem: a missing rate drops the prices and
/// the timestamp rather than the whole answer.
pub(super) fn quoted(rates: Option<&Rates>, cluster: &str) -> (Vec<Quote>, i64, bool) {
    match rates {
        Some(rates) => (quotes_for(rates, cluster), rates.fetched_at, true),
        None => (Vec::new(), 0, false),
    }
}

#[cfg(test)]
mod tests;
