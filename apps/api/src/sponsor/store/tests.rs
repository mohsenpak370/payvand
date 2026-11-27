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
