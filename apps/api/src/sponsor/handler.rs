//! `GET /api/sponsor`, `POST /api/sponsor/prepare` and `POST /api/sponsor/settle`.
//!
//! The relayer signs first. `prepare` builds the settle from the invoice's
//! fields, signs it as fee payer and hands it to the page; the buyer's wallet
//! then signs a transaction that is complete except for its own signature, which
//! is the case a wallet can simulate and preview as the price it is. `settle`
//! takes that transaction back, checks it is byte for byte the one the relayer
//! signed, and sends it. See `docs/payment-costs.md`.

use axum::extract::{Json, Query, State};
use axum::http::HeaderMap;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use solana_sdk::hash::Hash;
use solana_sdk::message::VersionedMessage;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signature};
use solana_sdk::signer::Signer;
use solana_sdk::transaction::VersionedTransaction;

use crate::auth;
use crate::payments::{bad, resolve_endpoint, ApiError, ApiResult};
use crate::AppState;

use super::diagnostics::{first_difference, logged, mint_label, shape};
use super::message::{expected_message, legacy_message, message_bytes};
use super::model::{
    InfoQuery, InfoResponse, PrepareRequest, PrepareResponse, SettleRequest, SettleResponse,
};
use super::params::Params;
use super::relayer::{cap_from_env, sponsor_keypair, unavailable, SPONSORED_LAMPORTS};
use super::store::{mark_submitted, recent_count, record_signed, was_signed};

fn per_wallet_cap() -> u32 {
    cap_from_env("SPONSOR_DAILY_PER_WALLET", 10)
}

fn total_cap() -> u32 {
    cap_from_env("SPONSOR_DAILY_TOTAL", 200)
}

/// What the client needs before it builds anything: whether a sponsor exists,
/// which address it pays from, and whether this wallet still has its daily
/// allowance. Readable without a session, because the buyer may not have one;
/// with no wallet named at all it answers for the deployment.
pub async fn info(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<InfoQuery>,
) -> ApiResult<Json<InfoResponse>> {
    let user = auth::optional_user(&state.db, &headers).await;
    let per_wallet = per_wallet_cap();
    let sponsor = sponsor_keypair();
    let wallet = query
        .wallet
        .as_deref()
        .map(str::trim)
        .filter(|wallet| !wallet.is_empty())
        .map(str::to_owned)
        .or_else(|| user.and_then(|user| user.wallet));
    let mut available = sponsor.is_some() && recent_count(&state, None).await? < total_cap() as i64;
    if available {
        if let Some(wallet) = wallet.as_deref() {
            available = recent_count(&state, Some(wallet)).await? < per_wallet as i64;
        }
    }
    Ok(Json(InfoResponse {
        available,
        address: sponsor.as_ref().map(|key| key.pubkey().to_string()),
        needs_lamports: SPONSORED_LAMPORTS,
        per_wallet_daily: per_wallet,
    }))
}

/// Both daily caps, checked before the relayer signs anything: once it has, the
/// transaction can reach the cluster without us.
async fn ensure_within_caps(state: &AppState, wallet: &str) -> ApiResult<()> {
    let per_wallet = per_wallet_cap();
    if recent_count(state, Some(wallet)).await? >= per_wallet as i64 {
        return Err(ApiError::TooManyRequests(format!(
            "This wallet has used today's {per_wallet} sponsored payments. Add a little SOL to it to keep paying."
        )));
    }
    if recent_count(state, None).await? >= total_cap() as i64 {
        return Err(unavailable());
    }
    Ok(())
}

