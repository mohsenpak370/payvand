//! Shared test fixtures for payment unit tests.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use solana_pubkey::Pubkey;

use crate::payments::onchain::reader::{discriminator_for, event_discriminator_for};
use crate::payments::CreatePaymentRequest;

pub(crate) fn sample_request() -> CreatePaymentRequest {
    CreatePaymentRequest {
        payment_pda: "4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J".to_string(),
        seller: "4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J".to_string(),
        reference: "4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J".to_string(),
        amount_lamports: 1_000_000,
        options: Vec::new(),
        memo: "coffee".to_string(),
        name: None,
        description: None,
        items: Vec::new(),
        discount_code: None,
        allow_discounts: false,
        cluster: "devnet".to_string(),
        endpoint: None,
    }
}

/// The fee policy is a program-owned account shaped nothing like a
/// payment. `get_program_accounts` returns it alongside invoices, so the
/// scan has to skip it — an error here would take the whole payment list
/// down the moment `Config` is initialized.
pub(crate) fn config_data(session_fee_float: u64) -> Vec<u8> {
    let mut data = discriminator_for("Config").to_vec();
    data.extend_from_slice(&[6u8; 32]); // admin
    data.extend_from_slice(&100u16.to_le_bytes()); // fee_bps_below
    data.extend_from_slice(&50u16.to_le_bytes()); // fee_bps_above
    data.extend_from_slice(&100_000_000u64.to_le_bytes()); // sol_tier_lamports
    data.extend_from_slice(&10_000_000u64.to_le_bytes()); // token_tier_base_units
    data.extend_from_slice(&62_782u64.to_le_bytes()); // flat_usdc_base_units
    data.extend_from_slice(&57_346u64.to_le_bytes()); // flat_eurc_base_units
    data.extend_from_slice(&[4u8; 32]); // usdc_mint
    data.extend_from_slice(&[5u8; 32]); // eurc_mint
    data.extend_from_slice(&5_000u64.to_le_bytes()); // network_fee_lamports
    data.extend_from_slice(&351u64.to_le_bytes()); // network_fee_usdc_base_units
    data.extend_from_slice(&320u64.to_le_bytes()); // network_fee_eurc_base_units
    data.push(0); // fee_mode: SellerPays
    data.extend_from_slice(&session_fee_float.to_le_bytes());
    data.extend_from_slice(&1_760_000_000i64.to_le_bytes()); // flats_updated_at
    data.push(251); // bump
    data.push(250); // treasury_bump
    data.push(4); // version
    data
}

/// The pre-marker `PaymentReceipt` account — 301 bytes with a full-length
/// memo. Same discriminator and same invoice offset as the marker, then
/// seller, reference, mint, amount, memo, payer and three timestamps, so its
/// `settled_at` is nowhere near the offset the marker decoder reads.
pub(crate) fn old_receipt_data(invoice: Pubkey) -> Vec<u8> {
    let mut data = discriminator_for("PaymentReceipt").to_vec();
    data.extend_from_slice(invoice.as_ref());
    data.extend_from_slice(&[7u8; 32]); // seller
    data.extend_from_slice(&[9u8; 32]); // reference
    data.extend_from_slice(&[0u8; 32]); // mint: SOL
    data.extend_from_slice(&42u64.to_le_bytes()); // amount
    let memo = [b'm'; 96];
    data.extend_from_slice(&(memo.len() as u32).to_le_bytes());
    data.extend_from_slice(&memo);
    data.extend_from_slice(&[5u8; 32]); // payer
    data.extend_from_slice(&100i64.to_le_bytes()); // created_at
    data.extend_from_slice(&200i64.to_le_bytes()); // settled_at
    data.extend_from_slice(&300i64.to_le_bytes()); // closed_at
    data.push(255); // bump
    data
}

/// The marker as the program writes it: discriminator, invoice, `settled_at`.
pub(crate) fn receipt_data(invoice: Pubkey, settled_at: i64) -> Vec<u8> {
    let mut data = discriminator_for("PaymentReceipt").to_vec();
    data.extend_from_slice(invoice.as_ref());
    data.extend_from_slice(&settled_at.to_le_bytes());
    data
}

/// The `PaymentSettled` log line a settle transaction carries, built in the
/// program's field order: amount, fee, seller_proceeds, payer, seller, mint,
/// reference, invoice, memo.
pub(crate) fn settled_log_line(invoice: Pubkey, mint: Pubkey, amount: u64, memo: &str) -> String {
    let mut body = event_discriminator_for("PaymentSettled").to_vec();
    body.extend_from_slice(&amount.to_le_bytes());
    body.extend_from_slice(&10u64.to_le_bytes()); // fee
    body.extend_from_slice(&(amount - 10).to_le_bytes()); // seller_proceeds
    body.extend_from_slice(&[5u8; 32]); // payer
    body.extend_from_slice(&[7u8; 32]); // seller
    body.extend_from_slice(mint.as_ref());
    body.extend_from_slice(&[9u8; 32]); // reference
    body.extend_from_slice(invoice.as_ref());
    body.extend_from_slice(&(memo.len() as u32).to_le_bytes());
    body.extend_from_slice(memo.as_bytes());
    format!("Program data: {}", STANDARD.encode(body))
}

/// A database on disk, so two connections see the same rows: an in-memory
/// SQLite belongs to one connection, which is not what a cold instance and
/// the instance that filled the cache before it look like.
pub(crate) async fn temp_db() -> (libsql::Database, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!(
        "payvand-settlement-{}.db",
        crate::auth::random_id()
    ));
    let db = libsql::Builder::new_local(&path)
        .build()
        .await
        .expect("temp db");
    crate::db::migrate(&db).await.expect("schema");
    (db, path)
}

/// A stand-in cluster: answers every JSON-RPC batch from `answers`, and
/// counts the HTTP requests it received so a test can see the batching.
pub(crate) async fn stub_rpc(
    answers: impl Fn(&serde_json::Value) -> serde_json::Value + Send + Sync + 'static,
) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = hits.clone();
    let answers = std::sync::Arc::new(answers);
    let app = axum::Router::new().route(
        "/",
        axum::routing::post(move |axum::Json(body): axum::Json<serde_json::Value>| {
            let answers = answers.clone();
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                // A batch is answered in kind; a single request — what
                // `RpcClient` posts — gets its own object back, not an array.
                let batch = body.as_array().cloned();
                let calls = batch.clone().unwrap_or_else(|| vec![body]);
                let answered: Vec<serde_json::Value> =
                    calls.iter().map(|call| (answers)(call)).collect();
                axum::Json(match batch {
                    Some(_) => serde_json::Value::Array(answered),
                    None => answered
                        .into_iter()
                        .next()
                        .unwrap_or(serde_json::Value::Null),
                })
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
    (format!("http://{addr}/"), hits)
}
