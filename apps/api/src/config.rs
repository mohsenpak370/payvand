//! `GET /api/config` — every number a quote needs, in one cached answer.
//!
//! A checkout quotes a price from three things: the fee policy the program
//! settles with, the cluster's rent minimums for the accounts a payment opens,
//! and the mint addresses the currencies are priced in. All three live on chain,
//! and the browser used to read them itself — an account read and six rent
//! reads, from every tab, every minute, against the buyer's own RPC.
//!
//! This answers them from the API instead. The API reads the cluster once and
//! keeps the answer for a minute, so a hundred checkouts in that minute cost one
//! set of reads; the response also carries a minute of `Cache-Control`, so a CDN
//! in front of it costs none at all. A read that fails is never cached.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::extract::Query;
use axum::http::header;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use solana_pubkey::Pubkey;
use std::str::FromStr;

use crate::payments::{self, ApiError, ApiResult};

/// `CONFIG_SEED`: `seeds = [b"config"]` under the Payvand program.
const CONFIG_SEED: &[u8] = b"config";
/// `CONFIG_SPACE` in the program: the account this endpoint reads.
const CONFIG_SPACE: usize = 184;

/// The account sizes a quote asks the cluster's rent for. The names are the
/// ones the response and the client both use, so a size is never guessed twice.
const PAYMENT_SPACE: u64 = 230;
const TOKEN_PAYMENT_SPACE: u64 = 262;
const PAYMENT_RECEIPT_SPACE: u64 = 48;
const TOKEN_ACCOUNT_SPACE: u64 = 165;
const BUDGET_SPACE: u64 = 97;
const TOKEN_BUDGET_SPACE: u64 = 129;

/// How long one read of the policy and the rents answers every request.
///
/// A fee change is an admin instruction, not a market move: a minute of lag
/// costs a quote nothing, and the same minute is what the checkout already holds
/// its own reads for.
const CONFIG_TTL: Duration = Duration::from_secs(60);

/// What the browser receives. Field names are the ones the client reads, so the
/// shape is one contract rather than a translation.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ConfigResponse {
    pub cluster: String,
    pub program_id: String,
    pub admin: String,
    pub fee_mode: String,
    pub session_fee_float: u64,
    pub fees: Fees,
    pub mints: Mints,
    pub spaces: Spaces,
    pub rent: Rent,
}

