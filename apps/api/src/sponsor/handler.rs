//! `GET /api/sponsor` and `POST /api/sponsor/settle`.

use axum::extract::{Json, Query, State};
use axum::http::HeaderMap;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use solana_sdk::signature::Signature;
use solana_sdk::signer::Signer;
use solana_sdk::transaction::VersionedTransaction;

use crate::auth;
use crate::payments::{bad, resolve_endpoint, ApiError, ApiResult};
use crate::AppState;

use super::diagnostics::{first_difference, logged, mint_label, shape};
use super::message::{expected_message, message_bytes, same_settlement};
use super::model::{InfoQuery, InfoResponse, SettleRequest, SettleResponse};
use super::params::Params;
use super::relayer::{cap_from_env, sponsor_keypair, unavailable, SPONSORED_LAMPORTS};
use super::store::{recent_count, record};

/// What the client needs before it builds anything: whether a sponsor exists,
/// which address to name as fee payer, and whether this wallet still has its
/// daily allowance. Readable without a session, because the buyer may not have
/// one; with no wallet named at all it answers for the deployment, and the
/// settle endpoint still enforces the wallet's own cap.
pub async fn info(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<InfoQuery>,
) -> ApiResult<Json<InfoResponse>> {
    let user = auth::optional_user(&state.db, &headers).await;
    let per_wallet = cap_from_env("SPONSOR_DAILY_PER_WALLET", 10);
    let sponsor = sponsor_keypair();
    let wallet = query
        .wallet
        .as_deref()
        .map(str::trim)
        .filter(|wallet| !wallet.is_empty())
        .map(str::to_owned)
        .or_else(|| user.and_then(|user| user.wallet));
    let mut available = sponsor.is_some()
        && recent_count(&state, None).await? < cap_from_env("SPONSOR_DAILY_TOTAL", 200) as i64;
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

pub async fn settle(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SettleRequest>,
) -> ApiResult<Json<SettleResponse>> {
    // No session required: the buyer's signature is the authority, and a printed
    // code is often paid by someone who never signed in. The account, when there
    // is one, is recorded because we know it — not because the cap needs it.
    let user = auth::optional_user(&state.db, &headers).await;
    let sponsor = sponsor_keypair().ok_or_else(unavailable)?;
    let sponsor_key = sponsor.pubkey();

    // The caps are keyed by the wallet that signs, and it has to be a wallet
    // before it can be a key: a request naming nonsense is a bad request rather
    // than somebody's spend. The refusal is logged — the caller gets a sentence,
    // and the fields an operator needs to tell a client bug from a probing
    // caller stay on our side.
    let params = Params::read(&request).map_err(|error| {
        tracing::warn!(
            %error,
            cluster = %logged(&request.cluster),
            buyer = %logged(&request.buyer),
            seller = %logged(&request.seller),
            reference = %logged(&request.reference),
            mint = %logged(&request.mint),
            amount = request.amount,
            memo_len = request.memo.len(),
            "a sponsored settle named an invoice that cannot exist"
        );
        error
    })?;
    let wallet = params.buyer.to_string();

    let per_wallet = cap_from_env("SPONSOR_DAILY_PER_WALLET", 10);
    let total = cap_from_env("SPONSOR_DAILY_TOTAL", 200);
    if recent_count(&state, Some(&wallet)).await? >= per_wallet as i64 {
        return Err(ApiError::TooManyRequests(format!(
            "This wallet has used today's {per_wallet} sponsored payments. Add a little SOL to it to keep paying."
        )));
    }
    if recent_count(&state, None).await? >= total as i64 {
        return Err(unavailable());
    }

    let wire = STANDARD
        .decode(request.transaction.trim())
        .map_err(|error| {
            tracing::warn!(
                %error,
                cluster = %logged(&request.cluster),
                buyer = %params.buyer,
                bytes = request.transaction.len(),
                "a sponsored settle's transaction was not base64"
            );
            bad("That transaction is not valid base64.")
        })?;
    let transaction: VersionedTransaction = bincode::deserialize(&wire).map_err(|error| {
        tracing::warn!(
            %error,
            cluster = %logged(&request.cluster),
            buyer = %params.buyer,
            bytes = wire.len(),
            "a sponsored settle's transaction could not be decoded"
        );
        bad("That transaction could not be read.")
    })?;

    // Signature shape: the fee payer has to be this sponsor, and the only
    // signature present is the buyer's, in the second slot, with the sponsor's
    // own left empty for us to fill. Both refusals are logged with the slots
    // that were wrong.
    let fee_payer = transaction.message.static_account_keys().first().copied();
    if fee_payer != Some(sponsor_key) {
        tracing::warn!(
            cluster = %logged(&request.cluster),
            buyer = %params.buyer,
            expected = %sponsor_key,
            actual = %fee_payer.map(|key| key.to_string()).unwrap_or_else(|| "none".into()),
            "a sponsored settle was addressed to another fee payer"
        );
        return Err(bad("The transaction's fee payer is not this sponsor."));
    }
    if transaction.signatures.len() != 2 || transaction.signatures[0] != Signature::default() {
        tracing::warn!(
            cluster = %logged(&request.cluster),
            buyer = %params.buyer,
            signatures = transaction.signatures.len(),
            fee_payer_slot_empty = transaction
                .signatures
                .first()
                .is_none_or(|signature| *signature == Signature::default()),
            buyer_slot_empty = transaction
                .signatures
                .get(1)
                .is_none_or(|signature| *signature == Signature::default()),
            "a sponsored settle did not carry exactly one buyer signature"
        );
        return Err(bad(
            "The transaction must carry exactly the buyer's signature and no fee-payer one.",
        ));
    }

    let expected = expected_message(
        &params,
        &sponsor_key,
        *transaction.message.recent_blockhash(),
    )?;
    let ours = message_bytes(&expected)?;
    let theirs = message_bytes(&transaction.message)?;
    if !same_settlement(&expected, &transaction.message) {
        // The one refusal a buyer cannot act on: their signature is over some
        // other message. Both shapes and the offset where they diverge say
        // whether a stale client or a wrong invoice is the cause, and the
        // buyer's signature stays here so support can look the
        // transaction up instead of asking for a screenshot of a network tab.
        let (our_accounts, our_instruction_accounts, our_data) = shape(&expected);
        let (their_accounts, their_instruction_accounts, their_data) = shape(&transaction.message);
        tracing::warn!(
            cluster = %logged(&request.cluster),
            buyer = %params.buyer,
            seller = %params.seller,
            amount = params.amount,
            mint = %mint_label(&params),
            memo_len = params.memo.len(),
            buyer_signature = %transaction.signatures[1],
            accounts = their_accounts,
            expected_accounts = our_accounts,
            instruction_accounts = their_instruction_accounts,
            expected_instruction_accounts = our_instruction_accounts,
            data_len = their_data,
            expected_data_len = our_data,
            first_difference = ?first_difference(&ours, &theirs),
            "a sponsored settle was not the settlement its invoice is priced at"
        );
        return Err(bad(
            "This transaction is not the settlement this invoice is priced at.",
        ));
    }

    // Counted here, not before the message was matched: a burst limit keyed by
    // a wallet that anyone can name would let a stranger spend somebody else's
    // minute with junk requests. Only a settlement the buyer actually signed
    // reaches this line, and a person pays once or twice a minute at most.
    crate::rate_limit::hit("sponsor-wallet", &wallet, 6, crate::rate_limit::MINUTE)?;

    // The same settlement, though perhaps not the same bytes: the buyer's
    // signature is over their own serialization, so that is the message the
    // sponsor signs too and the one submitted. The runtime verifies both.
    let signature = sponsor.sign_message(&theirs);
    let signed = VersionedTransaction {
        signatures: vec![signature, transaction.signatures[1]],
        message: transaction.message,
    };

    let endpoint = resolve_endpoint(request.endpoint.as_deref(), &request.cluster)?;
    let rpc = solana_rpc_client::nonblocking::rpc_client::RpcClient::new_with_commitment(
        endpoint.clone(),
        solana_commitment_config::CommitmentConfig::confirmed(),
    );
    // The cluster's own words, with both signatures: a preflight failure, an
    // expired blockhash and a program error all read differently here, and the
    // buyer's screen says only that the payment was not accepted.
    let sent = rpc.send_transaction(&signed).await.map_err(|error| {
        tracing::warn!(
            %error,
            details = ?error,
            cluster = %logged(&request.cluster),
            endpoint = %endpoint,
            buyer = %params.buyer,
            seller = %params.seller,
            amount = params.amount,
            mint = %mint_label(&params),
            sponsor_signature = %signature,
            buyer_signature = %transaction.signatures[1],
            "the cluster refused a sponsored settle"
        );
        ApiError::BadRequest(format!("The sponsored payment was not accepted: {error}"))
    })?;

    record(
        &state,
        user.as_ref().map(|user| user.id.as_str()),
        &params.buyer.to_string(),
        &request.cluster,
        params.amount,
        &sent.to_string(),
    )
    .await?;

    Ok(Json(SettleResponse {
        signature: sent.to_string(),
    }))
}
