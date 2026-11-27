//! `GET /api/me/transactions`: the linked wallet's history, from our own rows
//! first and the program scan only for what they cannot answer.

use axum::extract::{Json, Query, State};
use axum::http::HeaderMap;

use crate::auth;
use crate::payments::{self, ApiError, ApiResult, PaymentRecord, PaymentStatus};

use super::matching::match_transactions;
use super::meta::fill_invoice_meta;
use super::model::{FeedQuery, TransactionsResponse};

/// The newest transactions returned by `GET /api/me/transactions`.
const TRANSACTION_LIMIT: usize = 50;

fn wants_scan(query: &FeedQuery) -> bool {
    query
        .scan
        .as_deref()
        .is_some_and(|value| value == "1" || value == "true")
}

pub async fn transactions(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Query(query): Query<FeedQuery>,
) -> ApiResult<Json<TransactionsResponse>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let wallet = user.wallet.ok_or_else(|| {
        ApiError::BadRequest("Link your wallet on the profile page first.".into())
    })?;

    let cluster = payments::default_cluster();
    let endpoint = payments::resolve_endpoint(None, &cluster)?;

    // A row from before this API kept amounts cannot be listed — it has nothing
    // to show and no currency to say — so a seller returning to such a history
    // would fall back to the program scan on every load. Filling the page from
    // the chain once ends that: the rows then answer like any other.
    payments::backfill_invoice_rows(&state.db, &wallet, &cluster, &endpoint).await;

    // Our own rows answer most of this: the invoices this wallet made, and the
    // settlements it paid. The cluster is asked only what a row cannot say —
    // whether each invoice has settled — and that is one batched read for the
    // page instead of a scan of every account the program owns.
    let own = payments::recorded_invoices(&state.db, &wallet, &cluster, TRANSACTION_LIMIT).await;
    // The invoices a reusable code minted and nobody paid are not history, but
    // the newest of them are still watched: a payment made while no one was
    // polling its status is discovered here, stored, and listed from then on.
    let awaiting = payments::reusable_invoices_awaiting(
        &state.db,
        &wallet,
        &cluster,
        payments::AWAITING_REUSABLE_LIMIT,
    )
    .await;
    // What only the cluster knows: whether each of those invoices has settled,
    // and by whom. One batched read for the page, and a row that survives the
    // cluster not answering at all.
    let ids: Vec<String> = own
        .iter()
        .chain(awaiting.iter())
        .map(|record| record.id.clone())
        .collect();
    let settled = if ids.is_empty() {
        std::collections::HashMap::new()
    } else {
        payments::fetch_many(
            payments::WatchRequest {
                pdas: ids,
                cluster: Some(cluster.clone()),
                endpoint: Some(endpoint.clone()),
            },
            &state.db,
        )
        .await
        .map(|records| {
            records
                .into_iter()
                .map(|record| (record.id.clone(), record))
                .collect::<std::collections::HashMap<_, _>>()
        })
        .unwrap_or_default()
    };
    let mut records: Vec<PaymentRecord> = own
        .into_iter()
        .map(|row| match settled.get(&row.id) {
            // A settlement is the whole record: it names the payer and the time.
            Some(chain) if chain.status == PaymentStatus::Settled => chain.clone(),
            Some(chain) => PaymentRecord {
                status: chain.status,
                ..row
            },
            None => row,
        })
        .collect();
    // Only a lookup that was paid belongs on the page: the rest stay watched and
    // unlisted until they settle. A paid one is marked as such, so it stays
    // history even if its settlement row was never written.
    let paid: Vec<PaymentRecord> = awaiting
        .into_iter()
        .filter_map(|row| {
            settled
                .get(&row.id)
                .filter(|chain| chain.status == PaymentStatus::Settled)
                .cloned()
        })
        .collect();
    let paid_ids: Vec<String> = paid.iter().map(|record| record.id.clone()).collect();
    payments::mark_reusable_paid(&state.db, &paid_ids).await;
    records.extend(paid);
    // A payment from an allowance is signed by its session key, so its settle
    // names a payer that is not the wallet. Which key spends for which wallet is
    // on chain — the budget account holds both — so it is looked up once and
    // kept; every load after that matches the payments with a row read.
    let mut session_keys =
        payments::session_keys_for(&state.db, &wallet, &cluster, &endpoint).await;
    let mut found_allowance = false;
    if session_keys.is_empty()
        && payments::allowance_check_due(&state.db, &wallet, &cluster, &endpoint).await
    {
        if let Ok(buyer) = payments::parse_pubkey(&wallet, "wallet") {
            if let Some(found) =
                payments::detect_allowance(&state.db, &endpoint, &cluster, &buyer).await
            {
                payments::mark_allowance_checked(&state.db, &wallet, &cluster, &endpoint).await;
                session_keys = found;
                found_allowance = !session_keys.is_empty();
            }
        }
    }

    let mut payers = vec![wallet.clone()];
    payers.extend(session_keys.iter().map(|key| key.to_string()));
    records.extend(
        payments::settlements_paid_by_any(
            &state.db,
            &endpoint,
            &cluster,
            &payers,
            TRANSACTION_LIMIT,
        )
        .await,
    );

    // The allowances this wallet is known to have, in the shape the matcher
    // reads: a payment whose payer is one of these keys is this buyer's.
    let mut budgets: Vec<payments::BudgetAccount> = match payments::parse_pubkey(&wallet, "wallet")
    {
        Ok(buyer) => session_keys
            .iter()
            .map(|key| payments::BudgetAccount {
                buyer,
                session_key: *key,
            })
            .collect(),
        Err(_) => Vec::new(),
    };

    // The fallback: the program scan, for anything we did not record — a code
    // another client made, an invoice from before this API kept rows, or a
    // payment from an allowance it has no invoice for. It also backfills what it
    // finds, so this is the cold path rather than the usual one.
    //
    // An allowance that was just found is scanned for once: its payments are not
    // ours to have rows for until a scan has read them.
    if wants_scan(&query) || records.is_empty() || found_allowance {
        let scan = payments::fetch_program_scan(TRANSACTION_LIMIT, &state.db).await?;
        for record in scan.payments {
            if !records.iter().any(|known| known.id == record.id) {
                records.push(record);
            }
        }
        payments::remember_budgets(&state.db, &scan.budgets, &cluster, &endpoint).await;
        budgets.extend(scan.budgets);
        payments::remember_scanned(&state.db, &records).await;
    }

    // One row per invoice: an invoice this wallet both made and paid is one
    // payment, and the settled copy is the one worth showing.
    let mut unique: Vec<PaymentRecord> = Vec::with_capacity(records.len());
    for record in records {
        match unique.iter().position(|known| known.id == record.id) {
            Some(index) => {
                if record.status == PaymentStatus::Settled {
                    unique[index] = record;
                }
            }
            None => unique.push(record),
        }
    }

    let mut items = match_transactions(unique, &budgets, &wallet);
    // The count the card shows is everything the wallet has, which is not the
    // page it was cut down to.
    let total = items.len();
    items.truncate(TRANSACTION_LIMIT);
    fill_invoice_meta(&state.db, &mut items).await;

    Ok(Json(TransactionsResponse {
        wallet,
        total,
        cluster,
        items,
    }))
}
