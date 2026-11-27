//! The challenge, link, sign-in and unlink routes end to end.

use axum::extract::{Json as JsonExtractor, Query as QueryExtractor, State as StateExtractor};
use axum::http::HeaderMap;

use super::fixtures::{bearer, sign_base58, signup, test_state, wallet_fixture};
use super::*;
use crate::auth;
use crate::payments::ApiError;

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
    let me = auth::require_user(&state.db, &bearer(&session.token))
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
        QueryExtractor(FeedQuery { scan: None }),
    )
    .await;
    assert!(matches!(feed, Err(ApiError::BadRequest(_))));

    // 10. Switching: linking a different wallet replaces the first one,
    //     and the old wallet can no longer open a session.
    let (address2, key2) = wallet_fixture(11);
    let switch_challenge = challenge(
        StateExtractor(state.clone()),
        JsonExtractor(ChallengeRequest {
            address: address2.clone(),
            purpose: ChallengePurpose::Link,
        }),
    )
    .await
    .expect("switch challenge")
    .0;
    let switched = link(
        StateExtractor(state.clone()),
        headers.clone(),
        JsonExtractor(WalletSignatureRequest {
            address: address2.clone(),
            signature: sign_base58(&key2, &switch_challenge.message),
        }),
    )
    .await
    .expect("switch");
    let new_wallet: Option<String> = switched
        .0
        .get("user")
        .and_then(|user| user.get("wallet"))
        .and_then(|value| value.as_str())
        .map(str::to_string);
    assert_eq!(new_wallet.as_deref(), Some(address2.as_str()));

    let old_still_works = {
        let issued = challenge(
            StateExtractor(state.clone()),
            JsonExtractor(ChallengeRequest {
                address: address.clone(),
                purpose: ChallengePurpose::Signin,
            }),
        )
        .await
        .expect("old wallet challenge")
        .0;
        signin(
            StateExtractor(state.clone()),
            JsonExtractor(WalletSignatureRequest {
                address: address.clone(),
                signature: sign_base58(&key, &issued.message),
            }),
        )
        .await
    };
    assert!(matches!(old_still_works, Err(ApiError::BadRequest(_))));

    // 11. Unlink detaches the wallet; anonymous callers cannot.
    let anon_unlink = unlink(StateExtractor(state.clone()), HeaderMap::new()).await;
    assert!(matches!(anon_unlink, Err(ApiError::Unauthorized)));
    let removed = unlink(StateExtractor(state.clone()), headers.clone())
        .await
        .expect("unlink");
    assert!(removed.0["user"]["wallet"].is_null());
    let unlink_again = unlink(StateExtractor(state.clone()), headers.clone()).await;
    assert!(matches!(unlink_again, Err(ApiError::BadRequest(_))));

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
