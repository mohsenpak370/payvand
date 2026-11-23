//! Web push for the installed app.
//!
//! Three pieces live here: the VAPID public key the browser subscribes with,
//! the per-device subscriptions an account stores, and the sender — used by the
//! test button in Settings and by the daily cron that reminds people their
//! monthly allowance reset without the pot being refilled.
//!
//! The private key never leaves this process: subscriptions are rows, the
//! payload is encrypted per device by `web-push`, and the only thing the browser
//! ever learns is the public key in `/api/push/key`.
//!
//! A nudge is about on-chain state the API does not keep, so the cron reads the
//! budget accounts straight off the cluster: one `getMultipleAccounts` for every
//! linked wallet that has a subscription. It runs once a month per account —
//! `push_nudges` is the record of what was already sent.

use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

use axum::extract::{Json, State};
use axum::http::{header, HeaderMap};
use serde::Deserialize;
use serde_json::{json, Value};
use solana_commitment_config::CommitmentConfig;
use solana_pubkey::Pubkey;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use web_push::{
    request_builder, ContentEncoding, SubscriptionInfo, VapidSignatureBuilder,
    WebPushMessageBuilder,
};

use crate::auth;
use crate::payments::{bad, mint_for, ApiError, ApiResult};
use crate::push_budget::{
    budget_address, budget_facts, current_month_index, needs_nudge, token_amount,
    token_budget_address, token_budget_facts, token_vault_address, BudgetFacts,
};

/// The most accounts one `getMultipleAccounts` call may name.
const MAX_ACCOUNT_READS: usize = 100;

/// The devnet deployment. A mainnet one is configured with
/// `PAYVAND_PROGRAM_ID`, because the address is per cluster.
const DEVNET_PROGRAM_ID: &str = "4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J";

/// Locales the copy below is written in; anything else is answered in English.
const LOCALES: &[&str] = &["en", "de", "es", "fa", "fr"];

/// Hosts a browser's push service can live on. An endpoint arrives from a
/// client and the server POSTs to it, so without this list the endpoint would
/// be a request-forgery tool aimed at whatever the function can reach.
const PUSH_HOSTS: &[&str] = &[
    "fcm.googleapis.com",
    "android.googleapis.com",
    "updates.push.services.mozilla.com",
    "push.services.mozilla.com",
    "notify.windows.com",
    "push.apple.com",
    "push.services.apple.com",
    "wns2-par02p.notify.windows.com",
];

#[derive(Debug, Deserialize)]
pub struct SubscriptionKeys {
    pub p256dh: String,
    pub auth: String,
}

#[derive(Debug, Deserialize)]
pub struct SubscribeRequest {
    pub endpoint: String,
    pub keys: SubscriptionKeys,
    #[serde(default)]
    pub locale: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UnsubscribeRequest {
    pub endpoint: String,
}

/// One stored device subscription, as a send needs it.
#[derive(Debug, Clone)]
pub struct StoredSubscription {
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    pub locale: String,
    pub user_id: String,
    pub wallet: Option<String>,
}

/// What a push service answered: delivered, or the subscription is gone and the
/// row should be removed rather than retried forever.
#[derive(Debug, PartialEq, Eq)]
enum Delivery {
    Sent,
    Expired,
}

/// The public half of the VAPID key pair, which the browser subscribes with.
/// `null` when push is not configured, so Settings can say so instead of the
/// browser subscribing to a key that nobody can sign with.
pub async fn key() -> Json<Value> {
    Json(json!({ "publicKey": env_value("VAPID_PUBLIC_KEY") }))
}

pub async fn subscribe(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Json(request): Json<SubscribeRequest>,
) -> ApiResult<Json<Value>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let endpoint = validated_endpoint(&request.endpoint)?;
    let (p256dh, auth_key) = validated_keys(&request.keys)?;
    let locale = request
        .locale
        .as_deref()
        .filter(|code| LOCALES.contains(code))
        .unwrap_or("en")
        .to_string();