/// `FeePolicy` plus the two per-signature network fees beside it: the numbers
/// `fee_for` reads at settlement.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Fees {
    pub fee_bps_below: u16,
    pub fee_bps_above: u16,
    pub sol_tier_lamports: u64,
    pub token_tier_base_units: u64,
    pub flat_usdc_base_units: u64,
    pub flat_eurc_base_units: u64,
    pub network_fee_lamports: u64,
    pub network_fee_usdc_base_units: u64,
    pub network_fee_eurc_base_units: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Mints {
    pub usdc: String,
    pub eurc: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Spaces {
    pub payment: u64,
    pub token_payment: u64,
    pub payment_receipt: u64,
    pub token_account: u64,
    pub budget: u64,
    pub token_budget: u64,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Rent {
    pub payment: u64,
    pub token_payment: u64,
    pub payment_receipt: u64,
    pub token_account: u64,
    pub budget: u64,
    pub token_budget: u64,
}

#[derive(Debug, Deserialize)]
pub struct ConfigQuery {
    /// Which cluster's policy to answer; the deployment's own when omitted.
    pub cluster: Option<String>,
}

struct ConfigCache {
    inner: Mutex<HashMap<String, (Instant, ConfigResponse)>>,
}

static CONFIG_CACHE: OnceLock<ConfigCache> = OnceLock::new();

fn config_cache() -> &'static ConfigCache {
    CONFIG_CACHE.get_or_init(|| ConfigCache {
        inner: Mutex::new(HashMap::new()),
    })
}

fn cached_config(cluster: &str) -> Option<ConfigResponse> {
    let mut inner = config_cache()
        .inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cached_config_in(&mut inner, cluster, Instant::now())
}

fn cached_config_in(
    inner: &mut HashMap<String, (Instant, ConfigResponse)>,
    cluster: &str,
    now: Instant,
) -> Option<ConfigResponse> {
    let (read_at, config) = inner.get(cluster)?;
    if now.duration_since(*read_at) >= CONFIG_TTL {
        inner.remove(cluster);
        return None;
    }
    Some(config.clone())
}

fn remember_config(cluster: &str, config: &ConfigResponse) {
    let mut inner = config_cache()
        .inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    remember_config_in(&mut inner, cluster, config, Instant::now());
}

fn remember_config_in(
    inner: &mut HashMap<String, (Instant, ConfigResponse)>,
    cluster: &str,
    config: &ConfigResponse,
    now: Instant,
) {
    // One entry per cluster, and a cluster is one of two or three names: the map
    // cannot grow past a handful, so expired entries are simply dropped here.
    inner.retain(|_, (read_at, _)| now.duration_since(*read_at) < CONFIG_TTL);
    inner.insert(cluster.to_owned(), (now, config.clone()));
}

/// The fee policy, the mints and the rent minimums, from one read a minute.
///
/// Public, because a checkout quotes before anyone signs in. The response is
/// cacheable for the same minute it is kept: a CDN in front of the API then
/// answers every tab of every buyer for free.
pub async fn config(Query(query): Query<ConfigQuery>) -> ApiResult<impl IntoResponse> {
    let cluster = query.cluster.unwrap_or_else(payments::default_cluster);
    let payload = match cached_config(&cluster) {
        Some(cached) => cached,
        None => {
            let endpoint = payments::resolve_endpoint(None, &cluster)?;
            let read = read_config(&endpoint, &cluster).await?;
            remember_config(&cluster, &read);
            read
        }
    };
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=60")],
        Json(payload),
    ))
}

/// Reads the policy account and the six rent minimums from `endpoint`.
///
/// A cluster that cannot answer is an error rather than a guess: a quote built
/// on an invented fee is a transaction that fails its pre-flight, or charges the
/// wrong amount.
pub async fn read_config(endpoint: &str, cluster: &str) -> ApiResult<ConfigResponse> {
    let program = Pubkey::from_str(payments::PAYVAND_PROGRAM_ID)
        .map_err(|error| ApiError::Internal(format!("bad program id: {error}")))?;
    let address = Pubkey::find_program_address(&[CONFIG_SEED], &program).0;

    let rpc = payments::payment_rpc(endpoint.to_owned());
    let account = match rpc.get_account(&address).await {
        Ok(account) => account,
        Err(error) => {
            tracing::info!(%error, %address, "the fee policy is not readable");
            return Err(ApiError::Internal(
                "The fee policy is not readable. Try again.".into(),
            ));
        }
    };

    let policy = decode_policy(&account.data, cluster).ok_or_else(|| {
        ApiError::Internal("The fee policy account is not one this program wrote.".into())
    })?;

    let (payment, token_payment, payment_receipt, token_account, budget, token_budget) = tokio::join!(
        rpc.get_minimum_balance_for_rent_exemption(PAYMENT_SPACE as usize),
        rpc.get_minimum_balance_for_rent_exemption(TOKEN_PAYMENT_SPACE as usize),
        rpc.get_minimum_balance_for_rent_exemption(PAYMENT_RECEIPT_SPACE as usize),
        rpc.get_minimum_balance_for_rent_exemption(TOKEN_ACCOUNT_SPACE as usize),
        rpc.get_minimum_balance_for_rent_exemption(BUDGET_SPACE as usize),
        rpc.get_minimum_balance_for_rent_exemption(TOKEN_BUDGET_SPACE as usize),
    );
    let rent = |value: Result<u64, _>| {
        value.map_err(|error| {
            tracing::info!(%error, "a rent minimum is not readable");
            ApiError::Internal("The cluster's rents are not readable. Try again.".into())
        })
    };

    Ok(ConfigResponse {
        cluster: cluster.to_string(),
        program_id: program.to_string(),
        rent: Rent {
            payment: rent(payment)?,
            token_payment: rent(token_payment)?,
            payment_receipt: rent(payment_receipt)?,
            token_account: rent(token_account)?,
            budget: rent(budget)?,
            token_budget: rent(token_budget)?,
        },
        spaces: Spaces {
            payment: PAYMENT_SPACE,
            token_payment: TOKEN_PAYMENT_SPACE,
            payment_receipt: PAYMENT_RECEIPT_SPACE,
            token_account: TOKEN_ACCOUNT_SPACE,
            budget: BUDGET_SPACE,
            token_budget: TOKEN_BUDGET_SPACE,
        },
        ..policy
    })
}