/// Signs `message` as the relayer and records the signature before it leaves
/// this process. The buyer's slot is left empty for the wallet.
async fn sign_and_record(
    state: &AppState,
    user_id: Option<&str>,
    sponsor: &Keypair,
    params: &Params,
    cluster: &str,
    message: VersionedMessage,
) -> ApiResult<(VersionedTransaction, Signature)> {
    let signature = sponsor.sign_message(&message_bytes(&message)?);
    record_signed(
        state,
        user_id,
        &params.buyer.to_string(),
        cluster,
        params.amount,
        &signature.to_string(),
    )
    .await?;
    Ok((
        VersionedTransaction {
            signatures: vec![signature, Signature::default()],
            message,
        },
        signature,
    ))
}

async fn latest_blockhash(endpoint: &str) -> ApiResult<Hash> {
    let rpc = solana_rpc_client::nonblocking::rpc_client::RpcClient::new(endpoint.to_owned());
    rpc.get_latest_blockhash().await.map_err(|error| {
        tracing::info!(%error, "latest blockhash unavailable for a sponsored settle");
        ApiError::Internal("The network did not return a blockhash. Try again.".into())
    })
}

fn read_params(fields: &super::model::SettleFields) -> ApiResult<Params> {
    // A request naming nonsense is a bad request rather than somebody's spend.
    // The refusal is logged — the caller gets a sentence, and the fields an
    // operator needs to tell a client bug from a probing caller stay here.
    Params::read(fields).map_err(|error| {
        tracing::warn!(
            %error,
            cluster = %logged(&fields.cluster),
            buyer = %logged(&fields.buyer),
            seller = %logged(&fields.seller),
            reference = %logged(&fields.reference),
            mint = %logged(&fields.mint),
            amount = fields.amount,
            memo_len = fields.memo.len(),
            "a sponsored settle named an invoice that cannot exist"
        );
        error
    })
}

/// `POST /api/sponsor/prepare` — the settle for an invoice, built here and
/// signed by the relayer as fee payer.
///
/// No session and no buyer signature: the buyer may have scanned a printed code
/// and own no account, and the transaction this returns cannot do anything until
/// the buyer's wallet signs it. What the relayer risks by signing first is the
/// fee of a transaction that then fails on chain, which nothing refunds; the
/// deployment's daily cap counts every signature handed out, which bounds it.
pub async fn prepare(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PrepareRequest>,
) -> ApiResult<Json<PrepareResponse>> {
    let user = auth::optional_user(&state.db, &headers).await;
    let sponsor = sponsor_keypair().ok_or_else(unavailable)?;
    let params = read_params(&request)?;
    ensure_within_caps(&state, &params.buyer.to_string()).await?;

    let endpoint = resolve_endpoint(request.endpoint.as_deref(), &request.cluster)?;
    let blockhash = latest_blockhash(&endpoint).await?;
    let message = expected_message(&params, &sponsor.pubkey(), blockhash)?;
    let (transaction, signature) = sign_and_record(
        &state,
        user.as_ref().map(|user| user.id.as_str()),
        &sponsor,
        &params,
        &request.cluster,
        message,
    )
    .await?;
    let wire = bincode::serialize(&transaction)
        .map_err(|_| ApiError::Internal("the settle could not be serialized".into()))?;
    Ok(Json(PrepareResponse {
        transaction: STANDARD.encode(wire),
        signature: signature.to_string(),
    }))
}

