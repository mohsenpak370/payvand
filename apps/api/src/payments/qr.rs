//! Compact QR payload written into every payment code.

use serde::Serialize;

use super::model::{PaymentDiscount, PaymentItem, PaymentOption, PaymentRecord};

/// Version tag carried in the QR payload so scanners can evolve the format.
pub const QR_PAYLOAD_VERSION: u8 = 1;

impl PaymentRecord {
    /// Compact, self-describing blob the seller encodes into the QR code.
    pub fn qr_payload(&self) -> String {
        // An invoice that takes SOL and nothing else keeps the old shape: the
        // code scans exactly as it did before a seller could offer currencies.
        let many = self.options.len() > 1 || self.options.iter().any(|option| !option.is_sol());
        let primary = self
            .options
            .iter()
            .find(|option| option.is_sol())
            .or_else(|| self.options.first())
            .map(|option| option.payment_pda.as_str())
            .unwrap_or(self.id.as_str());
        serde_json::to_string(&QrPayload {
            v: QR_PAYLOAD_VERSION,
            id: primary,
            seller: &self.seller,
            // Lets the buyer's screen ask the API for this seller's codes.
            seller_user: self.seller_user.as_deref(),
            reference: &self.reference,
            amount_lamports: self.amount_lamports,
            memo: &self.memo,
            name: &self.name,
            description: &self.description,
            items: &self.items,
            discount: self.discount.as_ref(),
            allow_discounts: self.allow_discounts,
            options: if many { &self.options } else { &[] },
            cluster: &self.cluster,
            endpoint: &self.endpoint,
        })
        .expect("qr payload is serializable")
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Serialize)]
struct QrPayload<'a> {
    v: u8,
    id: &'a str,
    seller: &'a str,
    seller_user: Option<&'a str>,
    reference: &'a str,
    amount_lamports: u64,
    memo: &'a str,
    // Skipped when empty so a payment without them keeps the code as sparse
    // (and as easy to scan) as it was before these fields existed.
    #[serde(skip_serializing_if = "str::is_empty")]
    name: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    description: &'a str,
    /// The receipt, skipped on an invoice that has none so ordinary codes keep
    /// exactly the shape they had before shops existed.
    #[serde(skip_serializing_if = "<[PaymentItem]>::is_empty")]
    items: &'a [PaymentItem],
    /// Skipped when absent, so an ordinary invoice scans exactly as before.
    #[serde(skip_serializing_if = "Option::is_none")]
    discount: Option<&'a PaymentDiscount>,
    /// Skipped when false: only an invoice that invites a code says so.
    #[serde(skip_serializing_if = "is_false")]
    allow_discounts: bool,
    /// The currencies this invoice accepts. Skipped for a SOL-only invoice,
    /// whose `id` and `amount_lamports` already say everything.
    #[serde(skip_serializing_if = "<[PaymentOption]>::is_empty")]
    options: &'a [PaymentOption],
    cluster: &'a str,
    endpoint: &'a str,
}

#[cfg(test)]
mod tests;
