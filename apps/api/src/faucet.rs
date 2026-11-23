//! Starter tokens for a new wallet, from the deployment's own treasury.
//!
//! Payvand runs on a test network, and a wallet with an empty balance cannot try
//! anything: every payment needs a little SOL for the network fee, and the
//! stablecoins have to be in the wallet before the checkout can spend them. The
//! public faucets work, but they are another site and another set of addresses to
//! copy, which is exactly where someone trying the app for the first time stops.
//!
//! So one claim per account, once: 1 SOL and 10 USDC and 10 EURC, sent by the
//! server's treasury key. Only on devnet — a mainnet treasury behind this endpoint
//! would be a way to drain real money — and only to the wallet the account has
//! linked, so the tokens land where the app will spend them.

use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use serde::Serialize;
use solana_commitment_config::CommitmentConfig;
use solana_pubkey::Pubkey;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::signature::Signature;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::transaction::Transaction;
use std::str::FromStr;
use std::time::Duration;

use crate::auth;
use crate::keypair_from_env;
use crate::payments::{self, bad, ApiError, ApiResult};
use crate::AppState;

const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ATA_PROGRAM: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";

/// 1 SOL, which covers hundreds of network fees and the small rents a wallet pays
/// for its own token accounts.
const DEFAULT_SOL_LAMPORTS: u64 = 1_000_000_000;
/// 10 tokens, six decimals: enough for several invoices in either stablecoin.
const DEFAULT_TOKEN_BASE_UNITS: u64 = 10 * 1_000_000;

