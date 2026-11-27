//! Where starter tokens are handed out, and how many.

/// 1 SOL, which covers hundreds of network fees and the small rents a wallet pays
/// for its own token accounts.
const DEFAULT_SOL_LAMPORTS: u64 = 1_000_000_000;
/// 10 tokens, six decimals: enough for several invoices in either stablecoin.
const DEFAULT_TOKEN_BASE_UNITS: u64 = 10 * 1_000_000;

/// Start tokens are a test-network feature, full stop. A local validator is the
/// other place they make sense; everything else is a real chain.
pub(crate) fn hands_out(cluster: &str) -> bool {
    matches!(cluster, "devnet" | "localnet")
}

fn amount_from_env(name: &str, fallback: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

pub(crate) fn sol_lamports() -> u64 {
    amount_from_env("FAUCET_SOL_LAMPORTS", DEFAULT_SOL_LAMPORTS)
}

pub(crate) fn token_base_units() -> u64 {
    amount_from_env("FAUCET_TOKEN_BASE_UNITS", DEFAULT_TOKEN_BASE_UNITS)
}

#[cfg(test)]
mod tests;
