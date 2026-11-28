use super::*;

async fn test_state() -> (AppState, std::path::PathBuf) {
    use std::sync::Arc;
    let path = std::env::temp_dir().join(format!("payvand-sponsor-test-{}.db", auth::random_id()));
    let db = libsql::Builder::new_local(&path)
        .build()
        .await
        .expect("temp db");
    crate::db::migrate(&db).await.expect("migrate");
    (AppState { db: Arc::new(db) }, path)
}

/// The deployment's cap counts every settle the relayer signed, however it
/// reached the cluster; a wallet's cap counts only the ones this API submitted
/// for it. A prepare names a wallet without proving it, so counting prepares
/// against the wallet would let a stranger spend somebody else's allowance.
#[tokio::test]
async fn the_global_cap_counts_signatures_and_a_wallet_cap_counts_submissions() {
    let (state, path) = test_state().await;
    assert_eq!(
        recent_count(&state, Some("buyer-wallet"))
            .await
            .expect("count"),
        0
    );
    assert_eq!(recent_count(&state, None).await.expect("count"), 0);

    for index in 0..3 {
        record_signed(
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
    // Signed, not yet sent by us: the deployment has spent three signatures,
    // the wallet nothing.
    assert_eq!(recent_count(&state, None).await.expect("count"), 3);
    assert_eq!(
        recent_count(&state, Some("buyer-wallet"))
            .await
            .expect("count"),
        0
    );
    assert!(was_signed(&state, "signature-1").await.expect("lookup"));
    assert!(!was_signed(&state, "signature-9").await.expect("lookup"));

    mark_submitted(&state, "signature-1").await.expect("mark");
    // A retried prepare that produced the same signature is one row.
    record_signed(
        &state,
        None,
        "buyer-wallet",
        "devnet",
        1_000_000,
        "signature-1",
    )
    .await
    .expect("idempotent");
    assert_eq!(
        recent_count(&state, Some("buyer-wallet"))
            .await
            .expect("count"),
        1
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