/// What the endpoint hands out, so the card can say the amounts without copying
/// them from the server's constants.
#[derive(Debug, Serialize)]
pub struct FaucetStatus {
    /// True when this account can claim right now.
    pub claimable: bool,
    pub claimed: bool,
    /// The wallet the tokens would go to, when one is linked.
    pub wallet: Option<String>,
    pub cluster: String,
    pub sol_lamports: u64,
    pub token_base_units: u64,
    /// Why `claimable` is false, when it is: linked wallet missing, already
    /// claimed, mainnet, or an unconfigured treasury.
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ClaimResponse {
    pub signature: String,
    pub sol_lamports: u64,
    pub token_base_units: u64,
}

/// Start tokens are a test-network feature, full stop. A local validator is the
/// other place they make sense; everything else is a real chain.
fn hands_out(cluster: &str) -> bool {
    matches!(cluster, "devnet" | "localnet")
}

fn amount_from_env(name: &str, fallback: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

fn sol_lamports() -> u64 {
    amount_from_env("FAUCET_SOL_LAMPORTS", DEFAULT_SOL_LAMPORTS)
}

fn token_base_units() -> u64 {
    amount_from_env("FAUCET_TOKEN_BASE_UNITS", DEFAULT_TOKEN_BASE_UNITS)
}

/// A lamport transfer: the system program's `Transfer` instruction, which is the
/// tag `2` followed by the amount in little-endian bytes. Written out rather
/// than imported, because the SDK this API pins no longer re-exports the system
/// program's builders.
fn sol_transfer_ix(from: &Pubkey, to: &Pubkey, lamports: u64) -> Instruction {
    let mut data = Vec::with_capacity(12);
    data.extend_from_slice(&2u32.to_le_bytes());
    data.extend_from_slice(&lamports.to_le_bytes());
    Instruction {
        program_id: Pubkey::from_str(SYSTEM_PROGRAM).expect("the system program id parses"),
        accounts: vec![AccountMeta::new(*from, true), AccountMeta::new(*to, false)],
        data,
    }
}

/// The owner's associated token account under the classic token program, which
/// is the only program the app's invoices accept.
fn ata_for(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    let token = Pubkey::from_str(TOKEN_PROGRAM).expect("the token program id parses");
    let program = Pubkey::from_str(ATA_PROGRAM).expect("the ATA program id parses");
    Pubkey::find_program_address(&[owner.as_ref(), token.as_ref(), mint.as_ref()], &program).0
}

/// "Create the account if it does not exist", so a wallet that already holds the
/// token is not an error. The treasury pays the rent, which is why the claim is
/// one transaction: the tokens arrive and the account exists when it lands.
fn create_ata_ix(payer: &Pubkey, owner: &Pubkey, mint: &Pubkey) -> Instruction {
    Instruction {
        program_id: Pubkey::from_str(ATA_PROGRAM).expect("the ATA program id parses"),
        accounts: vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new(ata_for(owner, mint), false),
            AccountMeta::new_readonly(*owner, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new_readonly(
                Pubkey::from_str(SYSTEM_PROGRAM).expect("the system program id parses"),
                false,
            ),
            AccountMeta::new_readonly(
                Pubkey::from_str(TOKEN_PROGRAM).expect("the token program id parses"),
                false,
            ),
        ],
        // `AssociatedTokenAccountInstruction::CreateIdempotent`
        data: vec![1],
    }
}

/// `TokenInstruction::Transfer`, which is the instruction tag then the amount in
/// little-endian base units.
fn token_transfer_ix(
    source: &Pubkey,
    destination: &Pubkey,
    authority: &Pubkey,
    amount: u64,
) -> Instruction {
    let mut data = Vec::with_capacity(9);
    data.push(3);
    data.extend_from_slice(&amount.to_le_bytes());
    Instruction {
        program_id: Pubkey::from_str(TOKEN_PROGRAM).expect("the token program id parses"),
        accounts: vec![
            AccountMeta::new(*source, false),
            AccountMeta::new(*destination, false),
            AccountMeta::new_readonly(*authority, true),
        ],
        data,
    }
}

async fn claimed(state: &AppState, user_id: &str) -> ApiResult<bool> {
    let connection = state.db.connect().map_err(auth::internal)?;
    let mut rows = connection
        .query(
            "SELECT 1 FROM faucet_claims WHERE user_id = ?1",
            [user_id.to_string()],
        )
        .await
        .map_err(auth::internal)?;
    Ok(rows.next().await.map_err(auth::internal)?.is_some())
}

/// `GET /api/faucet` — whether this account can claim, and for how much.
pub async fn status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<FaucetStatus>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let cluster = payments::default_cluster();
    let claimed = claimed(&state, &user.id).await?;
    let treasury = keypair_from_env("FAUCET_KEYPAIR");
    let wallet = user.wallet.filter(|address| !address.trim().is_empty());

    let reason = if !hands_out(&cluster) {
        Some("Starter tokens are only for the devnet build of Payvand.".to_string())
    } else if treasury.is_none() {
        Some("Starter tokens are not configured on this server yet.".to_string())
    } else if claimed {
        Some("This account has already claimed its starter tokens.".to_string())
    } else if wallet.is_none() {
        Some("Link a wallet first, and the tokens go straight to it.".to_string())
    } else {
        None
    };

    Ok(Json(FaucetStatus {
        claimable: reason.is_none(),
        claimed,
        wallet,
        cluster,
        sol_lamports: sol_lamports(),
        token_base_units: token_base_units(),
        reason,
    }))
}

/// `POST /api/faucet` — sends the starter tokens to the linked wallet, once.
///
/// The row is reserved before anything is sent: two taps in the same second are
/// one claim, and a claim that the cluster never accepted is released so the
/// person can try again.
pub async fn claim(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<ClaimResponse>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let cluster = payments::default_cluster();
    if !hands_out(&cluster) {
        return Err(ApiError::Forbidden(
            "Starter tokens are only for the devnet build of Payvand.".into(),
        ));
    }
    // A deployment without a treasury is a misconfiguration, not the caller's
    // problem: the card never offers the button in that case, so this is the
    // guard behind it, logged where an operator will see it.
    let treasury = keypair_from_env("FAUCET_KEYPAIR").ok_or_else(|| {
        ApiError::Internal("FAUCET_KEYPAIR is not set, so no claim can be sent.".into())
    })?;
    let wallet_address = user
        .wallet
        .clone()
        .filter(|address| !address.trim().is_empty())
        .ok_or_else(|| bad("Link a wallet first, and the tokens go straight to it."))?;
    let wallet = Pubkey::from_str(wallet_address.trim())
        .map_err(|_| bad("The linked wallet is not a valid Solana address."))?;

    reserve(&state, &user.id, &wallet_address, &cluster).await?;

    let sol = sol_lamports();
    let tokens = token_base_units();
    let sender = treasury.pubkey();

    let mut instructions = vec![sol_transfer_ix(&sender, &wallet, sol)];
    for currency in ["usdc", "eurc"] {
        let Some(mint) = payments::mint_for(currency, &cluster) else {
            continue;
        };
        instructions.push(create_ata_ix(&sender, &wallet, &mint));
        instructions.push(token_transfer_ix(
            &ata_for(&sender, &mint),
            &ata_for(&wallet, &mint),
            &sender,
            tokens,
        ));
    }

    let signature = match send(&cluster, &treasury, instructions).await {
        Ok(signature) => signature,
        Err(error) => {
            // Nothing landed, so the claim is not spent.
            release(&state, &user.id).await;
            return Err(error);
        }
    };

    record(&state, &user.id, &signature).await?;
    Ok(Json(ClaimResponse {
        signature,
        sol_lamports: sol,
        token_base_units: tokens,
    }))
}

/// Builds, signs and confirms the transfer transaction, and answers with the
/// signature the cluster accepted.
///
/// The signature is checked rather than assumed: a send that errors can still
/// have reached the cluster, and a transfer that landed while the caller was
/// told it failed is a second claim paying twice. So a failure is only reported
/// after the cluster has been asked and says it has no such transaction, or the
/// transaction is there and failed.
async fn send(
    cluster: &str,
    treasury: &Keypair,
    instructions: Vec<Instruction>,
) -> ApiResult<String> {
    let rpc = RpcClient::new_with_commitment(
        payments::resolve_endpoint(None, cluster)?,
        CommitmentConfig::confirmed(),
    );
    let blockhash = rpc
        .get_latest_blockhash()
        .await
        .map_err(|error| ApiError::Internal(format!("The cluster did not answer: {error}")))?;
    let transaction = Transaction::new_signed_with_payer(
        &instructions,
        Some(&treasury.pubkey()),
        &[treasury],
        blockhash,
    );
    let Some(signature) = transaction.signatures.first().copied() else {
        return Err(ApiError::Internal(
            "The starter token transaction was not signed.".into(),
        ));
    };

    let sent = rpc.send_transaction(&transaction).await;
    if let Err(error) = &sent {
        // Preflight rejections are the common case here (an empty treasury, a
        // frozen token account). Give the cluster a moment to prove otherwise.
        tracing::warn!(%error, "starter token transfer was not accepted");
    }
    // A send that went through is worth half a minute of waiting; one that
    // errored gets three tries to prove it landed anyway, then counts as failed.
    let waits = if sent.is_ok() { 15 } else { 3 };
    if let Some(verdict) = settle(&rpc, &signature, waits).await {
        return verdict;
    }

    // Accepted but never seen: keep the claim (the caller records this
    // signature) rather than risk paying the same account twice.
    tracing::warn!(%signature, "starter token transfer was not confirmed in time");
    Ok(signature.to_string())
}

/// Waits for the cluster's verdict on a submitted transaction: `None` when it
/// never saw one, otherwise the claim's answer.
async fn settle(rpc: &RpcClient, signature: &Signature, waits: usize) -> Option<ApiResult<String>> {
    for _ in 0..waits {
        match rpc.get_signature_statuses(&[*signature]).await {
            Ok(statuses) => {
                if let Some(status) = statuses.value.into_iter().next().flatten() {
                    return Some(match status.err {
                        None => Ok(signature.to_string()),
                        Some(error) => {
                            tracing::error!(%error, %signature, "starter token transfer failed");
                            Err(ApiError::Internal(
                                "The starter tokens could not be sent. Try again in a moment."
                                    .into(),
                            ))
                        }
                    });
                }
            }
            Err(error) => tracing::warn!(%error, "signature status read failed"),
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    None
}

/// Claims the slot before sending, so a double tap cannot send twice.
async fn reserve(state: &AppState, user_id: &str, wallet: &str, cluster: &str) -> ApiResult<()> {
    let connection = state.db.connect().map_err(auth::internal)?;
    let inserted = connection
        .execute(
            "INSERT INTO faucet_claims \
             (user_id, wallet, cluster, sol_lamports, token_base_units, signature, claimed_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, '', ?6)",
            (
                user_id.to_string(),
                wallet.to_string(),
                cluster.to_string(),
                sol_lamports() as i64,
                token_base_units() as i64,
                auth::unix_now(),
            ),
        )
        .await;
    match inserted {
        Ok(_) => Ok(()),
        // Only a primary-key clash means "already claimed"; a database that is
        // down or a broken migration is not something to blame on the caller.
        Err(error) if error.to_string().to_ascii_lowercase().contains("unique") => Err(
            ApiError::Forbidden("This account has already claimed its starter tokens.".into()),
        ),
        Err(error) => {
            let message = error.to_string();
            tracing::error!(%message, "could not reserve a starter-token claim");
            Err(ApiError::Internal(
                "The starter tokens could not be prepared. Try again in a moment.".into(),
            ))
        }
    }
}

async fn record(state: &AppState, user_id: &str, signature: &str) -> ApiResult<()> {
    let connection = state.db.connect().map_err(auth::internal)?;
    connection
        .execute(
            "UPDATE faucet_claims SET signature = ?2, claimed_at = ?3 WHERE user_id = ?1",
            (user_id.to_string(), signature.to_string(), auth::unix_now()),
        )
        .await
        .map_err(auth::internal)?;
    Ok(())
}

async fn release(state: &AppState, user_id: &str) {
    let Ok(connection) = state.db.connect() else {
        return;
    };
    let _ = connection
        .execute(
            "DELETE FROM faucet_claims WHERE user_id = ?1 AND signature = ''",
            [user_id.to_string()],
        )
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: &str) -> Pubkey {
        Pubkey::from_str(text).expect("valid address")
    }

    #[test]
    fn hands_out_only_test_network_tokens() {
        assert!(hands_out("devnet"));
        assert!(hands_out("localnet"));
        assert!(!hands_out("mainnet"));
        assert!(!hands_out("mainnet-beta"));
        assert!(!hands_out("testnet"));
    }

    #[test]
    fn moves_the_named_lamports_to_the_linked_wallet() {
        let from = key("6dQgFZDTkhDMTZv9ftZe4ufU9jG5fvJEw6zpw6WCKjPz");
        let to = key("3rpBzUBbZPXFKhGywCRduD3hrREDjvDtaw2Mos2yUUPx");
        let instruction = sol_transfer_ix(&from, &to, 1_000_000_000);

        assert_eq!(instruction.program_id, key(SYSTEM_PROGRAM));
        assert_eq!(&instruction.data[..4], &2u32.to_le_bytes());
        assert_eq!(&instruction.data[4..], &1_000_000_000u64.to_le_bytes());
        assert_eq!(instruction.accounts[0], AccountMeta::new(from, true));
        assert_eq!(instruction.accounts[1], AccountMeta::new(to, false));
    }

    #[test]
    fn transfers_the_named_amount_in_little_endian() {
        let source = key("6dQgFZDTkhDMTZv9ftZe4ufU9jG5fvJEw6zpw6WCKjPz");
        let destination = key("3rpBzUBbZPXFKhGywCRduD3hrREDjvDtaw2Mos2yUUPx");
        let instruction = token_transfer_ix(&source, &destination, &source, 10_000_000);

        assert_eq!(instruction.program_id, key(TOKEN_PROGRAM));
        assert_eq!(instruction.data[0], 3);
        assert_eq!(instruction.data.len(), 9);
        assert_eq!(&instruction.data[1..], &10_000_000u64.to_le_bytes());
        assert_eq!(
            instruction.accounts,
            vec![
                AccountMeta::new(source, false),
                AccountMeta::new(destination, false),
                AccountMeta::new_readonly(source, true),
            ]
        );
    }

    #[test]
    fn creates_the_token_account_only_when_it_is_missing() {
        let payer = key("6dQgFZDTkhDMTZv9ftZe4ufU9jG5fvJEw6zpw6WCKjPz");
        let mint = key("4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU");
        let owner = key("3rpBzUBbZPXFKhGywCRduD3hrREDjvDtaw2Mos2yUUPx");
        let instruction = create_ata_ix(&payer, &owner, &mint);

        assert_eq!(instruction.program_id, key(ATA_PROGRAM));
        // `CreateIdempotent`, not `Create`: a wallet that already holds USDC must
        // not make the whole claim fail.
        assert_eq!(instruction.data, vec![1]);
        assert_eq!(instruction.accounts[1].pubkey, ata_for(&owner, &mint));
        assert!(instruction.accounts[0].is_signer);
        assert!(instruction.accounts[0].is_writable);
    }

    #[test]
    fn derives_the_same_token_account_the_program_would() {
        // ATA = PDA(["<owner>", "<token program>", "<mint>"], ATA program); the
        // example is the treasury's devnet USDC account from the running app.
        let owner = key("6dQgFZDTkhDMTZv9ftZe4ufU9jG5fvJEw6zpw6WCKjPz");
        let mint = key("4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU");
        let derived = ata_for(&owner, &mint);
        assert_eq!(derived, ata_for(&owner, &mint));
        assert_ne!(
            derived,
            ata_for(&owner, &key("HzwqbKZw8HxMN6bF2yFZNrht3c2iXXzpKcFu7uBEDKtr"))
        );
    }

    #[test]
    fn reads_the_amounts_from_the_environment_or_keeps_the_defaults() {
        std::env::remove_var("FAUCET_SOL_LAMPORTS");
        std::env::remove_var("FAUCET_TOKEN_BASE_UNITS");
        assert_eq!(sol_lamports(), DEFAULT_SOL_LAMPORTS);
        assert_eq!(token_base_units(), DEFAULT_TOKEN_BASE_UNITS);

        std::env::set_var("FAUCET_SOL_LAMPORTS", "0");
        assert_eq!(sol_lamports(), DEFAULT_SOL_LAMPORTS);
        std::env::set_var("FAUCET_SOL_LAMPORTS", "250000000");
        std::env::set_var("FAUCET_TOKEN_BASE_UNITS", "5000000");
        assert_eq!(sol_lamports(), 250_000_000);
        assert_eq!(token_base_units(), 5_000_000);
        std::env::remove_var("FAUCET_SOL_LAMPORTS");
        std::env::remove_var("FAUCET_TOKEN_BASE_UNITS");
    }
}