/// The `Config` account, read by offset.
///
/// Mirrors the program's layout: the discriminator, the admin, the `FeePolicy`
/// (two rates, their two boundaries, the two per-currency flats, the two mints,
/// the network fee in lamports and in each token), the fee mode, the session
/// float and the bump/version bytes this endpoint does not need.
fn decode_policy(data: &[u8], cluster: &str) -> Option<ConfigResponse> {
    if data.len() < CONFIG_SPACE {
        return None;
    }
    let tagged = Sha256::digest(b"account:Config");
    if data[..8] != tagged[..8] {
        return None;
    }

    let read_u16 = |offset: usize| u16::from_le_bytes([data[offset], data[offset + 1]]);
    let read_u64 = |offset: usize| {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&data[offset..offset + 8]);
        u64::from_le_bytes(bytes)
    };
    let read_key = |offset: usize| {
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&data[offset..offset + 32]);
        Pubkey::new_from_array(bytes)
    };

    let admin = read_key(8);
    let usdc = read_key(76);
    let eurc = read_key(108);
    // The policy names its own mints; a deployment whose account predates them
    // falls back to the cluster's Circle addresses rather than an empty string.
    let mint_or = |named: Pubkey, currency: &str| {
        if named == Pubkey::default() {
            payments::mint_for(currency, cluster)
                .map(|mint| mint.to_string())
                .unwrap_or_default()
        } else {
            named.to_string()
        }
    };

    Some(ConfigResponse {
        cluster: cluster.to_string(),
        program_id: payments::PAYVAND_PROGRAM_ID.to_string(),
        admin: admin.to_string(),
        fee_mode: if data[164] == 1 { "buyer" } else { "seller" }.to_string(),
        session_fee_float: read_u64(165),
        mints: Mints {
            usdc: mint_or(usdc, "usdc"),
            eurc: mint_or(eurc, "eurc"),
        },
        fees: Fees {
            fee_bps_below: read_u16(40),
            fee_bps_above: read_u16(42),
            sol_tier_lamports: read_u64(44),
            token_tier_base_units: read_u64(52),
            flat_usdc_base_units: read_u64(60),
            flat_eurc_base_units: read_u64(68),
            network_fee_lamports: read_u64(140),
            network_fee_usdc_base_units: read_u64(148),
            network_fee_eurc_base_units: read_u64(156),
        },
        spaces: Spaces {
            payment: PAYMENT_SPACE,
            token_payment: TOKEN_PAYMENT_SPACE,
            payment_receipt: PAYMENT_RECEIPT_SPACE,
            token_account: TOKEN_ACCOUNT_SPACE,
            budget: BUDGET_SPACE,
            token_budget: TOKEN_BUDGET_SPACE,
        },
        rent: Rent::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// A `Config` account as the program writes one: a 2%/1% marginal policy, a
    /// 0.1 SOL and 10 token boundary, two flats, both mints, the network fees in
    /// lamports and in each token, a seller-paid mode and a 12,000,000 float.
    fn policy_bytes() -> Vec<u8> {
        let mut data = Sha256::digest(b"account:Config")[..8].to_vec();
        data.extend_from_slice(&[6u8; 32]); // admin
        data.extend_from_slice(&200u16.to_le_bytes()); // fee_bps_below
        data.extend_from_slice(&100u16.to_le_bytes()); // fee_bps_above
        data.extend_from_slice(&100_000_000u64.to_le_bytes()); // sol_tier_lamports
        data.extend_from_slice(&10_000_000u64.to_le_bytes()); // token_tier_base_units
        data.extend_from_slice(&62_782u64.to_le_bytes()); // flat_usdc_base_units
        data.extend_from_slice(&57_346u64.to_le_bytes()); // flat_eurc_base_units
        data.extend_from_slice(&[4u8; 32]); // usdc_mint
        data.extend_from_slice(&[5u8; 32]); // eurc_mint
        data.extend_from_slice(&5_000u64.to_le_bytes()); // network_fee_lamports
        data.extend_from_slice(&351u64.to_le_bytes()); // network_fee_usdc_base_units
        data.extend_from_slice(&320u64.to_le_bytes()); // network_fee_eurc_base_units
        data.push(1); // fee_mode: BuyerPays
        data.extend_from_slice(&12_000_000u64.to_le_bytes());
        data.extend_from_slice(&1_760_000_000i64.to_le_bytes()); // flats_updated_at
        data.push(251); // bump
        data.push(250); // treasury_bump
        data.push(4); // version
        data
    }

    #[test]
    fn decodes_the_policy_the_program_writes() {
        let data = policy_bytes();
        assert_eq!(data.len(), CONFIG_SPACE);
        let policy = decode_policy(&data, "devnet").expect("a policy");

        assert_eq!(policy.fees.fee_bps_below, 200);
        assert_eq!(policy.fees.fee_bps_above, 100);
        assert_eq!(policy.fees.sol_tier_lamports, 100_000_000);
        assert_eq!(policy.fees.token_tier_base_units, 10_000_000);
        assert_eq!(policy.fees.flat_usdc_base_units, 62_782);
        assert_eq!(policy.fees.flat_eurc_base_units, 57_346);
        assert_eq!(policy.fees.network_fee_lamports, 5_000);
        assert_eq!(policy.fees.network_fee_usdc_base_units, 351);
        assert_eq!(policy.fees.network_fee_eurc_base_units, 320);
        assert_eq!(policy.fee_mode, "buyer");
        assert_eq!(policy.session_fee_float, 12_000_000);
        assert_eq!(
            policy.mints.usdc,
            Pubkey::from([4u8; 32]).to_string(),
            "the mint the account names"
        );
        assert_eq!(policy.mints.eurc, Pubkey::from([5u8; 32]).to_string());
        assert_eq!(policy.spaces.payment, 230);
        assert_eq!(policy.spaces.token_budget, 129);
        assert_eq!(policy.cluster, "devnet");
        assert_eq!(policy.program_id, payments::PAYVAND_PROGRAM_ID);
    }

    /// The mints a policy names are the ones a quote prices against; one that
    /// names none falls back to the cluster's own, so a client is never handed an
    /// empty mint it would price a token payment at.
    #[test]
    fn a_policy_that_names_no_mint_prices_the_clusters() {
        let mut data = policy_bytes();
        data[76..108].fill(0);
        data[108..140].fill(0);
        let policy = decode_policy(&data, "devnet").expect("a policy");
        assert_eq!(
            policy.mints.usdc,
            payments::mint_for("usdc", "devnet")
                .expect("USDC on devnet")
                .to_string()
        );
        assert_eq!(
            policy.mints.eurc,
            payments::mint_for("eurc", "devnet")
                .expect("EURC on devnet")
                .to_string()
        );
    }

    /// Anything that is not this program's `Config` is refused rather than
    /// quoted: a wrong fee is a transaction that fails, or charges the wrong
    /// amount.
    #[test]
    fn a_short_or_foreign_account_is_not_a_policy() {
        assert!(decode_policy(&policy_bytes()[..100], "devnet").is_none());

        let mut foreign = policy_bytes();
        foreign[0] ^= 0xFF;
        assert!(decode_policy(&foreign, "devnet").is_none());
    }

    /// One read answers every request for a minute, per cluster, and the entry is
    /// dropped when it is looked at after that.
    #[test]
    fn a_config_is_kept_for_a_minute_per_cluster() {
        let devnet = decode_policy(&policy_bytes(), "devnet").expect("a policy");
        let mut inner = HashMap::new();
        let start = Instant::now();

        assert!(cached_config_in(&mut inner, "devnet", start).is_none());
        remember_config_in(&mut inner, "devnet", &devnet, start);

        let shared = cached_config_in(&mut inner, "devnet", start + Duration::from_secs(59))
            .expect("inside the minute");
        assert_eq!(shared.fees.fee_bps_below, 200);
        assert!(
            cached_config_in(&mut inner, "mainnet-beta", start).is_none(),
            "another cluster is another policy"
        );
        assert!(
            cached_config_in(&mut inner, "devnet", start + CONFIG_TTL).is_none(),
            "an expired answer is a miss"
        );
        assert!(
            inner.is_empty(),
            "the expired entry is dropped where it is found"
        );
    }

    /// A cluster that answers with a policy and rents is read once: one account
    /// read and one rent per account size the quote needs.
    #[tokio::test]
    async fn reads_the_policy_and_every_rent_from_the_cluster() {
        let data = policy_bytes();
        let encoded = base64_encode(&data);
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let app = axum::Router::new().route(
            "/",
            axum::routing::post(move |axum::Json(body): axum::Json<serde_json::Value>| {
                let counter = counter.clone();
                let encoded = encoded.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    let id = body.get("id").cloned().unwrap_or_default();
                    let answer = match body.get("method").and_then(serde_json::Value::as_str) {
                        Some("getMinimumBalanceForRentExemption") => {
                            let space = body["params"][0].as_u64().unwrap_or(0);
                            serde_json::json!({"jsonrpc": "2.0", "id": id, "result": space * 1_000})
                        }
                        Some("getAccountInfo") => serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {"context": {"slot": 1}, "value": {
                                "data": [encoded, "base64"],
                                "executable": false,
                                "lamports": 2_000_000,
                                "owner": payments::PAYVAND_PROGRAM_ID,
                                "rentEpoch": 0,
                            }},
                        }),
                        _ => serde_json::json!({"jsonrpc": "2.0", "id": id, "result": null}),
                    };
                    axum::Json(answer)
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("address");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let endpoint = format!("http://{addr}/");
        let config = read_config(&endpoint, "devnet")
            .await
            .expect("the cluster answered");

        assert_eq!(config.fees.fee_bps_below, 200);
        assert_eq!(config.rent.payment, 230_000);
        assert_eq!(config.rent.token_payment, 262_000);
        assert_eq!(config.rent.payment_receipt, 48_000);
        assert_eq!(config.rent.token_account, 165_000);
        assert_eq!(config.rent.budget, 97_000);
        assert_eq!(config.rent.token_budget, 129_000);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            7,
            "one account read and six rents"
        );
    }

    /// A cluster that does not answer is an error, not a policy of zeroes.
    #[tokio::test]
    async fn a_cluster_that_does_not_answer_is_an_error() {
        let app = axum::Router::new().route(
            "/",
            axum::routing::post(
                |axum::Json(body): axum::Json<serde_json::Value>| async move {
                    let id = body.get("id").cloned().unwrap_or_default();
                    axum::Json(serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {"code": -32005, "message": "node is unhealthy"},
                    }))
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("address");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        assert!(
            read_config(&format!("http://{addr}/"), "devnet")
                .await
                .is_err(),
            "no policy means no quote"
        );
    }

    fn base64_encode(bytes: &[u8]) -> String {
        use base64::{engine::general_purpose::STANDARD, Engine};
        STANDARD.encode(bytes)
    }
}