    let connection = state.db.connect().map_err(auth::internal)?;
    // The endpoint is the device, so re-subscribing updates the row rather than
    // adding one: a browser that rotates its keys keeps a single notification.
    connection
        .execute(
            "INSERT INTO push_subscriptions (endpoint, user_id, p256dh, auth, locale, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
             ON CONFLICT(endpoint) DO UPDATE SET \
                 user_id = excluded.user_id, \
                 p256dh = excluded.p256dh, \
                 auth = excluded.auth, \
                 locale = excluded.locale",
            (
                endpoint,
                user.id,
                p256dh,
                auth_key,
                locale,
                auth::unix_now(),
            ),
        )
        .await
        .map_err(auth::internal)?;

    Ok(Json(json!({ "ok": true })))
}

/// Removes this device. Scoped to the account, so an endpoint seen in a request
/// cannot unsubscribe someone else's browser.
pub async fn unsubscribe(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Json(request): Json<UnsubscribeRequest>,
) -> ApiResult<Json<Value>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let connection = state.db.connect().map_err(auth::internal)?;
    connection
        .execute(
            "DELETE FROM push_subscriptions WHERE endpoint = ?1 AND user_id = ?2",
            (request.endpoint, user.id),
        )
        .await
        .map_err(auth::internal)?;
    Ok(Json(json!({ "ok": true })))
}

/// One notification to this account's own devices — the Settings button that
/// proves the pipeline works.
pub async fn test(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    let user = auth::require_user(&state.db, &headers).await?;
    let devices = subscriptions_for(&state, Some(&user.id)).await?;
    if devices.is_empty() {
        return Err(bad(
            "This account has no device subscribed to notifications.",
        ));
    }

    let mut sent = 0;
    let mut failure = None;
    for device in &devices {
        let (title, body) = test_copy(&device.locale);
        match deliver(device, &payload(title, body, "/settings")).await {
            Ok(Delivery::Sent) => sent += 1,
            Ok(Delivery::Expired) => remove_subscription(&state, &device.endpoint).await?,
            Err(error) => failure = Some(error),
        }
    }
    Ok(Json(
        json!({ "ok": sent > 0, "sent": sent, "failure": failure }),
    ))
}

