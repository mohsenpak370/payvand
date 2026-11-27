//! Payment request validation, QR payload formatting and on-chain status reads.
//!
//! This file is the public surface. Implementation and unit tests live in the
//! `payments/` directory, one pair of files per responsibility.

mod allowances;
mod cache;
mod config;
mod create;
mod db;
mod error;
mod items;
mod model;
mod onchain;
mod options;
mod pda;
mod qr;
mod rpc;
mod scan;
mod settlement;
mod status;
mod watch;

#[cfg(test)]
mod test_support;

pub use allowances::{
    allowance_check_due, detect_allowance, mark_allowance_checked, remember_budgets,
    remember_session_key, session_keys_for,
};
pub use config::{currency_for_mint, mint_for, PAYVAND_PROGRAM_ID};
pub use create::{create_payment, CreateOption, CreatePaymentRequest};
pub use db::backfill::backfill_invoice_rows;
pub use db::invoices::{
    mark_reusable_paid, recorded_invoices, remember_invoice, remember_scanned,
    reusable_invoices_awaiting, AWAITING_REUSABLE_LIMIT,
};
pub use db::settlements::{settlements_paid_by, settlements_paid_by_any};
pub use error::{ApiError, ApiResult};
pub use model::{
    BudgetAccount, PaymentDiscount, PaymentItem, PaymentOption, PaymentRecord, PaymentStatus,
    MAX_DESCRIPTION_LEN, MAX_ITEMS, MAX_ITEM_NAME_LEN, MAX_ITEM_QUANTITY, MAX_MEMO_LEN,
    MAX_NAME_LEN, MAX_WATCH_PDAS, SOL_CURRENCY, SOL_DECIMALS, STABLE_DECIMALS,
};
pub use options::{build_options, MAX_OPTIONS};
pub use pda::{receipt_pda, PAYMENT_RECEIPT_SEED, PAYMENT_SEED, TOKEN_PAYMENT_SEED};
pub use qr::QR_PAYLOAD_VERSION;
pub use scan::{fetch_all_payments, fetch_program_scan, AllPayments, ProgramScan};
pub use settlement::{HistoryBudget, MAX_HISTORY_READS};
pub use status::{fetch_payment, PaymentQuery};
pub use watch::{fetch_many, WatchRequest};

pub(crate) use config::{default_cluster, payment_rpc, resolve_endpoint};
pub(crate) use error::{bad, parse_pubkey};
