//! The invoice a QR code carries: decoding it, proving it was not edited, and
//! pricing what the buyer actually pays.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use std::str::FromStr;

use crate::discounts::{discounted_lamports, resolve_for_seller};
use crate::payments::{
    bad, parse_pubkey, ApiError, ApiResult, PaymentDiscount, MAX_MEMO_LEN, PAYVAND_PROGRAM_ID,
    QR_PAYLOAD_VERSION,
};

/// Seed prefix for every payment PDA, mirroring the Anchor program.
const PAYMENT_SEED: &[u8] = b"payment";

/// The invoice a QR code hands out. Unknown fields in the token are ignored,
/// so the web app can add detail without breaking an older API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoice {
    pub v: u8,
    pub id: String,
    pub seller: String,
    pub reference: String,
    pub amount_lamports: u64,
    #[serde(default)]
    pub memo: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// The seller's account, when the code carries one. A hint for the buyer's
    /// screen — "this seller offers codes", nothing more. It prices nothing:
    /// the account is looked up from `seller`, which `verify()` has proven.
    #[serde(default)]
    pub seller_user: Option<String>,
    /// A code the invoice carries. Its percentage is never taken from the
    /// token: a buyer can edit a QR, so the code is re-priced server-side.
    #[serde(default)]
    pub discount: Option<PaymentDiscount>,
    pub cluster: String,
}

/// Decodes the base64url token from the QR path back into an invoice.
pub fn decode_invoice(token: &str) -> ApiResult<Invoice> {
    let bytes = URL_SAFE_NO_PAD
        .decode(token.trim_end_matches('='))
        .map_err(|_| bad("This is not a Payvand payment code."))?;
    let text = String::from_utf8(bytes).map_err(|_| bad("This is not a Payvand payment code."))?;
    let invoice: Invoice =
        serde_json::from_str(&text).map_err(|_| bad("This is not a Payvand payment code."))?;

    if invoice.v != QR_PAYLOAD_VERSION {
        return Err(bad("This payment code version is not supported."));
    }
    if invoice.amount_lamports == 0 {
        // A token invoice has no lamport price: the wallet transaction this
        // route builds is a SOL transfer, so the buyer pays it on the page.
        return Err(bad(
            "This invoice is priced in a token. Open the payment page to pay it.",
        ));
    }
    if invoice.memo.len() > MAX_MEMO_LEN {
        return Err(bad("This payment code has an overlong memo."));
    }
    // Proves the address and the full price were not edited apart.
    invoice.verify()?;

    Ok(invoice)
}

impl Invoice {
    pub(super) fn seller(&self) -> ApiResult<Pubkey> {
        parse_pubkey(&self.seller, "seller")
    }

    pub(super) fn reference(&self) -> ApiResult<Pubkey> {
        parse_pubkey(&self.reference, "reference")
    }

    /// The invoice address for a given price. The amount is part of the PDA
    /// seeds, so each price — full or discounted — has its own address.
    pub(super) fn address_for(&self, amount: u64) -> ApiResult<Pubkey> {
        let seller = self.seller()?;
        let reference = self.reference()?;
        let (address, _) = Pubkey::find_program_address(
            &[
                PAYMENT_SEED,
                seller.as_ref(),
                reference.as_ref(),
                &amount.to_le_bytes(),
            ],
            &program_id()?,
        );
        Ok(address)
    }

    /// Proves the address, seller, reference and full price were not edited
    /// apart before anything is built from them.
    fn verify(&self) -> ApiResult<()> {
        let derived = self.address_for(self.amount_lamports)?;
        let declared = parse_pubkey(&self.id, "id")?;
        if derived != declared {
            return Err(bad(
                "This payment code does not match its details. Ask the seller for a new one.",
            ));
        }
        Ok(())
    }

    /// What the buyer actually pays. A discount code is re-priced against the
    /// seller's own list rather than trusting the percentage in the code: a
    /// QR is data a buyer can edit, and an invented discount must never turn
    /// into a cheaper transaction.
    ///
    /// The seller's account is found from the **wallet address** the invoice
    /// was derived from, not from `seller_user` in the token. That field is
    /// writable by anyone who edits the code, so trusting it here would let a
    /// buyer attach a 99% code of their own to someone else's invoice.
    pub(super) async fn payable_lamports(&self, db: &libsql::Database) -> ApiResult<u64> {
        let Some(discount) = &self.discount else {
            return Ok(self.amount_lamports);
        };
        let seller_user = crate::auth::seller_account(db, &self.seller)
            .await?
            .ok_or_else(|| {
                bad("This payment's discount cannot be checked. Open the payment page to pay.")
            })?;

        let resolved = resolve_for_seller(db, &seller_user, &discount.code)
            .await
            .map_err(|error| match error {
                ApiError::BadRequest(_) => {
                    bad("This discount code is no longer valid. Open the payment page to pay.")
                }
                other => other,
            })?;
        Ok(discounted_lamports(
            self.amount_lamports,
            resolved.percent_off,
        ))
    }

    pub(super) fn label(&self) -> String {
        let name = self.name.trim();
        if !name.is_empty() {
            return name.to_string();
        }
        let memo = self.memo.trim();
        if !memo.is_empty() {
            return memo.to_string();
        }
        "Payvand payment".to_string()
    }

    pub(super) fn description(&self, payable: u64) -> String {
        let description = self.description.trim();
        if !description.is_empty() {
            return description.to_string();
        }
        format!(
            "{} SOL — scan with a Solana wallet to pay",
            format_lamports(payable)
        )
    }
}

pub(super) fn program_id() -> ApiResult<Pubkey> {
    Pubkey::from_str(PAYVAND_PROGRAM_ID)
        .map_err(|error| ApiError::Internal(format!("bad program id: {error}")))
}

/// `0.01` rather than `0.010000000`: the number the seller typed.
pub(super) fn format_lamports(lamports: u64) -> String {
    let whole = lamports / 1_000_000_000;
    let fraction = lamports % 1_000_000_000;
    if fraction == 0 {
        return whole.to_string();
    }
    let digits = format!("{fraction:09}");
    format!("{whole}.{}", digits.trim_end_matches('0'))
}

#[cfg(test)]
mod tests;
