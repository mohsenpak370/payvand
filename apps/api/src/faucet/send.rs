//! Signing, sending and confirming the starter-token transaction.

use solana_commitment_config::CommitmentConfig;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::instruction::Instruction;
use solana_sdk::signature::Signature;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::transaction::Transaction;
use std::time::Duration;

use crate::payments::{self, ApiError, ApiResult};

/// Builds, signs and confirms the transfer transaction, and answers with the
/// signature the cluster accepted.
///
/// The signature is checked rather than assumed: a send that errors can still
/// have reached the cluster, and a transfer that landed while the caller was
/// told it failed is a second claim paying twice. So a failure is only reported
/// after the cluster has been asked and says it has no such transaction, or the
/// transaction is there and failed.
pub(super) async fn send(
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
