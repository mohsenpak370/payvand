//! The sponsored settle fee: a relayer's keypair pays an invoice's network fee
//! and its two record rents, so a wallet holding only the price can buy
//! something — in USDC, EURC, or SOL.
//!
//! The relayer never signs a transaction a caller chose. `POST
//! /api/sponsor/settle` rebuilds the one settle message it expects from the
//! request's parameters — `settle_token_payment` with the invoice, the receipt,
//! the three token accounts, `Config` and the treasury, or `settle_payment` with
//! the invoice, the receipt, `Config` and the treasury when the request names no
//! mint — plus the sponsor's own account and, when the invoice was priced by a
//! shop, that shop's policy as the trailing optional account. Only if that
//! message is byte-for-byte the one the buyer signed does it add its fee-payer
//! signature and submit. A different instruction, account, amount or mint is a
//! mismatch, and a mismatch is refused before anything is signed.
//!
//! The program routes the treasury's refund of both rents back to the sponsor
//! (`plans/0004-relayer-sponsored-fees.md`), so a sponsored settlement is not a
//! lamport faucet for the buyer: the relayer's net cost is the network fee.
//! No session is needed: a buyer who scanned a printed code has no account, and
//! the buyer's own signature is what authorises the payment. Spend is bounded
//! here, not by the client: a rolling 24-hour cap per wallet and one for the
//! deployment, both in libSQL.

use std::str::FromStr;

use axum::extract::{Json, Query, State};
use axum::http::HeaderMap;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use solana_sdk::hash::Hash;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::message::{v0, VersionedMessage};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signature};
use solana_sdk::signer::Signer;
use solana_sdk::transaction::VersionedTransaction;

use crate::auth;
use crate::payments::{bad, currency_for_mint, resolve_endpoint, ApiError, ApiResult};
use crate::AppState;

/// The deployed program, overridable for a local cluster.
const DEFAULT_PROGRAM_ID: &str = "4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J";
const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ATA_PROGRAM: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";

/// What a sponsored settle fronts: the 262-byte token invoice's rent, the
/// 48-byte marker's, and the one signature the relayer pays for. The client asks
/// for a sponsor only below this, so a funded wallet keeps its own path.
pub const SPONSORED_LAMPORTS: u64 = 2_714_400 + 1_224_960 + 5_000;

/// A rolling day, so a cap cannot be gamed at midnight.
const DAY_SECS: i64 = 24 * 60 * 60;

