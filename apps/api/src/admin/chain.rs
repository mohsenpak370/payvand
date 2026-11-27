//! The deployment's on-chain footprint, for the admin page: the program and the
//! accounts it owns, what each one holds against its rent-exempt minimum, and
//! the balances of the two server keys that spend money — the faucet that sends
//! starter tokens and the relayer that sponsors settle fees.
//!
//! Everything here is a read. Addresses come from the program id and the
//! server's own keypairs, never from the request, so the endpoint cannot be
//! pointed at an arbitrary account; the only input is the admin session.

use std::collections::BTreeMap;
use std::str::FromStr;

use axum::extract::{Json, State};
use axum::http::HeaderMap;
use serde::Serialize;
use solana_pubkey::Pubkey;
use solana_sdk::signer::Signer;

use crate::payments::{self, ApiError, ApiResult};
use crate::spl::{ata_for, TOKEN_PROGRAM};
use crate::{auth, faucet, sponsor};

use super::require_admin;

const UPGRADEABLE_LOADER: &str = "BPFLoaderUpgradeab1e11111111111111111111111";

/// One account the deployment depends on.
#[derive(Debug, Serialize, PartialEq)]
pub struct ChainAccount {
    /// Which account this is: `program`, `program_data`, `config`, `treasury`,
    /// `faucet` or `sponsor`. The page names it from this key.
    pub key: &'static str,
    pub address: String,
    pub exists: bool,
    pub lamports: u64,
    pub data_len: u64,
    /// The cluster's rent-exempt minimum for an account of `data_len` bytes —
    /// what the account must keep, and so what would come back if it closed.
    pub rent_exempt_lamports: u64,
    pub owner: Option<String>,
    pub executable: bool,
    /// USDC and EURC held in this account's associated token accounts, for the
    /// accounts that move tokens. `None` when the token account does not exist.
    pub tokens: Option<TokenBalances>,
}

#[derive(Debug, Serialize, PartialEq, Default)]
pub struct TokenBalances {
    pub usdc_base_units: Option<u64>,
    pub eurc_base_units: Option<u64>,
}

/// What the loader records about the deployed program.
#[derive(Debug, Serialize, PartialEq)]
pub struct ProgramInfo {
    /// The slot of the last deploy or upgrade.
    pub last_deploy_slot: Option<u64>,
    /// `None` once the program is finalized (immutable).
    pub upgrade_authority: Option<String>,
}

/// A server key that spends money, and how far its balance goes.
#[derive(Debug, Serialize)]
pub struct ServerWallet {
    /// `faucet` or `sponsor`.
    pub key: &'static str,
    /// False when the keypair variable is unset or unreadable.
    pub configured: bool,
    pub account: Option<ChainAccount>,
}

#[derive(Debug, Serialize)]
pub struct ChainOverview {
    pub cluster: String,
    pub program: ProgramInfo,
    pub accounts: Vec<ChainAccount>,
    pub wallets: Vec<ServerWallet>,
    /// What one starter-token claim sends, so the page can say how many claims
    /// the faucet can still pay for.
    pub faucet_sol_lamports: u64,
    pub faucet_token_base_units: u64,
    /// Whether this cluster hands out starter tokens at all.
    pub faucet_enabled: bool,
    /// What one sponsored settle fronts from the relayer.
    pub sponsor_settle_lamports: u64,
    pub read_at: i64,
}

fn pubkey(text: &str) -> ApiResult<Pubkey> {
    Pubkey::from_str(text).map_err(|error| ApiError::Internal(format!("bad address: {error}")))
}

/// The ProgramData address an upgradeable program account points at:
/// `UpgradeableLoaderState::Program { programdata_address }`, a `u32` tag of 2
/// followed by the 32-byte address.
pub(super) fn program_data_address(program_account: &[u8]) -> Option<Pubkey> {
    if program_account.len() < 36 || program_account[..4] != 2u32.to_le_bytes() {
        return None;
    }
    Pubkey::try_from(&program_account[4..36]).ok()
}

/// `UpgradeableLoaderState::ProgramData { slot, upgrade_authority_address }`:
/// a `u32` tag of 3, the `u64` slot, then an `Option<Pubkey>` (one byte, then
/// 32 when it is `Some`).
pub(super) fn decode_program_data(data: &[u8]) -> ProgramInfo {
    let empty = ProgramInfo {
        last_deploy_slot: None,
        upgrade_authority: None,
    };
    if data.len() < 13 || data[..4] != 3u32.to_le_bytes() {
        return empty;
    }
    let slot = u64::from_le_bytes(data[4..12].try_into().expect("eight bytes"));
    let authority = match data[12] {
        1 if data.len() >= 45 => Pubkey::try_from(&data[13..45])
            .ok()
            .map(|key| key.to_string()),
        _ => None,
    };
    ProgramInfo {
        last_deploy_slot: Some(slot),
        upgrade_authority: authority,
    }
}

