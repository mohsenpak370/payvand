//! Saved payment codes.
//!
//! An invoice is never stored by the API: the code in the QR *is* the invoice,
//! and payment status is read from the chain on demand. Saving keeps a copy of
//! that code on the account and files it under one of the seller's shops, so
//! that shop can list it again later — and the page the code opens is public,
//! so the same link can be shared with anyone.

use axum::extract::{Json, State};
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};

use crate::auth;
use crate::payments::{bad, ApiError, ApiResult, QR_PAYLOAD_VERSION};

/// Saved codes per account — plenty for a regular seller, small enough that
/// the list stays a list.
const MAX_SAVED_PER_USER: usize = 50;

/// A payment code runs a few hundred bytes; far beyond that is not a code.
const MAX_PAYLOAD_LEN: usize = 2048;

#[derive(Debug, Clone, Serialize)]
pub struct SavedPayment {
    /// Row id, so the profile can key its list.
    pub id: String,
    /// The payment's own identifier: one payment saves once per account.
    pub payment_id: String,
    /// The code exactly as scanned, so the link can be rebuilt byte for byte.
    pub payload: String,
    /// The shop this code is filed under. Codes saved before shops kept them
    /// have none.
    pub shop_id: Option<String>,
    pub created_at_unix: i64,
}

#[derive(Debug, Deserialize)]
pub struct SavePaymentRequest {
    pub payload: String,
    /// Which shop keeps this code. Optional, so a client that predates shops
    /// still saves to the account itself.
    #[serde(default)]
    pub shop_id: Option<String>,
}

/// The part of a payload this module needs: enough to recognise a real code
/// and to deduplicate, without copying the whole QR schema.
#[derive(Debug, Deserialize)]
struct CodeIdentity {
    v: u8,
    id: String,
    seller: String,
    reference: String,
    amount_lamports: u64,
    /// Present on an invoice priced in a token, which carries no lamport price
    /// at all — so the price has to be read from here.
    #[serde(default)]
    options: Vec<CodeOption>,
}

#[derive(Debug, Deserialize)]
struct CodeOption {
    currency: String,
    amount: u64,
}

fn read_payment_id(payload: &str) -> ApiResult<String> {
    if payload.len() > MAX_PAYLOAD_LEN {
        return Err(bad("That payment code is too long to save."));
    }
    let code: CodeIdentity = serde_json::from_str(payload)
        .map_err(|_| bad("That does not look like a Payvand payment code."))?;
    let plausible = [&code.id, &code.seller, &code.reference]
        .iter()
        .all(|value| !value.is_empty() && value.len() <= 64);
    // A code is priced either in lamports or in a token; only a code with no
    // price at all is not an invoice.
    let priced = code.amount_lamports > 0
        || code
            .options
            .iter()
            .any(|option| option.amount > 0 && !option.currency.trim().is_empty());
    if code.v != QR_PAYLOAD_VERSION || !plausible || !priced {
        return Err(bad("That is not a Payvand payment code."));
    }
    Ok(code.id)
}

/// The signed-in account's saved codes, newest first.
pub async fn list(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<SavedPayment>>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;

    let mut rows = connection
        .query(
            "SELECT id, payment_id, payload, created_at, shop_id FROM saved_payments \
             WHERE user_id = ?1 ORDER BY created_at DESC, rowid DESC",
            [user.id.clone()],
        )
        .await
        .map_err(auth::internal)?;

    let mut saved = Vec::new();
    while let Some(row) = rows.next().await.map_err(auth::internal)? {
        saved.push(read_row(&row)?);
    }
    Ok(Json(saved))
}

