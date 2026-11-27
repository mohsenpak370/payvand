//! Sorts one program-owned account into the kinds the API keeps.

use solana_pubkey::Pubkey;

use super::{
    super::model::{BudgetAccount, PaymentRecord},
    accounts::{decode_budget, decode_receipt_marker, decode_token_budget, ReceiptMarker},
    records::record_for,
};

/// What one program-owned account turned out to be. The payment variant is
/// boxed because `PaymentRecord` carries every option, item and discount of an
/// invoice, and the scan builds one enum value per account.
pub(crate) enum ScannedAccount {
    Payment(Box<PaymentRecord>),
    /// A settlement's marker, unturned into a record. Resolving it is the
    /// caller's job, not this function's: it needs the cluster and the request's
    /// history budget, and this is a pure decoder.
    Settled(ReceiptMarker),
    Budget(BudgetAccount),
}

/// Sorts one account from `get_program_accounts` into what the scan keeps.
///
/// `None` is the normal answer for accounts this API has no use for — the fee
/// `Config` lives under the same program id — so an unknown or unreadable
/// account is skipped rather than failing the whole scan. A future account type
/// gets the same treatment for free.
pub(crate) fn classify_program_account(
    pubkey: Pubkey,
    data: &[u8],
    cluster: String,
    endpoint: String,
) -> Option<ScannedAccount> {
    if let Some(record) = record_for(pubkey, data, cluster, endpoint) {
        return Some(ScannedAccount::Payment(Box::new(record)));
    }
    if let Some(marker) = decode_receipt_marker(data) {
        // A settled invoice only exists as its marker, and the marker names the
        // invoice the scan addresses it by. Its payment facts are in the settle
        // transaction, which the scan reads within its history budget.
        return Some(ScannedAccount::Settled(marker));
    }
    decode_budget(data)
        .or_else(|| decode_token_budget(data))
        .map(ScannedAccount::Budget)
}

#[cfg(test)]
mod tests;
