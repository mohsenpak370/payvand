//! Storing a seller's offer, and minting a visitor's invoice from it.

use std::str::FromStr;

use solana_pubkey::Pubkey;

use crate::payments::{ApiError, ApiResult, PaymentRecord};

use super::model::{CreateReusableRequest, ReusableLink, LINK_PATH_PREFIX};
use super::offer::{fresh_reference, invoice_request, Offer, StoredLink};

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

#[cfg(test)]
mod tests;