/// Saves a payment code to the signed-in account. Saving the same payment
/// twice is a no-op that answers with the existing entry, so the button can
/// be pressed again without piling up duplicates.
pub async fn save(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Json(request): Json<SavePaymentRequest>,
) -> ApiResult<Json<SavedPayment>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let payload = request.payload.trim().to_string();
    let payment_id = read_payment_id(&payload)?;
    let connection = state.db.connect().map_err(auth::internal)?;

    // The shop has to be this account's own: the id comes from the client, and
    // a code must never be filed into somebody else's shop.
    let shop_id = match request
        .shop_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        Some(shop_id) => {
            let mut rows = connection
                .query(
                    "SELECT COUNT(*) FROM shops WHERE id = ?1 AND user_id = ?2",
                    (shop_id.to_string(), user.id.clone()),
                )
                .await
                .map_err(auth::internal)?;
            let owned: i64 = match rows.next().await.map_err(auth::internal)? {
                Some(row) => row.get(0).map_err(auth::internal)?,
                None => 0,
            };
            if owned == 0 {
                return Err(bad("That shop does not exist."));
            }
            Some(shop_id.to_string())
        }
        None => None,
    };

    // One payment saves once per account: pressing the button again answers
    // with the entry that already exists, whatever shop was picked this time.
    if let Some(existing) = read_one(&connection, &user.id, &payment_id).await? {
        return Ok(Json(existing));
    }

    let mut rows = connection
        .query(
            "SELECT COUNT(*) FROM saved_payments WHERE user_id = ?1",
            [user.id.clone()],
        )
        .await
        .map_err(auth::internal)?;
    let count: i64 = match rows.next().await.map_err(auth::internal)? {
        Some(row) => row.get(0).map_err(auth::internal)?,
        None => 0,
    };
    if count >= MAX_SAVED_PER_USER as i64 {
        return Err(bad(format!(
            "You already have the maximum of {MAX_SAVED_PER_USER} saved payments."
        )));
    }

    let id = auth::random_id();
    let inserted = connection
        .execute(
            "INSERT INTO saved_payments (id, user_id, payment_id, payload, created_at, shop_id) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                id,
                user.id.clone(),
                payment_id.clone(),
                payload,
                auth::unix_now(),
                shop_id,
            ),
        )
        .await;

    // A concurrent request saved the same payment first: answer with that one,
    // so the button behaves the same whether or not the saves overlap.
    if let Err(error) = inserted {
        if !auth::is_unique_violation(&error) {
            return Err(auth::internal(error));
        }
        return match read_one(&connection, &user.id, &payment_id).await? {
            Some(saved) => Ok(Json(saved)),
            None => Err(auth::internal(error)),
        };
    }

    match read_one(&connection, &user.id, &payment_id).await? {
        Some(saved) => Ok(Json(saved)),
        None => Err(ApiError::Internal("saved payment disappeared".into())),
    }
}

async fn read_one(
    connection: &libsql::Connection,
    user_id: &str,
    payment_id: &str,
) -> ApiResult<Option<SavedPayment>> {
    let mut rows = connection
        .query(
            "SELECT id, payment_id, payload, created_at, shop_id FROM saved_payments \
             WHERE user_id = ?1 AND payment_id = ?2",
            (user_id.to_string(), payment_id.to_string()),
        )
        .await
        .map_err(auth::internal)?;
    match rows.next().await.map_err(auth::internal)? {
        Some(row) => Ok(Some(read_row(&row)?)),
        None => Ok(None),
    }
}