#[derive(Debug, Deserialize)]
pub struct SettleRequest {
    /// The buyer-signed wire transaction, base64. Its fee payer is the sponsor
    /// and its message must be exactly the one this module rebuilds.
    pub transaction: String,
    pub buyer: String,
    pub seller: String,
    pub reference: String,
    /// The invoice's mint. Empty means a SOL invoice, which has no token
    /// accounts and is settled by `settle_payment` instead.
    #[serde(default)]
    pub mint: String,
    pub amount: u64,
    #[serde(default)]
    pub memo: String,
    /// The shop policy that priced this invoice, when its seller has one. The
    /// program takes it as a trailing optional account, so a sponsored settle
    /// of a shop invoice has to name it or its message is one account short of
    /// the buyer's and can never match.
    #[serde(default)]
    pub shop_policy: Option<String>,
    pub cluster: String,
    pub endpoint: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SettleResponse {
    pub signature: String,
}

#[derive(Debug, Serialize)]
pub struct InfoResponse {
    pub available: bool,
    pub address: Option<String>,
    /// Lamports a wallet needs before it can settle a token invoice itself.
    pub needs_lamports: u64,
    pub per_wallet_daily: u32,
}

/// What the endpoint answers with while it is not configured: the client shows
/// it instead of a wallet prompt that would fail on chain.
fn unavailable() -> ApiError {
    ApiError::TooManyRequests(
        "This payment needs a little SOL for the network fee, and sponsorship is not available right now."
            .into(),
    )
}

fn cap_from_env(name: &str, fallback: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(fallback)
}

/// The relayer's keypair, from `SPONSOR_KEYPAIR` — the solana CLI's JSON array
/// of 64 bytes, or the same bytes base58 encoded. Absent means no sponsorship.
fn sponsor_keypair() -> Option<Keypair> {
    crate::keypair_from_env("SPONSOR_KEYPAIR")
}

fn program_id() -> Pubkey {
    std::env::var("PAYVAND_PROGRAM_ID")
        .ok()
        .and_then(|value| Pubkey::from_str(value.trim()).ok())
        .unwrap_or_else(|| Pubkey::from_str(DEFAULT_PROGRAM_ID).expect("the built-in id parses"))
}

fn key(address: &str, what: &str) -> ApiResult<Pubkey> {
    Pubkey::from_str(address.trim())
        .map_err(|_| bad(format!("That {what} is not a valid Solana address.")))
}

/// The parameters the rebuilt message is made of, after validation.
struct Params {
    buyer: Pubkey,
    seller: Pubkey,
    reference: Pubkey,
    /// `None` is a SOL invoice.
    mint: Option<Pubkey>,
    amount: u64,
    memo: String,
    /// The shop's policy, absent for a shop that has none — the same absence
    /// that makes the program charge `Config`'s rate.
    shop_policy: Option<Pubkey>,
}

impl Params {
    /// Everything a request may name, checked against what the program will
    /// accept: a known stablecoin mint for the cluster, a positive price, a
    /// memo the invoice can hold, and real keys. Nothing here trusts the
    /// transaction the caller sent; it is only ever compared with the message
    /// these fields rebuild.
    fn read(request: &SettleRequest) -> ApiResult<Self> {
        if request.amount == 0 {
            return Err(bad("The price must be greater than zero."));
        }
        if request.memo.len() > 96 {
            return Err(bad("The invoice note is too long."));
        }
        // No mint is a SOL invoice: the price itself is lamports, so there is
        // nothing to check against the cluster's stablecoins.
        let mint = if request.mint.trim().is_empty() {
            None
        } else {
            let mint = key(&request.mint, "mint")?;
            if currency_for_mint(&mint, &request.cluster).is_none() {
                return Err(bad("Sponsored payments cover USDC, EURC and SOL only."));
            }
            Some(mint)
        };
        Ok(Self {
            buyer: key(&request.buyer, "buyer")?,
            seller: key(&request.seller, "seller")?,
            reference: key(&request.reference, "reference")?,
            mint,
            amount: request.amount,
            memo: request.memo.clone(),
            shop_policy: match request.shop_policy.as_deref() {
                Some(address) if !address.trim().is_empty() => Some(key(address, "shop policy")?),
                _ => None,
            },
        })
    }
}

fn pda(seeds: &[&[u8]], program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(seeds, program).0
}

/// The buyer's (or seller's, or treasury's) associated token account. Derived
/// under the classic token program, because that is the only program the
/// program settles.
fn ata(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    let token = Pubkey::from_str(TOKEN_PROGRAM).expect("the token program id parses");
    let ata_program = Pubkey::from_str(ATA_PROGRAM).expect("the ATA program id parses");
    pda(
        &[owner.as_ref(), token.as_ref(), mint.as_ref()],
        &ata_program,
    )
}

/// Anchor's instruction selector: the first eight bytes of
/// `sha256("global:<name>")`.
fn discriminator(name: &str) -> [u8; 8] {
    let digest = Sha256::digest(format!("global:{name}").as_bytes());
    let mut out = [0u8; 8];
    out.copy_from_slice(&digest[..8]);
    out
}

/// The settle instruction as the IDL declares it, in the IDL's account order,
/// with `sponsor` after the system program (before the optional shop policy).
/// A request with no mint is `settle_payment`; anything else is
/// `settle_token_payment` with the three token accounts the client's own
/// builder uses. The shop policy trails only when the invoice names one — the
/// program's optional account takes the program id in that slot to mean "none",
/// which is a message the client never builds.
fn settle_instruction(params: &Params, sponsor: &Pubkey) -> Instruction {
    let program = program_id();
    let treasury = pda(&[b"treasury"], &program);
    let config = pda(&[b"config"], &program);
    let system = Pubkey::from_str(SYSTEM_PROGRAM).expect("the system program id parses");

    // A SOL invoice: no mint, no token accounts, and `sponsor` is the account
    // that funds both records, as the program requires.
    if params.mint.is_none() {
        let invoice = pda(
            &[
                b"payment",
                params.seller.as_ref(),
                params.reference.as_ref(),
                &params.amount.to_le_bytes(),
            ],
            &program,
        );
        let mut data = discriminator("settle_payment").to_vec();
        data.extend_from_slice(&params.amount.to_le_bytes());
        data.extend_from_slice(&(params.memo.len() as u32).to_le_bytes());
        data.extend_from_slice(params.memo.as_bytes());

        let mut accounts = vec![
            AccountMeta::new(params.buyer, true),
            AccountMeta::new(invoice, false),
            AccountMeta::new(params.seller, false),
            AccountMeta::new(
                pda(&[b"payment-receipt", invoice.as_ref()], &program),
                false,
            ),
            AccountMeta::new_readonly(params.reference, false),
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(treasury, false),
            AccountMeta::new_readonly(system, false),
            AccountMeta::new(*sponsor, true),
        ];
        if let Some(shop_policy) = params.shop_policy.as_ref() {
            accounts.push(AccountMeta::new_readonly(*shop_policy, false));
        }
        return Instruction {
            program_id: program,
            accounts,
            data,
        };
    }

    let mint = params.mint.expect("checked above");
    let invoice = pda(
        &[
            b"token-payment",
            params.seller.as_ref(),
            params.reference.as_ref(),
            mint.as_ref(),
            &params.amount.to_le_bytes(),
        ],
        &program,
    );
    let token = Pubkey::from_str(TOKEN_PROGRAM).expect("the token program id parses");

    let mut data = discriminator("settle_token_payment").to_vec();
    data.extend_from_slice(&params.amount.to_le_bytes());
    data.extend_from_slice(&(params.memo.len() as u32).to_le_bytes());
    data.extend_from_slice(params.memo.as_bytes());

    let mut accounts = vec![
        AccountMeta::new(params.buyer, true),
        AccountMeta::new(invoice, false),
        AccountMeta::new(params.seller, false),
        AccountMeta::new(ata(&params.seller, &mint), false),
        AccountMeta::new(ata(&params.buyer, &mint), false),
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new(
            pda(&[b"payment-receipt", invoice.as_ref()], &program),
            false,
        ),
        AccountMeta::new_readonly(params.reference, false),
        AccountMeta::new_readonly(config, false),
        AccountMeta::new(treasury, false),
        AccountMeta::new(ata(&treasury, &mint), false),
        AccountMeta::new_readonly(token, false),
        AccountMeta::new_readonly(system, false),
        AccountMeta::new(*sponsor, true),
    ];
    // Trailing and read-only, exactly as the program declares it: absent for a
    // shop with no policy, never a placeholder in its slot.
    if let Some(shop_policy) = params.shop_policy.as_ref() {
        accounts.push(AccountMeta::new_readonly(*shop_policy, false));
    }

    Instruction {
        program_id: program,
        accounts,
        data,
    }
}

/// The one message this module will ever sign: version 0, the sponsor as fee
/// payer, and the settle instruction above.
fn expected_message(
    params: &Params,
    sponsor: &Pubkey,
    blockhash: Hash,
) -> ApiResult<VersionedMessage> {
    let instruction = settle_instruction(params, sponsor);
    let message = v0::Message::try_compile(sponsor, &[instruction], &[], blockhash)
        .map_err(|_| ApiError::Internal("the settle message could not be compiled".into()))?;
    Ok(VersionedMessage::V0(message))
}

fn message_bytes(message: &VersionedMessage) -> ApiResult<Vec<u8>> {
    Ok(message.serialize())
}

/// A caller's word as a log line may keep it: trimmed and bounded, so a request
/// cannot choose the size of the log. Only fields that are public anyway are
/// logged — an address, a cluster — never the memo, which is the buyer's.
fn logged(value: &str) -> String {
    value.trim().chars().take(64).collect()
}

/// The mint a refusal names, with `SOL` standing for the wallet path's absence
/// of one — an operator reading the log should not have to know that an empty
/// mint is the lamport invoice.
fn mint_label(params: &Params) -> String {
    params
        .mint
        .map(|mint| mint.to_string())
        .unwrap_or_else(|| "SOL".into())
}

/// The shape of a settle message: its account count, the instruction's account
/// count, and the instruction data's length. A missing shop policy, a different
/// buyer token account or a different price each change one of the three, which
/// is what makes a refusal readable without the transaction itself.
fn shape(message: &VersionedMessage) -> (usize, usize, usize) {
    match message {
        VersionedMessage::V0(message) => {
            let instruction = message.instructions.first();
            (
                message.account_keys.len(),
                instruction.map_or(0, |instruction| instruction.accounts.len()),
                instruction.map_or(0, |instruction| instruction.data.len()),
            )
        }
        _ => (0, 0, 0),
    }
}

/// Where two serialized settle messages first part ways. A version 0 message
/// ends with the blockhash, so an offset inside the keys or the instruction is
/// a stale client, a wrong invoice or a missing policy — and each of those is a
/// different bug for whoever reads the log.
fn first_difference(ours: &[u8], theirs: &[u8]) -> Option<usize> {
    ours.iter()
        .zip(theirs.iter())
        .position(|(ours, theirs)| ours != theirs)
        .or_else(|| (ours.len() != theirs.len()).then_some(ours.len().min(theirs.len())))
}

/// Sponsored settlements in the last rolling day, for one account or the whole
/// deployment. `user_id` is the account because that is what a session proves;
/// the wallet a sponsored payment pays from is named in the row.
async fn recent_count(state: &AppState, wallet: Option<&str>) -> ApiResult<i64> {
    let connection = state.db.connect().map_err(auth::internal)?;
    let since = auth::unix_now() - DAY_SECS;
    let mut rows = match wallet {
        Some(wallet) => {
            connection
                .query(
                    "SELECT COUNT(*) FROM sponsored_settlements \
                     WHERE wallet = ?1 AND created_at > ?2",
                    (wallet, since),
                )
                .await
        }
        None => {
            connection
                .query(
                    "SELECT COUNT(*) FROM sponsored_settlements WHERE created_at > ?1",
                    [since],
                )
                .await
        }
    }
    .map_err(auth::internal)?;
    let count: i64 = match rows.next().await.map_err(auth::internal)? {
        Some(row) => row.get(0).map_err(auth::internal)?,
        None => 0,
    };
    Ok(count)
}

/// Records a settlement whose transaction the cluster has accepted, so the caps
/// count what was actually sponsored rather than what was attempted.
async fn record(
    state: &AppState,
    user_id: Option<&str>,
    wallet: &str,
    cluster: &str,
    amount: u64,
    signature: &str,
) -> ApiResult<()> {
    let connection = state.db.connect().map_err(auth::internal)?;
    connection
        .execute(
            "INSERT INTO sponsored_settlements \
             (id, wallet, user_id, cluster, amount, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                signature.to_string(),
                wallet.to_string(),
                match user_id {
                    Some(id) => libsql::Value::Text(id.to_owned()),
                    None => libsql::Value::Null,
                },
                cluster.to_string(),
                amount as i64,
                auth::unix_now(),
            ),
        )
        .await
        .map_err(auth::internal)?;
    Ok(())
}

