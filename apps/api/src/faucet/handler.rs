//! `GET` and `POST /api/faucet`.

use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use serde::Serialize;
use solana_pubkey::Pubkey;
use solana_sdk::signer::Signer;
use std::str::FromStr;

use crate::auth;
use crate::keypair_from_env;
use crate::payments::{self, bad, ApiError, ApiResult};
use crate::spl::{ata_for, create_ata_ix, sol_transfer_ix, token_transfer_ix};
use crate::AppState;

use super::amounts::{hands_out, sol_lamports, token_base_units};
use super::claims::{claimed, record, release, reserve};
use super::send::send;

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

    // The treasury pays the rent for any token account it creates, which is why
    // the claim is one transaction: the tokens arrive and the account exists
    // when it lands.
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
