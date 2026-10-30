//! Wallet linking and wallet sign-in.
//!
//! A wallet proves it belongs to an account by signing a server-issued
//! message: `POST /api/auth/wallet/challenge` stores the exact text (one
//! live challenge per address, five minute expiry), the wallet signs it,
//! and the follow-up call verifies the signature against the address,
//! checks the purpose, and consumes the row — so a signature can never be
//! replayed, nor reused for a different request.
//!
//! `GET /api/me/transactions` reads the program's accounts and returns the
//! invoices that involve the linked wallet — as seller, as payer, or as the
//! buyer behind a budget's session key.

use axum::extract::{Json, State};
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};

use crate::auth::{self};
use crate::payments::{self, ApiError, ApiResult, BudgetAccount, PaymentRecord, PaymentStatus};

/// Challenges live for five minutes and are single use.
pub const CHALLENGE_TTL_SECS: i64 = 300;
/// The newest transactions returned by `GET /api/me/transactions`.
const TRANSACTION_LIMIT: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChallengePurpose {
    /// Attach the wallet to the account that is signed in right now.
    Link,
    /// Open a session for the account this wallet is linked to.
    Signin,
}

impl ChallengePurpose {
    fn label(self) -> &'static str {
        match self {
            Self::Link => "link",
            Self::Signin => "sign in",
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Link => "link",
            Self::Signin => "signin",
        }
    }

    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "link" => Some(Self::Link),
            "signin" => Some(Self::Signin),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ChallengeRequest {
    pub address: String,
    pub purpose: ChallengePurpose,
}

#[derive(Debug, Serialize)]
pub struct ChallengeResponse {
    /// The exact text to hand the wallet — never rebuilt client side.
    pub message: String,
    pub purpose: ChallengePurpose,
    pub expires_at: i64,
}

/// Step 1: issue the text the wallet must sign.
pub async fn challenge(
    State(state): State<crate::AppState>,
    Json(request): Json<ChallengeRequest>,
) -> ApiResult<Json<ChallengeResponse>> {
    // Reject malformed addresses before touching the database.
    payments::parse_pubkey(&request.address, "address")?;

    let connection = state.db.connect().map_err(auth::internal)?;
    let now = auth::unix_now();
    let expires_at = now + CHALLENGE_TTL_SECS;
    let nonce = auth::random_id();

    let message = format!(
        "Payvand wants you to {label} with your Solana account:\n\
         {address}\n\
         \n\
         This signature only proves this wallet is yours. It authorises no transaction.\n\
         \n\
         Nonce: {nonce}\n\
         Expires: {expires_at}",
        label = request.purpose.label(),
        address = request.address,
    );

    // One live challenge per address: asking again simply replaces it.
    connection
        .execute(
            "INSERT OR REPLACE INTO wallet_challenges \
             (address, purpose, nonce, message, issued_at, expires_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                request.address.clone(),
                request.purpose.as_str().to_string(),
                nonce,
                message.clone(),
                now,
                expires_at,
            ),
        )
        .await
        .map_err(auth::internal)?;

    // Opportunistic cleanup of anything long expired.
    let _ = connection
        .execute(
            "DELETE FROM wallet_challenges WHERE expires_at <= ?1",
            [now],
        )
        .await;

    Ok(Json(ChallengeResponse {
        message,
        purpose: request.purpose,
        expires_at,
    }))
}

#[derive(Debug, Deserialize)]
pub struct WalletSignatureRequest {
    pub address: String,
    pub signature: String,
}

