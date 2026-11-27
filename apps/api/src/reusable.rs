//! Reusable payment codes: one offer, paid by anyone, as often as they like.
//!
//! A one-time invoice cannot do this. Settlement closes its account, so one
//! address can only ever be paid once. A reusable code is therefore not an
//! invoice but an *offer* the seller stored — a price, a currency, a title and
//! the words around it — and every visit mints a fresh invoice from it: a new
//! reference, therefore a new invoice address, with exactly the same terms.
//! Each payer pays their own invoice, and each one lands in the seller's history
//! like any other payment.
//!
//! Minting is unauthenticated on purpose: the buyer scanning a printed code has
//! no account here. The terms come from the seller's stored row, never from the
//! request, so nothing a visitor sends can change what the invoice charges.

use std::str::FromStr;

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;

use crate::payments::{
    ApiError, ApiResult, CreateOption, CreatePaymentRequest, PaymentRecord, SOL_CURRENCY,
};
use crate::AppState;

/// Where a reusable code lives in the web app: the path a QR encodes, with the
/// app's own origin in front of it.
pub const LINK_PATH_PREFIX: &str = "/r/";

/// A stored offer, as the seller sees it back.
#[derive(Debug, Clone, Serialize)]
pub struct ReusableLink {
    pub id: String,
    pub seller: String,
    pub cluster: String,
    pub currency: String,
    /// Price in the currency's base units: lamports for SOL, the token's own
    /// units for a stablecoin.
    pub amount: u64,
    pub memo: String,
    pub name: String,
    pub description: String,
    /// The code's own path, so the seller's screen never has to know the shape.
    pub path: String,
    pub created_at_unix: i64,
}

/// What the seller posts to make a reusable code.
#[derive(Debug, Deserialize)]
pub struct CreateReusableRequest {
    pub seller: String,
    pub currency: String,
    pub amount: u64,
    #[serde(default)]
    pub memo: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// The shop the code belongs to, when it was made on a shop's page.
    #[serde(default)]
    pub shop_id: Option<String>,
    pub cluster: String,
    #[serde(default)]
    pub endpoint: Option<String>,
}

/// The stored row, without the fields only a response needs.
struct StoredLink {
    seller: String,
    seller_user: String,
    cluster: String,
    endpoint: String,
    currency: String,
    amount: u64,
    memo: String,
    name: String,
    description: String,
}

impl StoredLink {
    fn offer(&self) -> Offer<'_> {
        Offer {
            seller: &self.seller,
            cluster: &self.cluster,
            endpoint: Some(self.endpoint.clone()),
            currency: &self.currency,
            amount: self.amount,
            memo: &self.memo,
            name: &self.name,
            description: &self.description,
        }
    }
}

/// The terms a code charges, without the address they are bound to. Both the
/// stored row and the request being validated reduce to this.
struct Offer<'a> {
    seller: &'a str,
    cluster: &'a str,
    endpoint: Option<String>,
    currency: &'a str,
    amount: u64,
    memo: &'a str,
    name: &'a str,
    description: &'a str,
}