/// The `amount` of an SPL token account: bytes 64..72, after the mint and the
/// owner. Anything that is not a token-program account reads as nothing.
pub(super) fn token_amount(owner: &Pubkey, data: &[u8]) -> Option<u64> {
    if owner.to_string() != TOKEN_PROGRAM || data.len() < 72 {
        return None;
    }
    Some(u64::from_le_bytes(data[64..72].try_into().ok()?))
}

/// `GET /api/admin/chain` — program accounts, their rent, and the server
/// wallets' SOL, USDC and EURC balances, read live from the cluster.
pub async fn chain(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<ChainOverview>> {
    require_admin(&state.db, &headers).await?;

    let cluster = payments::default_cluster();
    let rpc = payments::payment_rpc(payments::resolve_endpoint(None, &cluster)?);
    let unreachable = |error: solana_rpc_client_api::client_error::Error| {
        tracing::warn!(%error, "admin chain read failed");
        ApiError::Internal("The cluster did not answer. Try again in a moment.".into())
    };

    let program = pubkey(payments::PAYVAND_PROGRAM_ID)?;
    let config = Pubkey::find_program_address(&[b"config"], &program).0;
    let treasury = Pubkey::find_program_address(&[b"treasury"], &program).0;
    let loader = pubkey(UPGRADEABLE_LOADER)?;
    // Where the loader keeps the bytes; read from the program account below
    // when it exists, derived here so a missing program still shows a row.
    let derived_program_data = Pubkey::find_program_address(&[program.as_ref()], &loader).0;

    let faucet_key = crate::keypair_from_env("FAUCET_KEYPAIR").map(|key| key.pubkey());
    let sponsor_key = crate::keypair_from_env("SPONSOR_KEYPAIR").map(|key| key.pubkey());
    let mints = [
        payments::mint_for("usdc", &cluster),
        payments::mint_for("eurc", &cluster),
    ];

    // Every address in one request: the four program accounts, the two server
    // wallets, and the USDC/EURC token account of each holder.
    let mut named: Vec<(&'static str, Pubkey)> = vec![
        ("program", program),
        ("program_data", derived_program_data),
        ("config", config),
        ("treasury", treasury),
    ];
    if let Some(key) = faucet_key {
        named.push(("faucet", key));
    }
    if let Some(key) = sponsor_key {
        named.push(("sponsor", key));
    }
    let holders: Vec<(&'static str, Pubkey)> = named
        .iter()
        .filter(|(name, _)| matches!(*name, "treasury" | "faucet" | "sponsor"))
        .copied()
        .collect();
    let mut addresses: Vec<Pubkey> = named.iter().map(|(_, key)| *key).collect();
    for (_, owner) in &holders {
        for mint in mints.iter().flatten() {
            addresses.push(ata_for(owner, mint));
        }
    }

    let fetched = rpc
        .get_multiple_accounts(&addresses)
        .await
        .map_err(unreachable)?;
    let by_address: BTreeMap<Pubkey, _> = addresses.iter().copied().zip(fetched).collect();

    let program_info = by_address
        .get(&program)
        .and_then(Option::as_ref)
        .and_then(|account| program_data_address(&account.data))
        .filter(|address| *address == derived_program_data)
        .and_then(|_| by_address.get(&derived_program_data))
        .and_then(Option::as_ref)
        .map(|account| decode_program_data(&account.data))
        .unwrap_or(ProgramInfo {
            last_deploy_slot: None,
            upgrade_authority: None,
        });

    // One rent read per distinct size; a deployment has three or four.
    let mut rents: BTreeMap<u64, u64> = BTreeMap::new();
    for (_, key) in &named {
        let len = by_address
            .get(key)
            .and_then(Option::as_ref)
            .map_or(0, |account| account.data.len() as u64);
        if let std::collections::btree_map::Entry::Vacant(slot) = rents.entry(len) {
            let rent = rpc
                .get_minimum_balance_for_rent_exemption(len as usize)
                .await
                .map_err(unreachable)?;
            slot.insert(rent);
        }
    }

    let token_balance = |owner: &Pubkey, mint: Option<Pubkey>| -> Option<u64> {
        let mint = mint?;
        let account = by_address.get(&ata_for(owner, &mint))?.as_ref()?;
        token_amount(&account.owner, &account.data)
    };

    let mut accounts: Vec<ChainAccount> = named
        .iter()
        .map(|(name, key)| {
            let account = by_address.get(key).and_then(Option::as_ref);
            let data_len = account.map_or(0, |account| account.data.len() as u64);
            let holds_tokens = holders.iter().any(|(holder, _)| holder == name);
            ChainAccount {
                key: name,
                address: key.to_string(),
                exists: account.is_some(),
                lamports: account.map_or(0, |account| account.lamports),
                data_len,
                rent_exempt_lamports: rents.get(&data_len).copied().unwrap_or(0),
                owner: account.map(|account| account.owner.to_string()),
                executable: account.is_some_and(|account| account.executable),
                tokens: holds_tokens.then(|| TokenBalances {
                    usdc_base_units: token_balance(key, mints[0]),
                    eurc_base_units: token_balance(key, mints[1]),
                }),
            }
        })
        .collect();

    // The server wallets are listed apart from the program's own accounts:
    // they are keys this deployment holds, not state the program owns.
    let mut wallets = Vec::new();
    for (name, key) in [("faucet", faucet_key), ("sponsor", sponsor_key)] {
        let account = key.and_then(|_| {
            accounts
                .iter()
                .position(|account| account.key == name)
                .map(|index| accounts.remove(index))
        });
        wallets.push(ServerWallet {
            key: name,
            configured: key.is_some(),
            account,
        });
    }

    Ok(Json(ChainOverview {
        faucet_enabled: faucet::hands_out(&cluster),
        cluster,
        program: program_info,
        accounts,
        wallets,
        faucet_sol_lamports: faucet::sol_lamports(),
        faucet_token_base_units: faucet::token_base_units(),
        sponsor_settle_lamports: sponsor::SPONSORED_LAMPORTS,
        read_at: auth::unix_now(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: &str) -> Pubkey {
        Pubkey::from_str(text).expect("valid address")
    }

    #[test]
    fn reads_the_program_data_address_from_an_upgradeable_program() {
        let program_data = key("6dQgFZDTkhDMTZv9ftZe4ufU9jG5fvJEw6zpw6WCKjPz");
        let mut data = 2u32.to_le_bytes().to_vec();
        data.extend_from_slice(program_data.as_ref());
        assert_eq!(program_data_address(&data), Some(program_data));

        // Any other loader state, or a short account, is not a program.
        data[0] = 3;
        assert_eq!(program_data_address(&data), None);
        assert_eq!(program_data_address(&[2, 0, 0, 0]), None);
    }

    #[test]
    fn decodes_the_deploy_slot_and_the_upgrade_authority() {
        let authority = key("Evmo7znLU9hSZr9t2VrGVLaqV7bifxG7RKPSGvM3Tvqw");
        let mut data = 3u32.to_le_bytes().to_vec();
        data.extend_from_slice(&522_529_255u64.to_le_bytes());
        data.push(1);
        data.extend_from_slice(authority.as_ref());
        data.extend_from_slice(&[0xAB; 64]); // the program bytes follow
        assert_eq!(
            decode_program_data(&data),
            ProgramInfo {
                last_deploy_slot: Some(522_529_255),
                upgrade_authority: Some(authority.to_string()),
            }
        );

        // A finalized program keeps its slot and has no authority.
        let mut finalized = 3u32.to_le_bytes().to_vec();
        finalized.extend_from_slice(&7u64.to_le_bytes());
        finalized.push(0);
        assert_eq!(
            decode_program_data(&finalized),
            ProgramInfo {
                last_deploy_slot: Some(7),
                upgrade_authority: None,
            }
        );

        assert_eq!(decode_program_data(&[0; 8]).last_deploy_slot, None);
    }

    #[test]
    fn reads_a_token_amount_only_from_a_token_program_account() {
        let mut data = vec![0u8; 165];
        data[64..72].copy_from_slice(&12_345_678u64.to_le_bytes());
        assert_eq!(token_amount(&key(TOKEN_PROGRAM), &data), Some(12_345_678));
        assert_eq!(
            token_amount(&key("11111111111111111111111111111111"), &data),
            None
        );
        assert_eq!(token_amount(&key(TOKEN_PROGRAM), &data[..70]), None);
    }
}