/// `?wallet=` — the address the daily cap is counted against. A signed-in
/// caller's linked wallet stands in when the query omits one.
#[derive(Debug, Deserialize)]
pub struct InfoQuery {
    pub wallet: Option<String>,
}

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
    if ours != theirs {
        // The one refusal a buyer cannot act on: their signature is over some
        // other message. Both shapes and the offset where they diverge say
        // whether a stale client, a wrong invoice or a policy-less shop is the
        // cause, and the buyer's signature stays here so support can look the
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
            shop_policy = %params
                .shop_policy
                .map(|policy| policy.to_string())
                .unwrap_or_else(|| "none".into()),
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

    // Exact match, so the buyer's signature is over these same bytes: keep it,
    // add the sponsor's, and submit. The runtime verifies both.
    let signature = sponsor.sign_message(&ours);
    let signed = VersionedTransaction {
        signatures: vec![signature, transaction.signatures[1]],
        message: expected,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn params(memo: &str) -> Params {
        Params {
            buyer: Pubkey::new_from_array([1; 32]),
            seller: Pubkey::new_from_array([2; 32]),
            reference: Pubkey::new_from_array([3; 32]),
            mint: Some(crate::payments::mint_for("usdc", "devnet").expect("USDC on devnet")),
            amount: 4_500_000,
            memo: memo.into(),
            shop_policy: None,
        }
    }

    /// The message the endpoint compares against is built here, so these are the
    /// invariants the byte match rests on: the sponsor is the fee payer and a
    /// signer, the instruction is the settle one, and the account list is the
    /// program's, not something shorter.
    #[test]
    fn rebuilds_the_settle_message_the_program_expects() {
        let sponsor = Pubkey::new_from_array([9; 32]);
        let message = expected_message(&params("flat white"), &sponsor, Hash::default())
            .expect("the message compiles");
        let VersionedMessage::V0(message) = message else {
            panic!("the sponsored settle is a version 0 message");
        };
        assert_eq!(message.account_keys[0], sponsor, "the sponsor pays the fee");
        assert_eq!(message.instructions.len(), 1);
        let instruction = &message.instructions[0];
        let program = message.account_keys[instruction.program_id_index as usize];
        assert_eq!(program, program_id());
        assert_eq!(
            message.account_keys[instruction.accounts[0] as usize],
            params("").buyer,
            "the buyer is the first account"
        );
        // The sponsor is both the fee payer and the last account; a version 0
        // message keeps one key per address, so the account list ends at it.
        let last = *instruction.accounts.last().expect("the sponsor account");
        assert_eq!(message.account_keys[last as usize], sponsor);
        assert_eq!(instruction.accounts.len(), 14);
    }

    /// A shop invoice's settle has one more account, and the relayer's rebuild
    /// must carry it: without it the message is a slot short and no sponsored
    /// settle of a shop invoice could ever match what the buyer signed.
    #[test]
    fn a_shop_policy_travels_as_the_trailing_account() {
        let sponsor = Pubkey::new_from_array([9; 32]);
        let shop_policy = Pubkey::new_from_array([8; 32]);
        let plain = settle_instruction(&params(""), &sponsor);
        let mut shop = params("");
        shop.shop_policy = Some(shop_policy);
        let priced = settle_instruction(&shop, &sponsor);

        assert_eq!(priced.accounts.len(), plain.accounts.len() + 1);
        assert_eq!(priced.accounts.len(), 15);
        let last = priced.accounts.last().expect("the policy account");
        assert_eq!(last.pubkey, shop_policy);
        assert!(!last.is_writable, "the policy is read-only");
        assert!(!last.is_signer, "the relayer never signs for a seller");
        assert_eq!(
            &priced.accounts[..14],
            &plain.accounts[..],
            "everything before the policy is the wallet settle's own list"
        );
    }

    #[test]
    fn the_instruction_is_the_idl_shape() {
        let sponsor = Pubkey::new_from_array([9; 32]);
        let instruction = settle_instruction(&params(""), &sponsor);
        assert_eq!(
            &instruction.data[..8],
            &discriminator("settle_token_payment")
        );
        assert_eq!(
            u64::from_le_bytes(instruction.data[8..16].try_into().expect("eight bytes")),
            4_500_000
        );
        // memo: a u32 length then the bytes
        assert_eq!(&instruction.data[16..20], &0u32.to_le_bytes());
        assert!(instruction.accounts[0].is_signer);
        assert!(instruction.accounts[13].is_signer);
    }

    /// A SOL invoice is settled by `settle_payment`, whose account list is the
    /// program's — and the relayer, not the buyer, is its fee payer, which is
    /// what keeps the buyer from needing lamports it does not have.
    #[test]
    fn rebuilds_the_sol_settle_message() {
        let sponsor = Pubkey::new_from_array([9; 32]);
        let params = Params {
            mint: None,
            ..params("coffee")
        };
        let message = expected_message(&params, &sponsor, Hash::default()).expect("compiles");
        let instructions = match &message {
            VersionedMessage::V0(message) => message.instructions.clone(),
            other => panic!("unexpected message version: {other:?}"),
        };
        assert_eq!(instructions.len(), 1);
        let instruction = &instructions[0];
        assert_eq!(instruction.data[..8], discriminator("settle_payment"));
        assert_eq!(
            instruction.accounts.len(),
            9,
            "payer, invoice, seller, receipt, reference, config, treasury, system, sponsor"
        );
        let keys = message.static_account_keys();
        assert_eq!(keys[0], sponsor, "the relayer is the fee payer");
        let account = |index: usize| keys[instruction.accounts[index] as usize];
        assert_eq!(account(0), params.buyer);
        assert_eq!(account(2), params.seller);
        assert_eq!(
            account(8),
            sponsor,
            "the sponsor is the account that funds the records"
        );
    }

    #[test]
    fn a_mint_outside_the_list_is_refused() {
        let mut request = SettleRequest {
            transaction: String::new(),
            buyer: Pubkey::new_from_array([1; 32]).to_string(),
            seller: Pubkey::new_from_array([2; 32]).to_string(),
            reference: Pubkey::new_from_array([3; 32]).to_string(),
            mint: Pubkey::new_from_array([7; 32]).to_string(),
            amount: 1_000_000,
            memo: String::new(),
            shop_policy: None,
            cluster: "devnet".into(),
            endpoint: None,
        };
        assert!(Params::read(&request).is_err());
        request.mint = crate::payments::mint_for("usdc", "devnet")
            .expect("USDC")
            .to_string();
        assert!(Params::read(&request).is_ok());
        request.shop_policy = Some("not-a-key".into());
        assert!(Params::read(&request).is_err());
        request.shop_policy = Some(String::new());
        assert_eq!(
            Params::read(&request)
                .expect("blank means no policy")
                .shop_policy,
            None
        );
        request.amount = 0;
        assert!(Params::read(&request).is_err());
        request.amount = 1;
        request.memo = "x".repeat(97);
        assert!(Params::read(&request).is_err());
    }

    async fn test_state() -> (AppState, std::path::PathBuf) {
        use std::sync::Arc;
        let path =
            std::env::temp_dir().join(format!("payvand-sponsor-test-{}.db", auth::random_id()));
        let db = libsql::Builder::new_local(&path)
            .build()
            .await
            .expect("temp db");
        crate::db::migrate(&db).await.expect("migrate");
        (AppState { db: Arc::new(db) }, path)
    }

    /// The caps are counted by the wallet that signs and by the deployment: a
    /// buyer with no account has the same daily bound as one with an account,
    /// which is what lets a printed code be sponsored.
    #[tokio::test]
    async fn counts_are_per_wallet_and_global() {
        let (state, path) = test_state().await;
        assert_eq!(
            recent_count(&state, Some("buyer-wallet"))
                .await
                .expect("count"),
            0
        );
        assert_eq!(recent_count(&state, None).await.expect("count"), 0);

        for index in 0..3 {
            record(
                &state,
                None,
                "buyer-wallet",
                "devnet",
                1_000_000,
                &format!("signature-{index}"),
            )
            .await
            .expect("record with no account");
        }
        assert_eq!(
            recent_count(&state, Some("buyer-wallet"))
                .await
                .expect("count"),
            3
        );
        assert_eq!(
            recent_count(&state, Some("other-wallet"))
                .await
                .expect("count"),
            0
        );
        assert_eq!(recent_count(&state, None).await.expect("count"), 3);

        let _ = std::fs::remove_file(path);
    }

    /// The refusal lines are only useful if they point somewhere: the offset
    /// where two settles differ, and the shape that says which part of the
    /// message it was.
    #[test]
    fn a_refusal_can_say_where_the_two_settles_differ() {
        assert_eq!(first_difference(b"abc", b"abc"), None);
        assert_eq!(first_difference(b"abc", b"abd"), Some(2));
        assert_eq!(first_difference(b"abc", b"abcd"), Some(3));
        assert_eq!(first_difference(b"", b"a"), Some(0));

        let sponsor = Pubkey::new_from_array([9; 32]);
        let message = expected_message(&params(""), &sponsor, Hash::default()).expect("compiles");
        let (accounts, instruction_accounts, data) = shape(&message);
        assert_eq!(instruction_accounts, 14, "the token settle's account list");
        assert_eq!(data, 20, "selector, amount and the memo's length prefix");
        assert!(
            accounts >= instruction_accounts,
            "the static keys also carry the program and the sponsor"
        );

        // An address is logged as given; a field nobody should see at length is
        // cut, so the request cannot choose how big the log line is.
        assert_eq!(logged("  devnet  "), "devnet");
        assert_eq!(logged(&"x".repeat(200)).len(), 64);
    }

    /// The mint a refusal names reads the same on both settle paths.
    #[test]
    fn a_refusal_names_the_mint_or_says_sol() {
        assert_eq!(
            mint_label(&params("")),
            crate::payments::mint_for("usdc", "devnet")
                .expect("USDC")
                .to_string()
        );
        assert_eq!(
            mint_label(&Params {
                mint: None,
                ..params("")
            }),
            "SOL"
        );
    }

    /// A `SPONSOR_KEYPAIR` the API cannot use is the same as no sponsor: the
    /// client falls back to asking for SOL rather than to a broken promise.
    #[test]
    fn an_unreadable_sponsor_key_is_no_sponsor() {
        std::env::set_var("SPONSOR_KEYPAIR", "not-a-key");
        assert!(sponsor_keypair().is_none());
        let generated = Keypair::new();
        std::env::set_var(
            "SPONSOR_KEYPAIR",
            serde_json::to_string(&generated.to_bytes().to_vec()).expect("json"),
        );
        assert!(sponsor_keypair().is_some());
        std::env::remove_var("SPONSOR_KEYPAIR");
        assert!(sponsor_keypair().is_none());
    }
}