/// Makes a reusable code for the signed-in seller's own wallet.
pub async fn create_link(
    db: &libsql::Database,
    user: &crate::auth::User,
    request: CreateReusableRequest,
) -> ApiResult<ReusableLink> {
    // The wallet in the request is the one that gets paid, so it has to be the
    // account's own: an offer in someone else's name would take their money.
    match user.wallet.as_deref() {
        Some(wallet) if wallet == request.seller => {}
        _ => {
            return Err(ApiError::Forbidden(
                "Link a wallet to this account before making a reusable code.".to_string(),
            ))
        }
    }
    let seller = request.seller.trim().to_string();
    if Pubkey::from_str(&seller).is_err() {
        return Err(ApiError::BadRequest(
            "seller is not a wallet address".to_string(),
        ));
    }

    let currency = request.currency.trim().to_ascii_lowercase();
    let memo = request.memo.trim().to_string();
    let name = request.name.unwrap_or_default().trim().to_string();
    let description = request.description.unwrap_or_default().trim().to_string();

    // The offer's price is validated by the same code a one-time invoice goes
    // through — currencies for the cluster, the amount, and the field limits —
    // so a reusable code can never charge something an invoice would refuse.
    // The invoice it derives is thrown away: only the offer is stored.
    let probe = invoice_request(
        &fresh_reference()?,
        &Offer {
            seller: &seller,
            cluster: &request.cluster,
            endpoint: request.endpoint.clone(),
            currency: &currency,
            amount: request.amount,
            memo: &memo,
            name: &name,
            description: &description,
        },
    );
    crate::payments::create_payment(probe)?;

    let id = crate::auth::random_id();
    let now = crate::auth::unix_now();
    let written = async {
        let connection = db.connect()?;
        connection
            .execute(
                "INSERT INTO reusable_links \
                    (id, seller, seller_user, shop_id, cluster, endpoint, currency, amount, \
                     memo, name, description, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                libsql::params![
                    id.clone(),
                    seller.clone(),
                    user.id.clone(),
                    request.shop_id.clone(),
                    request.cluster.clone(),
                    request.endpoint.clone().unwrap_or_default(),
                    currency.clone(),
                    request.amount as i64,
                    memo.clone(),
                    name.clone(),
                    description.clone(),
                    now
                ],
            )
            .await?;
        Ok::<(), crate::db::DbError>(())
    }
    .await;
    if let Err(error) = written {
        return Err(ApiError::Internal(format!(
            "reusable link not stored: {error}"
        )));
    }

    Ok(ReusableLink {
        id: id.clone(),
        seller,
        cluster: request.cluster,
        currency,
        amount: request.amount,
        memo,
        name,
        description,
        path: format!("{LINK_PATH_PREFIX}{id}"),
        created_at_unix: now,
    })
}

/// Mints one fresh invoice for a visitor of this code.
///
/// The reference is random, so two payers of the same code never share an
/// address — which is the whole point: the second payment would otherwise be
/// refused as already settled.
pub async fn mint_invoice(db: &libsql::Database, id: &str) -> ApiResult<(PaymentRecord, String)> {
    let link = load_link(db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound("This payment code does not exist.".to_string()))?;

    let reference = fresh_reference()?;
    let request = invoice_request(&reference, &link.offer());
    let mut payment = crate::payments::create_payment(request)?;
    // The invoice is recorded like any other, under the account that made the
    // offer: this is what puts a reusable code's payments in the seller's
    // history and answer the buyer's status reads from our own rows. The code's
    // id travels with the row, so history can leave this lookup out until it is
    // paid — a visit is not a sale.
    crate::payments::remember_invoice(db, &payment, Some(&link.seller_user), Some(id)).await;
    payment.seller_user = Some(link.seller_user.clone());
    payment.created_at_unix = crate::auth::unix_now();
    let qr_payload = payment.qr_payload();
    Ok((payment, qr_payload))
}

/// The invoice a code's offer describes, at one reference.
fn invoice_request(reference: &str, offer: &Offer<'_>) -> CreatePaymentRequest {
    CreatePaymentRequest {
        payment_pda: String::new(),
        seller: offer.seller.to_string(),
        reference: reference.to_string(),
        amount_lamports: if offer.currency == SOL_CURRENCY {
            offer.amount
        } else {
            0
        },
        options: vec![CreateOption {
            currency: offer.currency.to_string(),
            amount: offer.amount,
        }],
        memo: offer.memo.to_string(),
        name: Some(offer.name.to_string()),
        description: Some(offer.description.to_string()),
        items: Vec::new(),
        discount_code: None,
        allow_discounts: false,
        cluster: offer.cluster.to_string(),
        // An empty endpoint is "the cluster's own": a code made before the
        // seller pointed at a custom RPC still mints against the default one.
        endpoint: offer
            .endpoint
            .clone()
            .filter(|value| !value.trim().is_empty()),
    }
}

/// A reference no invoice has used: 32 random bytes, base58 like any address.
fn fresh_reference() -> ApiResult<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|error| ApiError::Internal(format!("reference randomness: {error}")))?;
    Ok(Pubkey::new_from_array(bytes).to_string())
}

