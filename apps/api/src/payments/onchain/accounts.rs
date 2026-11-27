//! Borsh layouts of the program's accounts and settlement event.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use solana_pubkey::Pubkey;

use super::{
    super::model::BudgetAccount,
    reader::{account_discriminator, discriminator_for, event_discriminator_for, Reader},
};

pub(crate) struct OnchainPayment {
    pub(crate) seller: Pubkey,
    pub(crate) reference: Pubkey,
    pub(crate) amount: u64,
    pub(crate) memo: String,
    pub(crate) state: u8,
    pub(crate) payer: Pubkey,
    pub(crate) created_at: i64,
    pub(crate) settled_at: i64,
}

/// Borsh layout of the `Payment` account: 8 byte discriminator, then the
/// struct fields in declaration order.
pub(crate) fn decode_payment(data: &[u8]) -> Option<OnchainPayment> {
    let mut reader = Reader::new(data);

    if reader.take(8)? != account_discriminator() {
        return None;
    }

    Some(OnchainPayment {
        seller: reader.pubkey()?,
        reference: reader.pubkey()?,
        amount: reader.u64()?,
        memo: reader.string()?,
        state: reader.u8()?,
        payer: reader.pubkey()?,
        created_at: reader.i64()?,
        settled_at: reader.i64()?,
    })
}

pub(crate) struct OnchainTokenPayment {
    pub(crate) seller: Pubkey,
    pub(crate) reference: Pubkey,
    pub(crate) mint: Pubkey,
    pub(crate) amount: u64,
    pub(crate) memo: String,
    pub(crate) state: u8,
    pub(crate) payer: Pubkey,
    pub(crate) created_at: i64,
    pub(crate) settled_at: i64,
}

/// Borsh layout of the `TokenPayment` account: discriminator, then the fields
/// in declaration order — the mint sits between the reference and the amount.
pub(crate) fn decode_token_payment(data: &[u8]) -> Option<OnchainTokenPayment> {
    let mut reader = Reader::new(data);

    if reader.take(8)? != discriminator_for("TokenPayment") {
        return None;
    }

    Some(OnchainTokenPayment {
        seller: reader.pubkey()?,
        reference: reader.pubkey()?,
        mint: reader.pubkey()?,
        amount: reader.u64()?,
        memo: reader.string()?,
        state: reader.u8()?,
        payer: reader.pubkey()?,
        created_at: reader.i64()?,
        settled_at: reader.i64()?,
    })
}

/// The permanent `PaymentReceipt` account as it now exists: the invoice it
/// closed and the `settled_at` replay guard, nothing more. The payment's own
/// fields are in the `PaymentSettled` event the settle transaction logged, and
/// `settled_from_history` reads them from there.
#[derive(Clone, Copy)]
pub(crate) struct ReceiptMarker {
    pub(crate) invoice: Pubkey,
    pub(crate) settled_at: i64,
}

/// One settlement, assembled from its event. The field names are the
/// `PaymentReceipt` account's old ones on purpose: every consumer of a settled
/// invoice below already speaks this shape, and where the facts live is the
/// only thing that changed.
#[derive(Clone, Debug)]
pub(crate) struct OnchainReceipt {
    pub(crate) invoice: Pubkey,
    pub(crate) seller: Pubkey,
    pub(crate) reference: Pubkey,
    /// `Pubkey::default()` for a SOL invoice.
    pub(crate) mint: Pubkey,
    pub(crate) amount: u64,
    pub(crate) memo: String,
    pub(crate) payer: Pubkey,
    pub(crate) created_at: i64,
    pub(crate) settled_at: i64,
}

/// The marker's exact size: discriminator, invoice, `settled_at` — 48 bytes,
/// mirroring `PAYMENT_RECEIPT_SPACE` in the program.
const RECEIPT_MARKER_LEN: usize = 8 + 32 + 8;

/// Borsh layout of the `PaymentReceipt` account since revision 23:
/// discriminator, the invoice, and `settled_at`. It is a marker, not a record.
///
/// The length is checked rather than trusted. The pre-marker receipt carried the
/// same discriminator with the invoice at the same offset but a different store
/// of fields after it, so a 301-byte account read as a marker would report its
/// seller's first bytes as `settled_at`.
pub(crate) fn decode_receipt_marker(data: &[u8]) -> Option<ReceiptMarker> {
    if data.len() != RECEIPT_MARKER_LEN {
        return None;
    }
    let mut reader = Reader::new(data);

    if reader.take(8)? != discriminator_for("PaymentReceipt") {
        return None;
    }

    Some(ReceiptMarker {
        invoice: reader.pubkey()?,
        settled_at: reader.i64()?,
    })
}

/// Borsh body of the `PaymentSettled` event, as the program emits it:
/// `amount, fee, seller_proceeds, payer, seller, mint, reference, invoice, memo`.
pub(crate) fn decode_payment_settled(data: &[u8]) -> Option<OnchainReceipt> {
    let mut reader = Reader::new(data);

    if reader.take(8)? != event_discriminator_for("PaymentSettled") {
        return None;
    }

    let amount = reader.u64()?;
    reader.u64()?; // fee: the treasury's share, not part of a status record
    reader.u64()?; // seller_proceeds: what the seller keeps after the fee
    let payer = reader.pubkey()?;
    let seller = reader.pubkey()?;
    let mint = reader.pubkey()?;
    let reference = reader.pubkey()?;
    let invoice = reader.pubkey()?;
    let memo = reader.string()?;

    Some(OnchainReceipt {
        invoice,
        seller,
        reference,
        mint,
        amount,
        memo,
        payer,
        // Filled from the transaction's own block time by the caller.
        created_at: 0,
        settled_at: 0,
    })
}

/// One log line, if it is this program's `PaymentSettled` event. An event is a
/// single `Program data: <base64>` line; another program's logs and the
/// program's own `msg!` lines sit beside it.
pub(crate) fn settled_event_in_log(line: &str) -> Option<OnchainReceipt> {
    let body = line.strip_prefix("Program data: ")?;
    let bytes = STANDARD.decode(body).ok()?;
    decode_payment_settled(&bytes)
}

/// Borsh layout of the `Budget` account: discriminator, buyer, session key,
/// then the allowance fields the chain enforces. Only the identities matter
/// to the API — the amounts are read from the chain by the client.
pub(crate) fn decode_budget(data: &[u8]) -> Option<BudgetAccount> {
    let mut reader = Reader::new(data);
    if reader.take(8)? != discriminator_for("Budget") {
        return None;
    }
    Some(BudgetAccount {
        buyer: reader.pubkey()?,
        session_key: reader.pubkey()?,
    })
}

/// A token allowance is a budget too: it holds the same buyer and session key at
/// the same offsets, with the mint after them. A payment made silently from a
/// USDC or EURC pot is signed by that session key, so a history that read only
/// the SOL pots could not show it at all.
pub(crate) fn decode_token_budget(data: &[u8]) -> Option<BudgetAccount> {
    let mut reader = Reader::new(data);
    if reader.take(8)? != discriminator_for("TokenBudget") {
        return None;
    }
    Some(BudgetAccount {
        buyer: reader.pubkey()?,
        session_key: reader.pubkey()?,
    })
}

#[cfg(test)]
mod tests;
