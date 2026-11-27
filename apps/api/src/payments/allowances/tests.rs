use super::*;
use crate::payments::onchain::reader::discriminator_for;
use crate::payments::test_support::{stub_rpc, temp_db};
use crate::payments::PAYVAND_PROGRAM_ID;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;

/// A session key is kept against the wallet it spends for, and no other
/// wallet, cluster or endpoint reads it back.
#[tokio::test]
async fn a_session_key_is_kept_for_the_wallet_it_spends_for() {
    let (db, path) = temp_db().await;
    let wallet = "wallet-a";
    let key = Pubkey::from([0x61u8; 32]);
    let endpoint = "https://allowance-a.invalid";
    assert!(
        session_keys_for(&db, wallet, "devnet", endpoint)
            .await
            .is_empty(),
        "nothing has been read yet"
    );

    remember_session_key(&db, &key.to_string(), wallet, "devnet", endpoint).await;
    assert_eq!(
        session_keys_for(&db, wallet, "devnet", endpoint).await,
        vec![key]
    );
    assert!(
        session_keys_for(&db, "wallet-b", "devnet", endpoint)
            .await
            .is_empty(),
        "another wallet has no key"
    );
    assert!(
        session_keys_for(&db, wallet, "mainnet-beta", endpoint)
            .await
            .is_empty(),
        "another cluster is another program"
    );
    let _ = std::fs::remove_file(path);
}

/// One bounded read of the addresses a buyer's budgets live at is enough to
/// learn its session key — the program is not scanned.
#[tokio::test]
async fn an_allowance_is_read_from_its_own_addresses() {
    let (db, path) = temp_db().await;
    let buyer = Pubkey::from([0x66u8; 32]);
    let session_key = Pubkey::from([0x67u8; 32]);
    let mut budget = discriminator_for("Budget").to_vec();
    budget.extend_from_slice(buyer.as_ref());
    budget.extend_from_slice(session_key.as_ref());
    budget.extend_from_slice(&10u64.to_le_bytes());
    budget.extend_from_slice(&0u64.to_le_bytes());
    budget.extend_from_slice(&1u64.to_le_bytes());
    budget.push(255);
    let account = serde_json::json!({
        "data": [STANDARD.encode(&budget), "base64"],
        "executable": false,
        "lamports": 1000,
        "owner": PAYVAND_PROGRAM_ID,
        "rentEpoch": 0,
    });
    let (endpoint, hits) = stub_rpc(move |call| {
        let id = call.get("id").cloned().unwrap_or_default();
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {"context": {"slot": 1}, "value": [account.clone(), null, null]},
        })
    })
    .await;

    let found = detect_allowance(&db, &endpoint, "devnet", &buyer)
        .await
        .expect("the cluster answered");
    assert_eq!(found, vec![session_key]);
    assert_eq!(
        hits.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "one read, whatever the buyer has"
    );
    assert_eq!(
        session_keys_for(&db, &buyer.to_string(), "devnet", &endpoint).await,
        vec![session_key],
        "and it is kept for the next load"
    );
    let _ = std::fs::remove_file(path);
}

/// A cluster that does not answer is not an answer: nothing is remembered,
/// and the wallet can be looked up again.
#[tokio::test]
async fn an_allowance_lookup_that_failed_learns_nothing() {
    let (db, path) = temp_db().await;
    let buyer = Pubkey::from([0x68u8; 32]);
    let (endpoint, _) = stub_rpc(|call| {
        let id = call.get("id").cloned().unwrap_or_default();
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": -32005, "message": "node is unhealthy"},
        })
    })
    .await;

    assert!(detect_allowance(&db, &endpoint, "devnet", &buyer)
        .await
        .is_none());
    assert!(
        session_keys_for(&db, &buyer.to_string(), "devnet", &endpoint)
            .await
            .is_empty()
    );
    let _ = std::fs::remove_file(path);
}

/// The lookup is once a window, not once a load: a wallet with no allowance
/// is remembered as checked, and looked at again when the window turns.
#[tokio::test]
async fn a_checked_wallet_is_not_looked_up_again_inside_the_window() {
    let (db, path) = temp_db().await;
    let wallet = "wallet-c";
    let endpoint = "https://allowance-c.invalid";
    assert!(
        allowance_check_due(&db, wallet, "devnet", endpoint).await,
        "never looked up"
    );
    mark_allowance_checked(&db, wallet, "devnet", endpoint).await;
    assert!(!allowance_check_due(&db, wallet, "devnet", endpoint).await);

    let connection = db.connect().expect("connect");
    connection
        .execute(
            "UPDATE allowance_checks SET checked_at = ?1 WHERE wallet = ?2",
            libsql::params![
                crate::auth::unix_now() - ALLOWANCE_CHECK_SECS - 1,
                wallet.to_string()
            ],
        )
        .await
        .expect("backdate");
    assert!(
        allowance_check_due(&db, wallet, "devnet", endpoint).await,
        "an old look is due again"
    );
    let _ = std::fs::remove_file(path);
}