/// Loads the stored challenge, checks it, verifies the signature over its
/// exact text, and consumes it. `purpose` must match the one requested.
async fn consume_challenge(
    connection: &libsql::Connection,
    address: &str,
    purpose: ChallengePurpose,
    signature: &str,
) -> ApiResult<()> {
    let now = auth::unix_now();

    let mut rows = connection
        .query(
            "SELECT purpose, message, expires_at FROM wallet_challenges WHERE address = ?1",
            [address],
        )
        .await
        .map_err(auth::internal)?;

    let row = rows
        .next()
        .await
        .map_err(auth::internal)?
        .ok_or_else(|| {
            ApiError::BadRequest(
                "That request expired or was already used — start again.".into(),
            )
        })?;

    let stored_purpose: String = row.get(0).map_err(auth::internal)?;
    let message: String = row.get(1).map_err(auth::internal)?;
    let expires_at: i64 = row.get(2).map_err(auth::internal)?;

    if expires_at <= now {
        let _ = connection
            .execute(
                "DELETE FROM wallet_challenges WHERE address = ?1",
                [address],
            )
            .await;
        return Err(ApiError::BadRequest(
            "That request expired — start again.".into(),
        ));
    }

    let wanted = ChallengePurpose::parse(&stored_purpose)
        .ok_or_else(|| ApiError::Internal("unknown challenge purpose".into()))?;
    if wanted != purpose {
        return Err(ApiError::BadRequest(
            "That signature was requested for a different purpose.".into(),
        ));
    }

    verify_signature(address, &message, signature)?;

    // Single use: a consumed challenge can never be replayed.
    let _ = connection
        .execute(
            "DELETE FROM wallet_challenges WHERE address = ?1",
            [address],
        )
        .await;
    Ok(())
}

/// Checks `signature` (base58, 64 bytes) over `message` for `address`.
fn verify_signature(address: &str, message: &str, signature: &str) -> ApiResult<()> {
    let pubkey = payments::parse_pubkey(address, "address")?;
    let bytes: [u8; 64] = bs58::decode(signature)
        .into_vec()
        .map_err(|_| ApiError::BadRequest("signature must be base58 encoded".into()))?
        .try_into()
        .map_err(|_| ApiError::BadRequest("signature must be 64 bytes".into()))?;

    let key = ed25519_dalek::VerifyingKey::from_bytes(&pubkey.to_bytes())
        .map_err(|_| ApiError::BadRequest("address is not a valid public key".into()))?;
    let signature = ed25519_dalek::Signature::from_bytes(&bytes);

    key.verify_strict(message.as_bytes(), &signature)
        .map_err(|_| ApiError::BadRequest("that signature does not match this wallet".into()))
}

/// Step 2 (link): attach the signed wallet to the signed-in account.
pub async fn link(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Json(request): Json<WalletSignatureRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;

    consume_challenge(
        &connection,
        &request.address,
        ChallengePurpose::Link,
        &request.signature,
    )
    .await?;

    let updated = connection
        .execute(
            "UPDATE users SET wallet = ?1 WHERE id = ?2",
            (request.address.clone(), user.id.clone()),
        )
        .await;
    match updated {
        Ok(0) => return Err(ApiError::Unauthorized),
        Ok(_) => {}
        Err(error) if auth::is_unique_violation(&error) => {
            return Err(ApiError::BadRequest(
                "That wallet is already linked to another account.".into(),
            ));
        }
        Err(error) => return Err(auth::internal(error)),
    }

    let user = auth::read_user(&connection, &user.id)
        .await?
        .ok_or_else(|| ApiError::Internal("the account could not be read back".into()))?;
    Ok(Json(serde_json::json!({ "user": user })))
}

/// Step 2 (signin): open a session for the account this wallet is linked to.
pub async fn signin(
    State(state): State<crate::AppState>,
    Json(request): Json<WalletSignatureRequest>,
) -> ApiResult<Json<auth::AuthResponse>> {
    let connection = state.db.connect().map_err(auth::internal)?;

    consume_challenge(
        &connection,
        &request.address,
        ChallengePurpose::Signin,
        &request.signature,
    )
    .await?;

    let mut rows = connection
        .query(
            "SELECT id FROM users WHERE wallet = ?1",
            [request.address.as_str()],
        )
        .await
        .map_err(auth::internal)?;

    let user_id: String = match rows.next().await.map_err(auth::internal)? {
        Some(row) => row.get(0).map_err(auth::internal)?,
        None => {
            return Err(ApiError::BadRequest(
                "That wallet is not linked to an account yet. \
                 Sign in with email, then link the wallet from your profile."
                    .into(),
            ))
        }
    };

    let user = auth::read_user(&connection, &user_id)
        .await?
        .ok_or_else(|| ApiError::Internal("the account could not be read back".into()))?;
    let token = auth::issue_session(&connection, &user.id).await?;
    Ok(Json(auth::AuthResponse { user, token }))
}