/// `POST /api/sponsor/settle` — sends a prepared settle once the buyer signed.
///
/// Only a transaction whose message is byte for byte the one `prepare` built for
/// the same fields, carrying the relayer's signature from `prepare` and a valid
/// buyer signature, is sent. Anything else — a wallet that changed the message,
/// a different invoice, a transaction the relayer never signed — is refused
/// here rather than by the cluster.
pub async fn settle(
    State(state): State<AppState>,
    Json(request): Json<SettleRequest>,
) -> ApiResult<Json<SettleResponse>> {
    let sponsor = sponsor_keypair().ok_or_else(unavailable)?;
    let fields = &request.fields;
    let params = read_params(fields)?;
    let wallet = params.buyer.to_string();

    let wire = STANDARD
        .decode(request.transaction.trim())
        .map_err(|error| {
            tracing::warn!(
                %error,
                cluster = %logged(&fields.cluster),
                buyer = %params.buyer,
                bytes = request.transaction.len(),
                "a sponsored settle's transaction was not base64"
            );
            bad("That transaction is not valid base64.")
        })?;
    let transaction: VersionedTransaction = bincode::deserialize(&wire).map_err(|error| {
        tracing::warn!(
            %error,
            cluster = %logged(&fields.cluster),
            buyer = %params.buyer,
            bytes = wire.len(),
            "a sponsored settle's transaction could not be decoded"
        );
        bad("That transaction could not be read.")
    })?;
    let relayer_signature = check_prepared(&params, &sponsor, &transaction, &fields.cluster)?;
    // `prepare` recorded every signature it handed out; one it did not is a
    // settle this relayer never agreed to.
    if !was_signed(&state, &relayer_signature.to_string()).await? {
        return Err(bad("This settle was not prepared by this relayer."));
    }

    // A person pays once or twice a minute at most. Keyed by the wallet only
    // now that the wallet's own signature proves the caller holds it.
    crate::rate_limit::hit("sponsor-wallet", &wallet, 6, crate::rate_limit::MINUTE)?;

    let endpoint = resolve_endpoint(fields.endpoint.as_deref(), &fields.cluster)?;
    let rpc = solana_rpc_client::nonblocking::rpc_client::RpcClient::new_with_commitment(
        endpoint.clone(),
        solana_commitment_config::CommitmentConfig::confirmed(),
    );
    // The cluster's own words: a preflight failure, an expired blockhash and a
    // program error all read differently here, and the buyer's screen says only
    // that the payment was not accepted.
    let sent = rpc.send_transaction(&transaction).await.map_err(|error| {
        tracing::warn!(
            %error,
            details = ?error,
            cluster = %logged(&fields.cluster),
            endpoint = %endpoint,
            buyer = %params.buyer,
            seller = %params.seller,
            amount = params.amount,
            mint = %mint_label(&params),
            sponsor_signature = %relayer_signature,
            "the cluster refused a sponsored settle"
        );
        ApiError::BadRequest(format!("The sponsored payment was not accepted: {error}"))
    })?;
    mark_submitted(&state, &sent.to_string()).await?;

    Ok(Json(SettleResponse {
        signature: sent.to_string(),
    }))
}

/// Whether `transaction` is a settle the relayer prepared for `params`, fully
/// signed: its message is byte for byte `expected_message` at its own
/// blockhash, the first signature is the relayer's over exactly those bytes, and
/// the buyer's verifies. Returns the relayer's signature, which is the
/// transaction's id and the key `prepare` recorded it under.
pub(super) fn check_prepared(
    params: &Params,
    sponsor: &Keypair,
    transaction: &VersionedTransaction,
    cluster: &str,
) -> ApiResult<Signature> {
    if transaction.signatures.len() != 2 {
        return Err(bad(
            "A sponsored settle carries the relayer's and the buyer's signatures.",
        ));
    }

    // The message the relayer signed for these fields at this blockhash. Both
    // were compiled by `expected_message`, so equal fields give equal bytes and
    // any difference at all is a different transaction.
    let expected = expected_message(
        params,
        &sponsor.pubkey(),
        *transaction.message.recent_blockhash(),
    )?;
    let ours = message_bytes(&expected)?;
    let theirs = message_bytes(&transaction.message)?;
    if ours != theirs {
        // Either the wallet rewrote the prepared transaction (which also broke
        // the relayer's signature) or the request names another invoice. The
        // shapes and the first differing byte say which.
        let (our_accounts, our_instruction_accounts, our_data) = shape(&expected);
        let (their_accounts, their_instruction_accounts, their_data) = shape(&transaction.message);
        tracing::warn!(
            cluster = %logged(cluster),
            buyer = %params.buyer,
            seller = %params.seller,
            amount = params.amount,
            mint = %mint_label(params),
            memo_len = params.memo.len(),
            accounts = their_accounts,
            expected_accounts = our_accounts,
            instruction_accounts = their_instruction_accounts,
            expected_instruction_accounts = our_instruction_accounts,
            data_len = their_data,
            expected_data_len = our_data,
            first_difference = ?first_difference(&ours, &theirs),
            "a sponsored settle was not the transaction the relayer prepared"
        );
        return Err(bad(
            "This is not the transaction prepared for this invoice. If your wallet changed it (for example by adding a priority fee), turn that off and try again.",
        ));
    }

    // Ed25519 signatures are deterministic, so the relayer's signature over
    // these bytes is the one `prepare` produced — and `prepare` recorded it.
    let relayer_signature = sponsor.sign_message(&ours);
    if transaction.signatures[0] != relayer_signature {
        return Err(bad("This settle was not prepared by this relayer."));
    }
    let verified = transaction.verify_with_results();
    if verified.len() != 2 || !verified.iter().all(|ok| *ok) {
        return Err(bad(
            "The buyer's signature does not match this transaction.",
        ));
    }

    Ok(relayer_signature)
}

