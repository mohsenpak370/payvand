//! Admin API — platform oversight for the people running Payvand.
//!
//! Authorization is configuration, not schema: `ADMIN_EMAILS` (comma
//! separated) is matched against the signed-in account's address, so
//! granting access never needs a migration or a bootstrap row. Every
//! handler re-checks the session first, then the address — a stolen
//! response shape is useless without a live admin session.
//!
//! Endpoint groups live in submodules; the public paths (`admin::stats`,
//! `admin::list_shops`, …) stay the same through the re-exports below.

mod airdrops;
mod chain;
mod feed;
mod records;
mod users;
mod waitlist;

#[cfg(test)]
mod tests;

pub use airdrops::{airdrops, Airdrop, AirdropPage, AirdropQuery};
pub use chain::{chain, ChainAccount, ChainOverview, ProgramInfo, ServerWallet, TokenBalances};
pub use feed::{list_payments, AdminPayment, PaymentListQuery, PaymentsPage};
pub use records::*;
pub use users::{delete_user, list_users, stats, AdminUser, Stats, UserListQuery, UserPage};
pub use waitlist::{remove_waitlist, waitlist, WaitlistEntry, WaitlistPage};

use axum::http::HeaderMap;

use crate::auth::{self, User};
use crate::db::SharedDb;
use crate::payments::{ApiError, ApiResult};

/// True when the address is listed in `ADMIN_EMAILS`.
/// Comparison happens on the normalized (trimmed, lowercased) form.
pub fn is_admin(email: &str) -> bool {
    std::env::var("ADMIN_EMAILS")
        .map(|raw| {
            raw.split(',')
                .map(|entry| entry.trim().to_lowercase())
                .any(|entry| !entry.is_empty() && entry == email)
        })
        .unwrap_or(false)
}

/// The signed-in account, when it has admin access.
pub async fn require_admin(db: &SharedDb, headers: &HeaderMap) -> ApiResult<User> {
    let user = auth::require_user(db, headers).await?;
    if is_admin(&user.email) {
        Ok(user)
    } else {
        Err(ApiError::Forbidden(
            "This account does not have admin access.".into(),
        ))
    }
}

async fn scalar(connection: &libsql::Connection, sql: &str) -> ApiResult<i64> {
    let mut rows = connection.query(sql, ()).await.map_err(auth::internal)?;
    first_count(&mut rows).await
}

async fn first_count(rows: &mut libsql::Rows) -> ApiResult<i64> {
    match rows.next().await.map_err(auth::internal)? {
        Some(row) => Ok(row.get(0).map_err(auth::internal)?),
        None => Ok(0),
    }
}