#[derive(Debug, Serialize)]
pub struct MyTransaction {
    pub id: String,
    /// `"sent"` or `"received"`.
    pub direction: String,
    /// Who paid, or who was paid.
    pub counterparty: String,
    pub amount_lamports: u64,
    pub memo: String,
    pub status: PaymentStatus,
    pub created_at_unix: i64,
    pub settled_at_unix: i64,
}

#[derive(Debug, Serialize)]
pub struct TransactionsResponse {
    pub wallet: String,
    pub items: Vec<MyTransaction>,
    /// How many the wallet has in total, before the limit was applied.
    pub total: usize,
    pub cluster: String,
}

/// The signed-in account's transactions for its linked wallet.
pub async fn transactions(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<TransactionsResponse>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let wallet = user.wallet.ok_or_else(|| {
        ApiError::BadRequest("Link your wallet on the profile page first.".into())
    })?;

    let scan = payments::fetch_program_scan().await?;
    let mut items = match_transactions(scan.payments, &scan.budgets, &wallet);
    items.truncate(TRANSACTION_LIMIT);

    Ok(Json(TransactionsResponse {
        wallet,
        total: items.len(),
        cluster: scan.cluster,
        items,
    }))
}

/// Pure part of the transactions feed: keep the invoices this wallet is
/// party to, label the direction, newest first.
///
/// The budget path records the *session key* as the payer, so a payment
/// made silently from this buyer's allowance counts as theirs too — which
/// is why the budget accounts come along.
pub(crate) fn match_transactions(
    records: Vec<PaymentRecord>,
    budgets: &[BudgetAccount],
    wallet: &str,
) -> Vec<MyTransaction> {
    let session = budgets
        .iter()
        .find(|budget| budget.buyer.to_string() == wallet)
        .map(|budget| budget.session_key.to_string());

    let mut items: Vec<MyTransaction> = records
        .into_iter()
        .filter_map(|record| {
            let is_seller = record.seller == wallet;
            let payer = record.settled_by.as_deref();
            let is_payer = payer == Some(wallet)
                || (session.is_some() && payer == session.as_deref());
            if !is_seller && !is_payer {
                return None;
            }

            let direction = if is_seller { "received" } else { "sent" };
            let counterparty = if is_seller {
                record
                    .settled_by
                    .clone()
                    .unwrap_or_else(|| "awaiting payment".to_string())
            } else {
                record.seller.clone()
            };

            Some(MyTransaction {
                id: record.id,
                direction: direction.to_string(),
                counterparty,
                amount_lamports: record.amount_lamports,
                memo: record.memo,
                status: record.status,
                created_at_unix: record.created_at_unix,
                settled_at_unix: record.settled_at_unix,
            })
        })
        .collect();

    items.sort_by(|a, b| {
        b.settled_at_unix
            .cmp(&a.settled_at_unix)
            .then_with(|| b.created_at_unix.cmp(&a.created_at_unix))
            .then_with(|| a.id.cmp(&b.id))
    });
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::{Json as JsonExtractor, State as StateExtractor};
    use ed25519_dalek::{Signer, SigningKey};

    async fn test_state() -> (crate::AppState, std::path::PathBuf) {
        let path = std::path::PathBuf::from(std::env::temp_dir())
            .join(format!("payvand-wallet-test-{}.db", auth::random_id()));
        let db = libsql::Builder::new_local(&path)
            .build()
            .await
            .expect("temp db");
        crate::db::migrate(&db).await.expect("migrate");
        (
            crate::AppState { db: std::sync::Arc::new(db) },
            path,
        )
    }

    fn bearer(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            format!("Bearer {token}").parse().expect("header value"),
        );
        headers
    }

    async fn signup(state: &crate::AppState, email: &str) -> String {
        crate::auth::signup(
            StateExtractor(state.clone()),
            JsonExtractor(crate::auth::SignupRequest {
                email: email.to_string(),
                password: "correct horse battery staple".into(),
            }),
        )
        .await
        .expect("signup")
        .0
        .token
    }

    /// A deterministic wallet: address in base58, key ready to sign.
    fn wallet_fixture(seed: u8) -> (String, SigningKey) {
        let key = SigningKey::from_bytes(&[seed; 32]);
        let address = bs58::encode(key.verifying_key().to_bytes()).into_string();
        (address, key)
    }

    fn sign_base58(key: &SigningKey, message: &str) -> String {
        bs58::encode(key.sign(message.as_bytes()).to_bytes()).into_string()
    }

    #[tokio::test]
    async fn wallet_link_signin_and_gates() {
        let (state, path) = test_state().await;
        let token = signup(&state, "wallet-owner@example.com").await;
        let headers = bearer(&token);
        let (address, key) = wallet_fixture(7);
        let (stranger_address, stranger_key) = wallet_fixture(9);

        // 1. Challenge issues stored text for the requested purpose.
        let issued = challenge(
            StateExtractor(state.clone()),
            JsonExtractor(ChallengeRequest {
                address: address.clone(),
                purpose: ChallengePurpose::Link,
            }),
        )
        .await
        .expect("challenge")
        .0;
        assert!(issued.message.contains(&address));
        assert_eq!(issued.purpose, ChallengePurpose::Link);

        // 2. A signature from the wrong key must not link.
        let wrong = sign_base58(&stranger_key, &issued.message);
        let rejected = link(
            StateExtractor(state.clone()),
            headers.clone(),
            JsonExtractor(WalletSignatureRequest {
                address: address.clone(),
                signature: wrong,
            }),
        )
        .await;
        assert!(matches!(rejected, Err(ApiError::BadRequest(_))));

        // 3. The right key links the wallet to this account.
        let good = sign_base58(&key, &issued.message);
        let linked = link(
            StateExtractor(state.clone()),
            headers.clone(),
            JsonExtractor(WalletSignatureRequest {
                address: address.clone(),
                signature: good.clone(),
            }),
        )
        .await
        .expect("link");
        let wallet: Option<String> = linked
            .0
            .get("user")
            .and_then(|user| user.get("wallet"))
            .and_then(|value| value.as_str())
            .map(str::to_string);
        assert_eq!(wallet.as_deref(), Some(address.as_str()));

        // 4. The challenge is consumed: the same signature cannot replay.
        let replay = link(
            StateExtractor(state.clone()),
            headers.clone(),
            JsonExtractor(WalletSignatureRequest {
                address: address.clone(),
                signature: good,
            }),
        )
        .await;
        assert!(matches!(replay, Err(ApiError::BadRequest(_))));

        // 5. Wallet sign-in opens a session for the linked account.
        let signin_challenge = challenge(
            StateExtractor(state.clone()),
            JsonExtractor(ChallengeRequest {
                address: address.clone(),
                purpose: ChallengePurpose::Signin,
            }),
        )
        .await
        .expect("signin challenge")
        .0;
        let session = signin(
            StateExtractor(state.clone()),
            JsonExtractor(WalletSignatureRequest {
                address: address.clone(),
                signature: sign_base58(&key, &signin_challenge.message),
            }),
        )
        .await
        .expect("wallet signin")
        .0;
        assert_eq!(session.user.wallet.as_deref(), Some(address.as_str()));
        let me = auth::require_user(
            &state.db,
            &bearer(&session.token),
        )
        .await
        .expect("session works");
        assert_eq!(me.id, session.user.id);

        // 6. An unlinked wallet gets a clear refusal, not a session.
        let orphan_challenge = challenge(
            StateExtractor(state.clone()),
            JsonExtractor(ChallengeRequest {
                address: stranger_address.clone(),
                purpose: ChallengePurpose::Signin,
            }),
        )
        .await
        .expect("orphan challenge")
        .0;
        let orphan = signin(
            StateExtractor(state.clone()),
            JsonExtractor(WalletSignatureRequest {
                address: stranger_address.clone(),
                signature: sign_base58(&stranger_key, &orphan_challenge.message),
            }),
        )
        .await;
        assert!(matches!(orphan, Err(ApiError::BadRequest(_))));

        // 7. A challenge issued for sign-in cannot be used to link (purpose).
        let signin_only = challenge(
            StateExtractor(state.clone()),
            JsonExtractor(ChallengeRequest {
                address: address.clone(),
                purpose: ChallengePurpose::Signin,
            }),
        )
        .await
        .expect("sign-in-only challenge")
        .0;
        let misused = link(
            StateExtractor(state.clone()),
            headers.clone(),
            JsonExtractor(WalletSignatureRequest {
                address: address.clone(),
                signature: sign_base58(&key, &signin_only.message),
            }),
        )
        .await;
        assert!(matches!(misused, Err(ApiError::BadRequest(_))));

        // 8. Anonymous callers cannot link or list transactions.
        let anonymous_link = link(
            StateExtractor(state.clone()),
            HeaderMap::new(),
            JsonExtractor(WalletSignatureRequest {
                address: address.clone(),
                signature: sign_base58(&key, &issued.message),
            }),
        )
        .await;
        assert!(matches!(anonymous_link, Err(ApiError::Unauthorized)));

        // 9. A signed-in account with no linked wallet is refused the feed
        //    before any RPC happens.
        let stranger_token = signup(&state, "no-wallet@example.com").await;
        let feed = transactions(
            StateExtractor(state.clone()),
            bearer(&stranger_token),
        )
        .await;
        assert!(matches!(feed, Err(ApiError::BadRequest(_))));

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn expired_challenges_are_refused() {
        let (state, path) = test_state().await;
        let (address, key) = wallet_fixture(3);

        let _issued = challenge(
            StateExtractor(state.clone()),
            JsonExtractor(ChallengeRequest {
                address: address.clone(),
                purpose: ChallengePurpose::Signin,
            }),
        )
        .await
        .expect("challenge");

        // Age the row past its expiry, exactly as time would.
        let connection = state.db.connect().expect("connect");
        connection
            .execute(
                "UPDATE wallet_challenges SET expires_at = ?1 WHERE address = ?2",
                (auth::unix_now() - 1, address.as_str()),
            )
            .await
            .expect("age");

        let expired = signin(
            StateExtractor(state.clone()),
            JsonExtractor(WalletSignatureRequest {
                address: address.clone(),
                signature: sign_base58(&key, "anything"),
            }),
        )
        .await;
        assert!(matches!(expired, Err(ApiError::BadRequest(_))));

        let _ = std::fs::remove_file(path);
    }

    /// A payment account as the chain would hand it over.
    fn payment_fixture(
        id: &str,
        seller: &str,
        settled_by: Option<&str>,
        created: i64,
        settled: i64,
    ) -> PaymentRecord {
        PaymentRecord {
            id: id.to_string(),
            status: if settled > 0 {
                PaymentStatus::Settled
            } else {
                PaymentStatus::Pending
            },
            payment_pda: id.to_string(),
            seller: seller.to_string(),
            seller_user: None,
            reference: "Reference111".into(),
            amount_lamports: 1_000_000,
            memo: format!("memo for {id}"),
            cluster: "devnet".into(),
            endpoint: "https://api.devnet.solana.com".into(),
            settled_by: settled_by.map(str::to_string),
            created_at_unix: created,
            settled_at_unix: settled,
        }
    }

    #[test]
    fn transactions_are_matched_and_labelled() {
        let (wallet, _) = wallet_fixture(1);
        let (other, _) = wallet_fixture(2);
        let (session, _) = wallet_fixture(4);

        let budget = BudgetAccount {
            buyer: payments::parse_pubkey(&wallet, "address").expect("wallet"),
            session_key: payments::parse_pubkey(&session, "address").expect("session"),
        };

        let records = vec![
            // Sold something: received.
            payment_fixture("aaa", &wallet, Some(&other), 100, 500),
            // Paid a seller from this wallet: sent.
            payment_fixture("bbb", &other, Some(&wallet), 200, 600),
            // Paid silently from this buyer's budget: sent, via session key.
            payment_fixture("ccc", &other, Some(&session), 300, 700),
            // Someone else's invoice: not ours.
            payment_fixture("ddd", &other, Some(&other), 400, 800),
            // Our unpaid invoice: still ours to see.
            payment_fixture("eee", &wallet, None, 50, 0),
        ];

        let items = match_transactions(records, std::slice::from_ref(&budget), &wallet);

        let ids: Vec<&str> = items.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(ids, vec!["ccc", "bbb", "aaa", "eee"], "newest first");

        assert_eq!(items[0].direction, "sent", "budget payment counts as sent");
        assert_eq!(items[1].direction, "sent");
        assert_eq!(items[2].direction, "received");
        assert_eq!(items[3].counterparty, "awaiting payment");

        // Without the budget, the silent payment is invisible.
        let without = match_transactions(
            vec![payment_fixture("ccc", &other, Some(&session), 300, 700)],
            &[],
            &wallet,
        );
        assert!(without.is_empty());
    }
}