/// The daily cron: remind everyone whose allowance rolled into a new month with
/// a pot that was never refilled — a SOL budget, or a USDC or EURC token budget,
/// whichever came up short. Vercel sends `Authorization: Bearer $CRON_SECRET`;
/// without that variable the endpoint refuses to run at all.
pub async fn nudge(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    require_cron_secret(&headers)?;

    let now = current_month_index(auth::unix_now());
    let devices = subscriptions_for(&state, None).await?;
    if devices.is_empty() {
        return Ok(Json(json!({ "checked": 0, "sent": 0 })));
    }

    // One read for every wallet with a subscription, covering all three
    // allowances the app can open: the SOL budget, and a token budget for each
    // stablecoin the cluster has. A wallet that cannot be parsed or has no
    // budget account simply has nothing to remind about.
    let program = program_id()?;
    let cluster = cluster_name();
    let rpc = RpcClient::new_with_commitment(rpc_url(), CommitmentConfig::confirmed());
    let mints: Vec<Pubkey> = ["usdc", "eurc"]
        .iter()
        .filter_map(|currency| mint_for(currency, &cluster))
        .collect();

    let mut wallets: Vec<(Pubkey, String)> = Vec::new();
    for device in &devices {
        let Some(wallet) = device.wallet.as_deref() else {
            continue;
        };
        if let Ok(address) = Pubkey::from_str(wallet) {
            if !wallets.iter().any(|(_, known)| known == wallet) {
                wallets.push((address, wallet.to_string()));
            }
        }
    }

    // The SOL budget per wallet, then each token budget with the vault that
    // holds its pot. Deriving every address first keeps one RPC call for all of
    // them: a wallet with no allowance simply comes back as a missing account.
    let sol_budgets: Vec<Pubkey> = wallets
        .iter()
        .map(|(address, _)| budget_address(address, &program))
        .collect();
    let token_budgets: Vec<(usize, Pubkey, Pubkey)> = wallets
        .iter()
        .enumerate()
        .flat_map(|(index, (wallet, _))| {
            mints.iter().map(move |mint| {
                let budget = token_budget_address(wallet, mint, &program);
                (index, budget, token_vault_address(&budget, mint))
            })
        })
        .collect();

    let mut addresses = sol_budgets.clone();
    addresses.extend(token_budgets.iter().map(|(_, budget, _)| *budget));
    addresses.extend(token_budgets.iter().map(|(_, _, vault)| *vault));
    let mut accounts = Vec::with_capacity(addresses.len());
    for chunk in addresses.chunks(MAX_ACCOUNT_READS) {
        accounts.extend(
            rpc.get_multiple_accounts(chunk)
                .await
                .map_err(|error| ApiError::Internal(format!("budget read failed: {error}")))?,
        );
    }

    let sol_accounts = &accounts[..sol_budgets.len()];
    let token_start = sol_budgets.len();
    let token_accounts = &accounts[token_start..token_start + token_budgets.len()];
    let vault_accounts = &accounts[token_start + token_budgets.len()..];

    // Every allowance a wallet has, so a pot in any currency can earn the
    // reminder. A token budget's pot is its vault's balance: the tokens sit in
    // the vault, not in the budget account.
    let mut budgets: HashMap<String, Vec<BudgetFacts>> = HashMap::new();
    for ((_, wallet), account) in wallets.iter().zip(sol_accounts) {
        if let Some(facts) = account
            .as_ref()
            .and_then(|account| budget_facts(&account.data, account.lamports))
        {
            budgets.entry(wallet.clone()).or_default().push(facts);
        }
    }
    for ((index, _, _), (budget, vault)) in token_budgets
        .iter()
        .zip(token_accounts.iter().zip(vault_accounts))
    {
        let Some(budget) = budget else {
            continue;
        };
        let pot = vault
            .as_ref()
            .and_then(|vault| token_amount(&vault.data))
            .unwrap_or(0);
        if let Some(facts) = token_budget_facts(&budget.data, pot) {
            budgets
                .entry(wallets[*index].1.clone())
                .or_default()
                .push(facts);
        }
    }

    // One account can have several devices; the reminder is one per account per
    // month, so the devices are grouped and every one of them is told once.
    let mut order: Vec<String> = Vec::new();
    let mut by_user: HashMap<String, Vec<&StoredSubscription>> = HashMap::new();
    for device in &devices {
        by_user
            .entry(device.user_id.clone())
            .or_insert_with(|| {
                order.push(device.user_id.clone());
                Vec::new()
            })
            .push(device);
    }

    let mut checked = 0;
    let mut sent = 0;
    let mut skipped = 0;
    for user_id in order {
        let group = by_user.get(&user_id).expect("grouped above");
        let facts = group
            .iter()
            .find_map(|device| device.wallet.as_deref())
            .and_then(|wallet| budgets.get(wallet));
        let Some(facts) = facts else {
            skipped += 1;
            continue;
        };
        checked += 1;
        // Any allowance that rolled over with a short pot earns the reminder,
        // whatever currency it is in; the copy names no currency.
        if !facts.iter().any(|facts| needs_nudge(facts, now))
            || already_nudged(&state, &user_id, now).await?
        {
            continue;
        }

        let mut delivered = false;
        for device in group {
            let (title, body) = nudge_copy(&device.locale);
            match deliver(device, &payload(title, body, "/budget")).await {
                Ok(Delivery::Sent) => {
                    delivered = true;
                    sent += 1;
                }
                Ok(Delivery::Expired) => remove_subscription(&state, &device.endpoint).await?,
                Err(error) => tracing::warn!(%error, endpoint = %device.endpoint, "push failed"),
            }
        }
        if delivered {
            record_nudge(&state, &user_id, now).await?;
        }
    }

    Ok(Json(json!({
        "checked": checked,
        "sent": sent,
        "skipped": skipped,
        "month": now,
    })))
}

