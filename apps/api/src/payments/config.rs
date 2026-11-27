//! Program id, currency registry, RPC client and endpoint allowlist.

use solana_commitment_config::CommitmentConfig;
use solana_pubkey::Pubkey;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use std::str::FromStr;

use super::error::{bad, ApiError, ApiResult};

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

/// `declare_id!` of the program, as a pubkey.
pub(crate) fn program_id() -> ApiResult<Pubkey> {
    Pubkey::from_str(PAYVAND_PROGRAM_ID)
        .map_err(|error| ApiError::Internal(format!("bad program id: {error}")))
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

/// `declare_id!` of the program in `apps/contracts/programs/payvand` —
/// the sole owner of payment and budget accounts.
pub const PAYVAND_PROGRAM_ID: &str = "4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J";

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

#[cfg(test)]
mod tests;
