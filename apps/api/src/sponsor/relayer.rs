//! The relayer's configuration: its keypair, the program it settles against,
//! and the daily caps, all read from the environment.

use std::str::FromStr;

use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;

use crate::payments::ApiError;

/// The deployed program, overridable for a local cluster.
const DEFAULT_PROGRAM_ID: &str = "4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J";

/// What a sponsored settle fronts: the 262-byte token invoice's rent, the
/// 48-byte marker's, and the one signature the relayer pays for. The client asks
/// for a sponsor only below this, so a funded wallet keeps its own path.
pub const SPONSORED_LAMPORTS: u64 = 2_714_400 + 1_224_960 + 5_000;

/// What the endpoint answers with while it is not configured: the client shows
/// it instead of a wallet prompt that would fail on chain.
pub(super) fn unavailable() -> ApiError {
    ApiError::TooManyRequests(
        "This payment needs a little SOL for the network fee, and sponsorship is not available right now."
            .into(),
    )
}

pub(super) fn cap_from_env(name: &str, fallback: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(fallback)
}

/// The relayer's keypair, from `SPONSOR_KEYPAIR` — the solana CLI's JSON array
/// of 64 bytes, or the same bytes base58 encoded. Absent means no sponsorship.
pub(super) fn sponsor_keypair() -> Option<Keypair> {
    crate::keypair_from_env("SPONSOR_KEYPAIR")
}

pub(super) fn program_id() -> Pubkey {
    std::env::var("PAYVAND_PROGRAM_ID")
        .ok()
        .and_then(|value| Pubkey::from_str(value.trim()).ok())
        .unwrap_or_else(|| Pubkey::from_str(DEFAULT_PROGRAM_ID).expect("the built-in id parses"))
}

#[cfg(test)]
mod tests;