fn env_value(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn program_id() -> Result<Pubkey, ApiError> {
    let raw = env_value("PAYVAND_PROGRAM_ID").unwrap_or_else(|| DEVNET_PROGRAM_ID.to_string());
    Pubkey::from_str(&raw)
        .map_err(|_| ApiError::Internal("PAYVAND_PROGRAM_ID is not an address".to_string()))
}

fn rpc_url() -> String {
    if let Some(url) = env_value("SOLANA_RPC_URL") {
        return url;
    }
    match cluster_name().as_str() {
        "mainnet" | "mainnet-beta" => "https://api.mainnet-beta.solana.com".to_string(),
        "testnet" => "https://api.testnet.solana.com".to_string(),
        "localnet" => "http://127.0.0.1:8899".to_string(),
        _ => "https://api.devnet.solana.com".to_string(),
    }
}

/// The cluster the API settles on, read the way every other module reads it.
fn cluster_name() -> String {
    crate::payments::default_cluster()
}

/// Only an https endpoint on a browser push service, with no credentials and a
/// real host: the server sends to whatever this says.
pub fn validated_endpoint(raw: &str) -> ApiResult<String> {
    let trimmed = raw.trim();
    if trimmed.len() > 2_048 {
        return Err(bad("That push subscription is too long."));
    }
    let parsed = url::Url::parse(trimmed)
        .map_err(|_| bad("That push subscription endpoint is not a URL."))?;
    if parsed.scheme() != "https" {
        return Err(bad("Push endpoints must use https."));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(bad("Push endpoints cannot carry credentials."));
    }
    let Some(host) = parsed.host_str() else {
        return Err(bad("Push endpoints must name a host."));
    };
    let host = host.to_ascii_lowercase();
    if !PUSH_HOSTS
        .iter()
        .any(|allowed| host == *allowed || host.ends_with(&format!(".{allowed}")))
    {
        return Err(bad("That host is not a browser push service."));
    }
    Ok(trimmed.to_string())
}

/// Both keys are base64url without padding, bounded so a row cannot hold a
/// payload the sender would happily encrypt against.
pub fn validated_keys(keys: &SubscriptionKeys) -> ApiResult<(String, String)> {
    let valid = |value: &str, max: usize| {
        let trimmed = value.trim();
        !trimmed.is_empty()
            && trimmed.len() <= max
            && trimmed
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    };
    if !valid(&keys.p256dh, 256) || !valid(&keys.auth, 64) {
        return Err(bad("That push subscription has invalid keys."));
    }
    Ok((keys.p256dh.trim().to_string(), keys.auth.trim().to_string()))
}

fn require_cron_secret(headers: &HeaderMap) -> ApiResult<()> {
    let Some(expected) = env_value("CRON_SECRET") else {
        return Err(ApiError::Forbidden(
            "Push reminders are not configured.".to_string(),
        ));
    };
    let presented = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default();
    if !constant_time_eq(presented.as_bytes(), expected.as_bytes()) {
        return Err(ApiError::Unauthorized);
    }
    Ok(())
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

async fn subscriptions_for(
    state: &crate::AppState,
    user_id: Option<&str>,
) -> ApiResult<Vec<StoredSubscription>> {
    let connection = state.db.connect().map_err(auth::internal)?;
    let (sql, params): (&str, Option<&str>) = match user_id {
        Some(id) => (
            "SELECT p.endpoint, p.p256dh, p.auth, p.locale, p.user_id, u.wallet \
             FROM push_subscriptions p JOIN users u ON u.id = p.user_id \
             WHERE p.user_id = ?1",
            Some(id),
        ),
        None => (
            "SELECT p.endpoint, p.p256dh, p.auth, p.locale, p.user_id, u.wallet \
             FROM push_subscriptions p JOIN users u ON u.id = p.user_id \
             WHERE u.wallet IS NOT NULL",
            None,
        ),
    };
    let mut rows = if let Some(id) = params {
        connection.query(sql, [id]).await
    } else {
        connection.query(sql, ()).await
    }
    .map_err(auth::internal)?;

    let mut devices = Vec::new();
    while let Some(row) = rows.next().await.map_err(auth::internal)? {
        devices.push(StoredSubscription {
            endpoint: row.get(0).map_err(auth::internal)?,
            p256dh: row.get(1).map_err(auth::internal)?,
            auth: row.get(2).map_err(auth::internal)?,
            locale: row.get(3).map_err(auth::internal)?,
            user_id: row.get(4).map_err(auth::internal)?,
            wallet: row.get(5).map_err(auth::internal)?,
        });
    }
    Ok(devices)
}

async fn remove_subscription(state: &crate::AppState, endpoint: &str) -> ApiResult<()> {
    let connection = state.db.connect().map_err(auth::internal)?;
    connection
        .execute(
            "DELETE FROM push_subscriptions WHERE endpoint = ?1",
            [endpoint],
        )
        .await
        .map_err(auth::internal)?;
    Ok(())
}

async fn already_nudged(state: &crate::AppState, user_id: &str, month: i64) -> ApiResult<bool> {
    let connection = state.db.connect().map_err(auth::internal)?;
    let mut rows = connection
        .query(
            "SELECT 1 FROM push_nudges WHERE user_id = ?1 AND month_index = ?2",
            (user_id.to_string(), month),
        )
        .await
        .map_err(auth::internal)?;
    Ok(rows.next().await.map_err(auth::internal)?.is_some())
}

async fn record_nudge(state: &crate::AppState, user_id: &str, month: i64) -> ApiResult<()> {
    let connection = state.db.connect().map_err(auth::internal)?;
    connection
        .execute(
            "INSERT INTO push_nudges (user_id, month_index, sent_at) VALUES (?1, ?2, ?3) \
             ON CONFLICT(user_id, month_index) DO NOTHING",
            (user_id.to_string(), month, auth::unix_now()),
        )
        .await
        .map_err(auth::internal)?;
    Ok(())
}

/// Encrypts and sends one payload to one device.
///
/// `web-push` builds the request — the RFC 8188 body and its headers, plus the
/// VAPID signature — and `reqwest` (already this API's client, rustls only)
/// posts it, so no second HTTP stack and no system TLS libraries come along.
async fn deliver(device: &StoredSubscription, body: &str) -> Result<Delivery, String> {
    let private_key =
        env_value("VAPID_PRIVATE_KEY").ok_or_else(|| "VAPID_PRIVATE_KEY is not set".to_string())?;
    let subject =
        env_value("VAPID_SUBJECT").ok_or_else(|| "VAPID_SUBJECT is not set".to_string())?;

    let info = SubscriptionInfo::new(&device.endpoint, &device.p256dh, &device.auth);
    let mut signature = VapidSignatureBuilder::from_base64(&private_key, &info)
        .map_err(|error| format!("vapid key: {error}"))?;
    signature.add_claim("sub", subject);
    let signature = signature
        .build()
        .map_err(|error| format!("vapid signature: {error}"))?;

    let mut builder = WebPushMessageBuilder::new(&info);
    builder.set_payload(ContentEncoding::Aes128Gcm, body.as_bytes());
    builder.set_vapid_signature(signature);
    builder.set_ttl(86_400);
    let message = builder
        .build()
        .map_err(|error| format!("push message: {error}"))?;

    let request = request_builder::build_request::<Vec<u8>>(message);
    let mut outgoing = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|error| format!("http client: {error}"))?
        .post(request.uri().to_string());
    for (name, value) in request.headers() {
        outgoing = outgoing.header(name.as_str(), value.as_bytes());
    }
    let response = outgoing
        .body(request.into_body())
        .send()
        .await
        .map_err(|error| format!("push send: {error}"))?;

    match response.status().as_u16() {
        200..=299 => Ok(Delivery::Sent),
        // The browser threw the subscription away: keep sending and the push
        // service keeps answering the same thing.
        404 | 410 => Ok(Delivery::Expired),
        status => Err(format!("push service answered {status}")),
    }
}

fn payload(title: &str, body: &str, url: &str) -> String {
    json!({ "title": title, "body": body, "url": url }).to_string()
}

fn nudge_copy(locale: &str) -> (&'static str, &'static str) {
    match locale {
        "de" => (
            "Dein Budget ist zurückgesetzt",
            "Ein neuer Monat: Das Limit gilt wieder voll, im Topf liegt aber nur, was übrig war. Füll auf, um wieder daraus zu zahlen.",
        ),
        "es" => (
            "Tu presupuesto se reinició",
            "Empieza un mes nuevo: el límite vuelve a estar completo, pero la reserva solo tiene lo que quedó. Rellénala para volver a pagar con ella.",
        ),
        "fa" => (
            "سقف ماهانه از نو شد",
            "ماه نو شروع شد: سقف دوباره کامل است، اما در استخر فقط همان چیزی مانده که باقی بود. برای خرج کردن دوباره شارژش کنید.",
        ),
        "fr" => (
            "Votre autorisation est repartie à zéro",
            "Un nouveau mois commence : la limite est de nouveau pleine, mais la cagnotte ne contient que ce qui restait. Rechargez-la pour payer à nouveau.",
        ),
        _ => (
            "Your allowance reset",
            "A new month started: the limit is whole again, but the pot kept only what was left. Top up to spend from it again.",
        ),
    }
}

fn test_copy(locale: &str) -> (&'static str, &'static str) {
    match locale {
        "de" => (
            "Benachrichtigungen sind an",
            "Payvand kann dich jetzt mit einer kurzen Erinnerung erreichen. Abschalten kannst du das jederzeit in den Einstellungen.",
        ),
        "es" => (
            "Las notificaciones están activadas",
            "Payvand ya puede avisarte con un recordatorio breve. Puedes desactivarlo cuando quieras en Ajustes.",
        ),
        "fa" => (
            "اعلان‌ها روشن شد",
            "پیوند حالا می‌تواند با یک یادآوری کوتاه به شما خبر بدهد. هر وقت خواستید از صفحهٔ تنظیمات خاموشش کنید.",
        ),
        "fr" => (
            "Les notifications sont activées",
            "Payvand peut désormais vous prévenir par un court rappel. Vous pouvez le désactiver à tout moment dans les réglages.",
        ),
        _ => (
            "Notifications are on",
            "Payvand can now reach you with a short reminder. You can turn this off any time in Settings.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_browser_push_services() {
        assert_eq!(
            validated_endpoint("https://fcm.googleapis.com/fcm/send/abc").expect("fcm"),
            "https://fcm.googleapis.com/fcm/send/abc"
        );
        assert!(
            validated_endpoint("https://updates.push.services.mozilla.com/wpush/v2/gAAAA").is_ok()
        );
        assert!(validated_endpoint("https://xn--fcm.googleapis.com.evil.example/x").is_err());
        assert!(validated_endpoint("http://fcm.googleapis.com/fcm/send/abc").is_err());
        assert!(validated_endpoint("https://127.0.0.1/push").is_err());
        assert!(validated_endpoint("https://fcm.googleapis.com@evil.example/x").is_err());
    }

    #[test]
    fn rejects_malformed_keys() {
        let good = SubscriptionKeys {
            p256dh: "BLMbF9ffKBiWQLCKvTHb6LO8Nb6dcUh6TItC455vu2kElga6PQvUmaFyCdykxY2nOSSL3yKgfbmFLRTUaGv4yV8".to_string(),
            auth: "xS03Fi5ErfTNH_l9WHE9Ig".to_string(),
        };
        assert!(validated_keys(&good).is_ok());

        for bad_keys in [
            SubscriptionKeys {
                p256dh: String::new(),
                auth: good.auth.clone(),
            },
            SubscriptionKeys {
                p256dh: good.p256dh.clone(),
                auth: "not base64!".to_string(),
            },
            SubscriptionKeys {
                p256dh: "a".repeat(300),
                auth: good.auth.clone(),
            },
        ] {
            assert!(validated_keys(&bad_keys).is_err());
        }
    }
}
