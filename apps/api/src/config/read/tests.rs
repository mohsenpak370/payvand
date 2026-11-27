use super::*;
use crate::config::test_support::{base64_encode, policy_bytes};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// A cluster that answers with a policy and rents is read once: one account
/// read and one rent per account size the quote needs.
#[tokio::test]
async fn reads_the_policy_and_every_rent_from_the_cluster() {
    let data = policy_bytes();
    let encoded = base64_encode(&data);
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let app = axum::Router::new().route(
        "/",
        axum::routing::post(move |axum::Json(body): axum::Json<serde_json::Value>| {
            let counter = counter.clone();
            let encoded = encoded.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                let id = body.get("id").cloned().unwrap_or_default();
                let answer = match body.get("method").and_then(serde_json::Value::as_str) {
                    Some("getMinimumBalanceForRentExemption") => {
                        let space = body["params"][0].as_u64().unwrap_or(0);
                        serde_json::json!({"jsonrpc": "2.0", "id": id, "result": space * 1_000})
                    }
                    Some("getAccountInfo") => serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {"context": {"slot": 1}, "value": {
                            "data": [encoded, "base64"],
                            "executable": false,
                            "lamports": 2_000_000,
                            "owner": payments::PAYVAND_PROGRAM_ID,
                            "rentEpoch": 0,
                        }},
                    }),
                    _ => serde_json::json!({"jsonrpc": "2.0", "id": id, "result": null}),
                };
                axum::Json(answer)
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("address");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let endpoint = format!("http://{addr}/");
    let config = read_config(&endpoint, "devnet")
        .await
        .expect("the cluster answered");

    assert_eq!(config.fees.fee_bps_below, 200);
    assert_eq!(config.rent.payment, 230_000);
    assert_eq!(config.rent.token_payment, 262_000);
    assert_eq!(config.rent.payment_receipt, 48_000);
    assert_eq!(config.rent.token_account, 165_000);
    assert_eq!(config.rent.budget, 97_000);
    assert_eq!(config.rent.token_budget, 129_000);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        7,
        "one account read and six rents"
    );
}

/// A rent of zero is not an answer — every account costs something — and a
/// quote built on it would understate the rent the seller funds. The whole
/// request fails rather than being cached with zeroes in it.
#[tokio::test]
async fn a_cluster_that_answers_no_rent_is_an_error() {
    let encoded = base64_encode(&policy_bytes());
    let app = axum::Router::new().route(
        "/",
        axum::routing::post(move |axum::Json(body): axum::Json<serde_json::Value>| {
            let encoded = encoded.clone();
            async move {
                let id = body.get("id").cloned().unwrap_or_default();
                let answer = match body.get("method").and_then(serde_json::Value::as_str) {
                    Some("getMinimumBalanceForRentExemption") => {
                        serde_json::json!({"jsonrpc": "2.0", "id": id, "result": 0})
                    }
                    Some("getAccountInfo") => serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {"context": {"slot": 1}, "value": {
                            "data": [encoded, "base64"],
                            "executable": false,
                            "lamports": 2_000_000,
                            "owner": payments::PAYVAND_PROGRAM_ID,
                            "rentEpoch": 0,
                        }},
                    }),
                    _ => serde_json::json!({"jsonrpc": "2.0", "id": id, "result": null}),
                };
                axum::Json(answer)
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("address");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    assert!(
        read_config(&format!("http://{addr}/"), "devnet")
            .await
            .is_err(),
        "zero rents are refused, not quoted"
    );
}

/// A cluster that does not answer is an error, not a policy of zeroes.
#[tokio::test]
async fn a_cluster_that_does_not_answer_is_an_error() {
    let app = axum::Router::new().route(
        "/",
        axum::routing::post(
            |axum::Json(body): axum::Json<serde_json::Value>| async move {
                let id = body.get("id").cloned().unwrap_or_default();
                axum::Json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {"code": -32005, "message": "node is unhealthy"},
                }))
            },
        ),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("address");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    assert!(
        read_config(&format!("http://{addr}/"), "devnet")
            .await
            .is_err(),
        "no policy means no quote"
    );
}
