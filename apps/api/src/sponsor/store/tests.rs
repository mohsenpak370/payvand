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

/// Moves a row back in time, as if it had been signed `secs` ago.
async fn age(state: &AppState, signature: &str, secs: i64) {
    state
        .db
        .connect()
        .expect("connect")
        .execute(
            "UPDATE sponsored_settlements SET created_at = ?2 WHERE id = ?1",
            (signature.to_string(), auth::unix_now() - secs),
        )
        .await
        .expect("age the row");
}

async fn signed(state: &AppState, wallet: &str, signature: &str) {
    record_signed(state, None, wallet, "devnet", 1_000_000, signature)
        .await
        .expect("record");
}

/// The rule the limits rest on: a payment that worked is never counted. A
/// landed settle refunded the relayer and an expired one was never sent, so
/// neither cost anything; only a settle that landed and failed did. A row past
/// its blockhash with no outcome yet counts as lost until it is resolved, so a
/// cluster that did not answer cannot hide a loss.
#[tokio::test]
async fn only_failed_settles_are_losses() {
    let (state, path) = test_state().await;
    for signature in ["landed", "expired", "failed", "unknown"] {
        signed(&state, "buyer", signature).await;
        age(&state, signature, RESOLVE_AFTER_SECS + 1).await;
    }
    assert_eq!(unresolved(&state, 10).await.expect("list").len(), 4);
    set_outcome(&state, "landed", Outcome::Landed)
        .await
        .expect("set");
    set_outcome(&state, "expired", Outcome::Expired)
        .await
        .expect("set");
    set_outcome(&state, "failed", Outcome::Failed)
        .await
        .expect("set");

    // `failed`, and `unknown` until the cluster says otherwise.
    assert_eq!(losses(&state, Some("buyer")).await.expect("count"), 2);
    assert_eq!(losses(&state, None).await.expect("count"), 2);
    assert_eq!(
        losses(&state, Some("someone-else")).await.expect("count"),
        0
    );
    assert_eq!(
        unresolved(&state, 10).await.expect("list"),
        vec![("unknown".to_string(), "devnet".to_string())]
    );
    set_outcome(&state, "unknown", Outcome::Landed)
        .await
        .expect("set");
    assert_eq!(losses(&state, None).await.expect("count"), 1);

    // A thousand payments that worked leave the count where it was.
    for index in 0..50 {
        let signature = format!("good-{index}");
        signed(&state, "buyer", &signature).await;
        age(&state, &signature, RESOLVE_AFTER_SECS + 1).await;
        set_outcome(&state, &signature, Outcome::Landed)
            .await
            .expect("set");
    }
    assert_eq!(losses(&state, Some("buyer")).await.expect("count"), 1);

    let _ = std::fs::remove_file(path);
}

/// In flight is what may still land: signed within the blockhash's life. It is
/// not a loss — nothing is known yet — and it stops counting once it is old
/// enough to have an outcome.
#[tokio::test]
async fn in_flight_is_only_what_may_still_land() {
    let (state, path) = test_state().await;
    signed(&state, "buyer", "fresh").await;
    signed(&state, "buyer", "old").await;
    age(&state, "old", RESOLVE_AFTER_SECS + 1).await;

    assert_eq!(in_flight(&state, Some("buyer")).await.expect("count"), 1);
    assert_eq!(in_flight(&state, None).await.expect("count"), 1);
    assert_eq!(
        unresolved(&state, 10).await.expect("list").len(),
        1,
        "the fresh one is not asked about yet"
    );
    assert!(was_signed(&state, "fresh").await.expect("lookup"));
    assert!(!was_signed(&state, "never").await.expect("lookup"));
    // A retried prepare that produced the same signature is one row.
    signed(&state, "buyer", "fresh").await;
    assert_eq!(in_flight(&state, None).await.expect("count"), 1);
    mark_submitted(&state, "fresh").await.expect("mark");

    let _ = std::fs::remove_file(path);
}