/// The invoice a Solana Pay wallet asked to pay, for [`solana_pay_settle`].
pub struct SolanaPaySettle<'a> {
    pub buyer: Pubkey,
    pub seller: Pubkey,
    pub reference: Pubkey,
    pub amount: u64,
    pub memo: &'a str,
    pub cluster: &'a str,
    pub blockhash: Hash,
}

/// The SOL settle a Solana Pay wallet signs and sends itself, with the relayer as
/// fee payer and its signature already on it — legacy wire bytes, or `None` when
/// no relayer will pay (none configured, or a cap reached), in which case the
/// caller hands the wallet its own self-paid settle instead.
///
/// The wallet sends this one, not us, so it is never marked submitted: it counts
/// toward the deployment's cap only.
pub async fn solana_pay_settle(state: &AppState, request: &SolanaPaySettle<'_>) -> Option<Vec<u8>> {
    let sponsor = sponsor_keypair()?;
    let params = Params {
        buyer: request.buyer,
        seller: request.seller,
        reference: request.reference,
        mint: None,
        amount: request.amount,
        memo: request.memo.to_owned(),
    };
    if ensure_within_caps(state, &params.buyer.to_string())
        .await
        .is_err()
    {
        return None;
    }
    let message = legacy_message(&params, &sponsor.pubkey(), request.blockhash);
    let (transaction, _) =
        sign_and_record(state, None, &sponsor, &params, request.cluster, message)
            .await
            .map_err(|error| tracing::warn!(?error, "a Solana Pay settle could not be sponsored"))
            .ok()?;
    bincode::serialize(&transaction).ok()
}

/// The fallback for [`solana_pay_settle`]: the same settle with the wallet as
/// its own sponsor, unsigned, in legacy wire format with the wallet's one
/// signature slot empty.
///
/// The program refunds a self-sponsoring wallet both rents and one signature's
/// network fee in the same instruction, so the buyer still spends the price.
/// The compute-unit limit is what keeps the wallet from adding a priority price
/// on top — the one cost nothing refunds.
pub fn self_paid_solana_pay_settle(request: &SolanaPaySettle<'_>) -> ApiResult<Vec<u8>> {
    let params = Params {
        buyer: request.buyer,
        seller: request.seller,
        reference: request.reference,
        mint: None,
        amount: request.amount,
        memo: request.memo.to_owned(),
    };
    let message = legacy_message(&params, &request.buyer, request.blockhash);
    bincode::serialize(&VersionedTransaction {
        signatures: vec![Signature::default()],
        message,
    })
    .map_err(|_| ApiError::Internal("the settle could not be serialized".into()))
}

#[cfg(test)]
mod tests;