async fn load_link(db: &libsql::Database, id: &str) -> ApiResult<Option<StoredLink>> {
    let read = async {
        let connection = db.connect()?;
        let mut rows = connection
            .query(
                "SELECT seller, seller_user, cluster, endpoint, currency, amount, \
                        memo, name, description \
                 FROM reusable_links WHERE id = ?1",
                libsql::params![id.to_string()],
            )
            .await?;
        Ok::<Option<StoredLink>, crate::db::DbError>(rows.next().await?.map(|row| StoredLink {
            seller: row.get(0).unwrap_or_default(),
            seller_user: row.get(1).unwrap_or_default(),
            cluster: row.get(2).unwrap_or_default(),
            endpoint: row.get(3).unwrap_or_default(),
            currency: row.get(4).unwrap_or_default(),
            amount: row.get::<i64>(5).unwrap_or(0).max(0) as u64,
            memo: row.get(6).unwrap_or_default(),
            name: row.get(7).unwrap_or_default(),
            description: row.get(8).unwrap_or_default(),
        }))
    }
    .await;

    match read {
        Ok(link) => Ok(link),
        Err(error) => {
            tracing::info!(%error, "a reusable link was not readable");
            Err(ApiError::Internal(
                "The payment code could not be read.".to_string(),
            ))
        }
    }
}

/// `POST /api/reusable` — the seller's own code for a repeatable price.
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateReusableRequest>,
) -> Result<Json<ReusableLink>, ApiError> {
    let user = crate::auth::require_user(&state.db, &headers).await?;
    Ok(Json(create_link(&state.db, &user, request).await?))
}

