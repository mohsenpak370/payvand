//! Session keys: which keys may spend from a wallet's budgets.

use solana_pubkey::Pubkey;
use std::str::FromStr;

use super::{
    config::{mint_for, payment_rpc, program_id},
    model::BudgetAccount,
    onchain::classify::{classify_program_account, ScannedAccount},
};

/// How long a wallet's allowances are trusted to be what they were when this
/// instance last looked.
///
/// The lookup is one bounded `getMultipleAccounts` on the addresses a buyer's
/// budgets live at, so it is cheap — but it is still a cluster read, and a feed
/// load is not the place to spend one every time. An hour is short enough that
/// an allowance opened mid-session is found on its own, and the refresh button
/// asks for a scan immediately regardless.
const ALLOWANCE_CHECK_SECS: i64 = 60 * 60;

/// The session keys the allowances of this wallet pay with, newest first.
///
/// Rows written when a budget account was read — by a scan, or by the one
/// lookup `detect_allowance` does. Empty is the normal answer for a wallet with
/// no allowance.
pub async fn session_keys_for(
    db: &libsql::Database,
    wallet: &str,
    cluster: &str,
    endpoint: &str,
) -> Vec<Pubkey> {
    let read = async {
        let connection = db.connect()?;
        connection
            .query(
                "SELECT session_key FROM session_keys \
                 WHERE wallet = ?1 AND cluster = ?2 AND endpoint = ?3 \
                 ORDER BY first_seen DESC LIMIT 8",
                libsql::params![
                    wallet.to_string(),
                    cluster.to_string(),
                    endpoint.to_string()
                ],
            )
            .await
    }
    .await;

    let Ok(mut rows) = read else {
        return Vec::new();
    };
    let mut keys = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        let Ok(key) = row.get::<String>(0) else {
            continue;
        };
        if let Ok(key) = Pubkey::from_str(&key) {
            keys.push(key);
        }
    }
    keys
}

/// Remembers that one session key spends for one wallet. `INSERT OR IGNORE`:
/// a budget is created once, so the pair cannot change, and a repeated look at
/// the same account costs nothing.
pub async fn remember_session_key(
    db: &libsql::Database,
    session_key: &str,
    wallet: &str,
    cluster: &str,
    endpoint: &str,
) {
    let now = crate::auth::unix_now();
    let written = async {
        let connection = db.connect()?;
        connection
            .execute(
                "INSERT OR IGNORE INTO session_keys \
                     (session_key, wallet, endpoint, cluster, first_seen) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                libsql::params![
                    session_key.to_string(),
                    wallet.to_string(),
                    endpoint.to_string(),
                    cluster.to_string(),
                    now
                ],
            )
            .await?;
        Ok::<(), crate::db::DbError>(())
    }
    .await;
    if let Err(error) = written {
        tracing::info!(%error, "a session key was not remembered");
    }
}

/// Remembers every budget a scan read, for whichever buyer it belongs to: the
/// next feed load then matches that buyer's silent payments without scanning.
pub async fn remember_budgets(
    db: &libsql::Database,
    budgets: &[BudgetAccount],
    cluster: &str,
    endpoint: &str,
) {
    for budget in budgets {
        if budget.session_key == budget.buyer {
            continue;
        }
        remember_session_key(
            db,
            &budget.session_key.to_string(),
            &budget.buyer.to_string(),
            cluster,
            endpoint,
        )
        .await;
    }
}

/// Whether this wallet's allowances are worth looking up again.
///
/// True when it has never been looked up, or when the last look is older than
/// [`ALLOWANCE_CHECK_SECS`]. A database that cannot answer says no: a feed load
/// is the wrong place to invent a cluster read.
pub async fn allowance_check_due(
    db: &libsql::Database,
    wallet: &str,
    cluster: &str,
    endpoint: &str,
) -> bool {
    let read = async {
        let connection = db.connect()?;
        let mut rows = connection
            .query(
                "SELECT checked_at FROM allowance_checks \
                 WHERE wallet = ?1 AND cluster = ?2 AND endpoint = ?3",
                libsql::params![
                    wallet.to_string(),
                    cluster.to_string(),
                    endpoint.to_string()
                ],
            )
            .await?;
        Ok::<Option<i64>, crate::db::DbError>(rows.next().await?.and_then(|row| row.get(0).ok()))
    }
    .await;

    match read {
        Ok(None) => true,
        Ok(Some(checked_at)) => crate::auth::unix_now() - checked_at >= ALLOWANCE_CHECK_SECS,
        Err(error) => {
            tracing::info!(%error, "allowance checks were not readable");
            false
        }
    }
}

/// Records that this wallet's allowances were just looked up, so the next load
/// is a row read whatever the answer was.
pub async fn mark_allowance_checked(
    db: &libsql::Database,
    wallet: &str,
    cluster: &str,
    endpoint: &str,
) {
    let now = crate::auth::unix_now();
    let written = async {
        let connection = db.connect()?;
        connection
            .execute(
                "INSERT INTO allowance_checks (wallet, endpoint, cluster, checked_at) \
                 VALUES (?1, ?2, ?3, ?4) \
                 ON CONFLICT(wallet, endpoint, cluster) \
                 DO UPDATE SET checked_at = excluded.checked_at",
                libsql::params![
                    wallet.to_string(),
                    endpoint.to_string(),
                    cluster.to_string(),
                    now
                ],
            )
            .await?;
        Ok::<(), crate::db::DbError>(())
    }
    .await;
    if let Err(error) = written {
        tracing::info!(%error, "an allowance check was not recorded");
    }
}

/// The session keys this buyer's allowances pay with, read off the cluster.
///
/// A budget's address is derived from the buyer and, for a token budget, the
/// mint, so the few an account can have are known without a scan: one
/// `getMultipleAccounts` on those addresses. `None` means the cluster did not
/// answer — the caller then leaves the check unrecorded and tries again later;
/// `Some(empty)` means this wallet has no allowance, which is worth knowing.
pub async fn detect_allowance(
    db: &libsql::Database,
    endpoint: &str,
    cluster: &str,
    buyer: &Pubkey,
) -> Option<Vec<Pubkey>> {
    let program = program_id().ok()?;
    let mut addresses = vec![crate::push_budget::budget_address(buyer, &program)];
    for currency in ["usdc", "eurc"] {
        if let Some(mint) = mint_for(currency, cluster) {
            addresses.push(crate::push_budget::token_budget_address(
                buyer, &mint, &program,
            ));
        }
    }

    let rpc = payment_rpc(endpoint.to_owned());
    let accounts = match rpc.get_multiple_accounts(&addresses).await {
        Ok(accounts) => accounts,
        Err(error) => {
            tracing::info!(%endpoint, %error, "allowance accounts not readable");
            return None;
        }
    };

    let mut session_keys = Vec::new();
    for (address, account) in addresses.iter().zip(accounts) {
        let Some(account) = account else {
            continue;
        };
        let Some(ScannedAccount::Budget(budget)) = classify_program_account(
            *address,
            &account.data,
            cluster.to_owned(),
            endpoint.to_owned(),
        ) else {
            continue;
        };
        // The address was derived from this buyer, so a different one in the
        // account is not this wallet's allowance.
        if budget.buyer != *buyer {
            continue;
        }
        session_keys.push(budget.session_key);
    }

    let budgets: Vec<BudgetAccount> = session_keys
        .iter()
        .map(|session_key| BudgetAccount {
            buyer: *buyer,
            session_key: *session_key,
        })
        .collect();
    remember_budgets(db, &budgets, cluster, endpoint).await;
    Some(session_keys)
}

#[cfg(test)]
mod tests;