fn read_row(row: &libsql::Row) -> ApiResult<SavedPayment> {
    Ok(SavedPayment {
        id: row.get(0).map_err(auth::internal)?,
        payment_id: row.get(1).map_err(auth::internal)?,
        payload: row.get(2).map_err(auth::internal)?,
        created_at_unix: row.get(3).map_err(auth::internal)?,
        shop_id: row.get(4).map_err(auth::internal)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{random_id, SignupRequest};
    use axum::extract::{Json as JsonExtractor, State as StateExtractor};

    fn code(id: &str) -> String {
        serde_json::json!({
            "v": 1,
            "id": id,
            "seller": "4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J",
            "reference": "7dHbWXmci3dT8UFYWYZweBLXgycu7Y3iL6trKn1Y7ARj",
            "amount_lamports": 42_000_000,
            "memo": "coffee",
            "cluster": "devnet",
            "endpoint": "https://api.devnet.solana.com"
        })
        .to_string()
    }

    fn bearer(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            format!("Bearer {token}").parse().expect("header value"),
        );
        headers
    }

    async fn test_state() -> (crate::AppState, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!("payvand-saved-test-{}.db", random_id()));
        let db = libsql::Builder::new_local(&path)
            .build()
            .await
            .expect("temp db");
        crate::db::migrate(&db).await.expect("migrate");
        (
            crate::AppState {
                db: std::sync::Arc::new(db),
            },
            path,
        )
    }

    async fn signup(state: &crate::AppState, email: &str) -> String {
        crate::auth::signup(
            StateExtractor(state.clone()),
            JsonExtractor(SignupRequest {
                email: email.to_string(),
                password: "correct horse battery staple".into(),
            }),
        )
        .await
        .expect("signup")
        .0
        .token
    }

    #[test]
    fn reads_a_token_priced_code_too() {
        // A stablecoin invoice has no lamport price: its amount lives in the
        // option, and saving it must not depend on a SOL figure it never had.
        let token = serde_json::json!({
            "v": 1,
            "id": "A8QjzuNi3ZZPZ573QjdNyJJmcm3QuGaw89JuDXRmjkkR",
            "seller": "4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J",
            "reference": "7dHbWXmci3dT8UFYWYZweBLXgycu7Y3iL6trKn1Y7ARj",
            "amount_lamports": 0,
            "options": [
                { "currency": "usdc", "amount": 5_000_000, "decimals": 6, "mint": "", "payment_pda": "" }
            ],
            "memo": "coffee",
            "cluster": "devnet"
        })
        .to_string();
        assert_eq!(
            read_payment_id(&token).unwrap(),
            "A8QjzuNi3ZZPZ573QjdNyJJmcm3QuGaw89JuDXRmjkkR"
        );
    }

    #[test]
    fn reads_the_payment_id_from_a_code() {
        assert_eq!(read_payment_id(&code("abc")).unwrap(), "abc");
        // Trailing whitespace from a copy-paste is not a different payment.
        assert_eq!(
            read_payment_id(&format!("{}\n", code("abc"))).unwrap(),
            "abc"
        );
    }

    #[test]
    fn refuses_anything_that_is_not_a_payment_code() {
        // Not JSON at all.
        assert!(read_payment_id("hello").is_err());
        // A different payload version.
        let mut other: serde_json::Value = serde_json::from_str(&code("abc")).unwrap();
        other["v"] = 2.into();
        assert!(read_payment_id(&other.to_string()).is_err());
        // A zero amount is not an invoice.
        let mut free: serde_json::Value = serde_json::from_str(&code("abc")).unwrap();
        free["amount_lamports"] = 0.into();
        assert!(read_payment_id(&free.to_string()).is_err());
        // No seller.
        let mut sellerless: serde_json::Value = serde_json::from_str(&code("abc")).unwrap();
        sellerless["seller"] = "".into();
        assert!(read_payment_id(&sellerless.to_string()).is_err());
        // Absurdly long code.
        assert!(read_payment_id(&"x".repeat(MAX_PAYLOAD_LEN + 1)).is_err());
    }

    #[tokio::test]
    async fn saves_once_per_payment_and_keeps_accounts_apart() {
        let (state, path) = test_state().await;
        let mine = bearer(&signup(&state, "seller@example.com").await);
        let theirs = bearer(&signup(&state, "other@example.com").await);

        let anonymous = save(
            StateExtractor(state.clone()),
            HeaderMap::new(),
            JsonExtractor(SavePaymentRequest {
                payload: code("one"),
                shop_id: None,
            }),
        )
        .await;
        assert!(matches!(anonymous, Err(ApiError::Unauthorized)));

        let first = save(
            StateExtractor(state.clone()),
            mine.clone(),
            JsonExtractor(SavePaymentRequest {
                payload: code("one"),
                shop_id: None,
            }),
        )
        .await
        .expect("saved")
        .0;
        assert_eq!(first.payment_id, "one");
        assert_eq!(first.payload, code("one"));

        // Pressing save again answers with the same entry rather than a copy.
        let again = save(
            StateExtractor(state.clone()),
            mine.clone(),
            JsonExtractor(SavePaymentRequest {
                payload: code("one"),
                shop_id: None,
            }),
        )
        .await
        .expect("saved again")
        .0;
        assert_eq!(again.id, first.id);

        let second = save(
            StateExtractor(state.clone()),
            mine.clone(),
            JsonExtractor(SavePaymentRequest {
                payload: code("two"),
                shop_id: None,
            }),
        )
        .await
        .expect("second payment saved")
        .0;
        assert_eq!(second.payment_id, "two");

        let saved = list(StateExtractor(state.clone()), mine)
            .await
            .expect("list")
            .0;
        assert_eq!(saved.len(), 2);
        let ids: Vec<&str> = saved.iter().map(|item| item.payment_id.as_str()).collect();
        assert!(ids.contains(&"one") && ids.contains(&"two"));

        // Another account sees none of it.
        let theirs_list = list(StateExtractor(state.clone()), theirs)
            .await
            .expect("list")
            .0;
        assert!(theirs_list.is_empty());

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn files_a_code_under_the_accounts_own_shop_only() {
        let (state, path) = test_state().await;
        let mine = bearer(&signup(&state, "seller@example.com").await);
        let connection = state.db.connect().expect("connect");

        let used_shop = {
            let mut rows = connection
                .query(
                    "SELECT id FROM users WHERE email = 'seller@example.com'",
                    (),
                )
                .await
                .expect("user");
            let id: String = rows
                .next()
                .await
                .expect("row")
                .expect("user")
                .get(0)
                .expect("id");
            connection
                .execute(
                    "INSERT INTO shops (id, user_id, name, description, created_at) \
                     VALUES ('shop-1', ?1, 'Corner Shop', '', 1)",
                    [id],
                )
                .await
                .expect("shop");
            "shop-1"
        };

        let saved = save(
            StateExtractor(state.clone()),
            mine.clone(),
            JsonExtractor(SavePaymentRequest {
                payload: code("one"),
                shop_id: Some(used_shop.into()),
            }),
        )
        .await
        .expect("saved")
        .0;
        assert_eq!(saved.shop_id.as_deref(), Some(used_shop));

        // A shop id from anywhere else is refused rather than honoured.
        let refused = save(
            StateExtractor(state.clone()),
            mine,
            JsonExtractor(SavePaymentRequest {
                payload: code("two"),
                shop_id: Some("somebody-elses-shop".into()),
            }),
        )
        .await;
        assert!(refused.is_err());

        let _ = std::fs::remove_file(path);
    }
}