/// `POST /api/reusable/{id}/invoice` — a fresh invoice for this visit.
pub(crate) async fn invoice(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let (payment, qr_payload) = mint_invoice(&state.db, &id).await?;
    Ok(Json(
        serde_json::json!({ "payment": payment, "qrPayload": qr_payload }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored() -> StoredLink {
        StoredLink {
            seller: "9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin".to_string(),
            seller_user: "user-1".to_string(),
            cluster: "devnet".to_string(),
            endpoint: String::new(),
            currency: "usdc".to_string(),
            amount: 5_000_000,
            memo: "Album".to_string(),
            name: "Digital album".to_string(),
            description: "Five tracks".to_string(),
        }
    }

    /// Two visitors of one code must never share an invoice address: the second
    /// payment would be refused as already settled.
    #[test]
    fn every_visit_gets_its_own_invoice_address() {
        let link = stored();
        let request_for = |reference: String| invoice_request(&reference, &link.offer());

        let first = crate::payments::create_payment(request_for(fresh_reference().unwrap()))
            .expect("first invoice");
        let second = crate::payments::create_payment(request_for(fresh_reference().unwrap()))
            .expect("second invoice");

        assert_ne!(first.id, second.id);
        // The same offer either way: what the seller stored is what is charged.
        assert_eq!(first.options.len(), 1);
        assert_eq!(first.options[0].currency, "usdc");
        assert_eq!(first.options[0].amount, 5_000_000);
        assert_eq!(second.options[0].amount, 5_000_000);
        assert_eq!(first.memo, "Album");
        assert_eq!(first.name, "Digital album");
        assert_eq!(first.description, "Five tracks");
    }

    async fn test_db() -> (libsql::Database, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "payvand-reusable-test-{}.db",
            crate::auth::random_id()
        ));
        let db = libsql::Builder::new_local(&path)
            .build()
            .await
            .expect("temp db");
        crate::db::migrate(&db).await.expect("migrate");
        (db, path)
    }

    fn seller_account(wallet: &str) -> crate::auth::User {
        crate::auth::User {
            id: "user-1".to_string(),
            email: "seller@example.com".to_string(),
            wallet: Some(wallet.to_string()),
            created_at: 0,
        }
    }

    fn offer(seller: &str) -> CreateReusableRequest {
        CreateReusableRequest {
            seller: seller.to_string(),
            currency: "usdc".to_string(),
            amount: 5_000_000,
            memo: "Album".to_string(),
            name: Some("Digital album".to_string()),
            description: Some("Five tracks".to_string()),
            shop_id: None,
            cluster: "devnet".to_string(),
            endpoint: None,
        }
    }

    /// The row is the thing that repeats: one stored offer, one invoice per
    /// visit, and only the ones that are paid reach the seller's history.
    #[tokio::test]
    async fn an_offer_is_stored_and_mints_a_fresh_invoice_per_visit() {
        let (db, path) = test_db().await;
        let seller = "9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin";
        let link = create_link(&db, &seller_account(seller), offer(seller))
            .await
            .expect("stored offer");
        assert_eq!(link.path, format!("{LINK_PATH_PREFIX}{}", link.id));
        assert_eq!(link.amount, 5_000_000);

        let (first, _) = mint_invoice(&db, &link.id).await.expect("first invoice");
        let (second, _) = mint_invoice(&db, &link.id).await.expect("second invoice");

        assert_ne!(first.id, second.id);
        assert_eq!(first.options[0].amount, 5_000_000);
        assert_eq!(second.options[0].amount, 5_000_000);
        assert_eq!(first.seller, seller);
        assert_eq!(first.seller_user.as_deref(), Some("user-1"));

        // Every mint names the code it came from, which is what the history
        // filter reads.
        let connection = db.connect().expect("connect");
        let mut rows = connection
            .query(
                "SELECT reusable_id FROM invoice_meta WHERE id = ?1",
                libsql::params![first.options[0].payment_pda.clone()],
            )
            .await
            .expect("read");
        let row = rows.next().await.expect("row").expect("invoice");
        assert_eq!(row.get::<String>(0).expect("tag"), link.id);

        // Two visits, nothing sold: a lookup is not the seller's history.
        let recorded = crate::payments::recorded_invoices(&db, seller, "devnet", 10).await;
        assert!(
            recorded.is_empty(),
            "an unpaid visit must not be listed as a payment"
        );
        // It is watched instead, so the payment below is found without anyone
        // polling that invoice.
        let awaiting = crate::payments::reusable_invoices_awaiting(&db, seller, "devnet", 10).await;
        assert_eq!(awaiting.len(), 2, "both lookups are still watched");

        // Marking one settled puts exactly that one in the seller's history.
        connection
            .execute(
                "INSERT INTO settled_payments \
                    (invoice, endpoint, seller, reference, mint, amount, memo, payer, \
                     created_at, settled_at) \
                 VALUES (?1, 'https://api.devnet.solana.com', ?2, 'ref', '', 5000000, \
                         'Album', ?3, 0, 0)",
                libsql::params![
                    first.options[0].payment_pda.clone(),
                    seller.to_string(),
                    "9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin"
                ],
            )
            .await
            .expect("settlement");

        let recorded = crate::payments::recorded_invoices(&db, seller, "devnet", 10).await;
        assert_eq!(recorded.len(), 1, "the paid visit is the seller's sale");
        assert_eq!(recorded[0].id, first.options[0].payment_pda);
        assert_eq!(
            crate::payments::reusable_invoices_awaiting(&db, seller, "devnet", 10)
                .await
                .len(),
            1,
            "only the unpaid one is still watched"
        );

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn an_offer_pays_the_accounts_own_wallet_only() {
        let (db, path) = test_db().await;
        let stranger = "9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin";
        let error = create_link(&db, &seller_account("someone-else"), offer(stranger)).await;
        assert!(matches!(error, Err(ApiError::Forbidden(_))));

        // And a code nobody made is not invoiced.
        assert!(mint_invoice(&db, "missing").await.is_err());

        std::fs::remove_file(path).ok();
    }

    /// A reference is a seed in the invoice address, so it has to be a valid
    /// 32-byte address of its own.
    #[test]
    fn a_reference_is_a_usable_address() {
        let reference = fresh_reference().unwrap();
        assert!(Pubkey::from_str(&reference).is_ok());
        assert_ne!(reference, fresh_reference().unwrap());
    }
}
