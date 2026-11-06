//! Solana Pay transaction requests.
//!
//! The QR code is a `solana:` URL, so a wallet recognizes it and offers to pay.
//! The URL points back here with the invoice base64url-encoded in its path:
//! the wallet POSTs its account and gets an unsigned `settle_payment`
//! transaction to sign. The invoice itself is never stored — it rides in the
//! token and is verified against its own PDA — so a scan works on any isolate
//! and after any restart.
//!
//! Payvand's own scanner reads the same string: the web app decodes the token
//! back into the invoice instead of asking a wallet to sign anything.

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap};
use axum::response::IntoResponse;
use axum::Json;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use solana_pubkey::Pubkey;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use std::str::FromStr;

use crate::discounts::{discounted_lamports, resolve_for_seller};
use crate::payments::{
    bad, parse_pubkey, resolve_endpoint, ApiError, ApiResult, PaymentDiscount, MAX_MEMO_LEN,
    PAYVAND_PROGRAM_ID, QR_PAYLOAD_VERSION,
};
use crate::AppState;

/// Seed prefix for every payment PDA, mirroring the Anchor program.
const PAYMENT_SEED: &[u8] = b"payment";
/// The System program, whose transfer moves the lamports to the seller.
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";

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
    /// The seller's account, needed to price an attached discount code.
    #[serde(default)]
    pub seller_user: Option<String>,
    /// A code the invoice carries. Its percentage is never taken from the
    /// token: a buyer can edit a QR, so the code is re-priced server-side.
    #[serde(default)]
    pub discount: Option<PaymentDiscount>,
    pub cluster: String,
}

/// The wallet's account, as the Solana Pay spec sends it in the POST body.
#[derive(Debug, Deserialize)]
pub struct AccountRequest {
    pub account: String,
}

/// Some wallets put the account in the query string instead of the body.
#[derive(Debug, Default, Deserialize)]
pub struct AccountQuery {
    pub account: Option<String>,
}

/// `GET` response: what the wallet shows before asking for the transaction.
/// The spec requires `icon`; `message` is the optional human note.
#[derive(Debug, Serialize)]
pub struct RequestInfo {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub message: String,
}

/// `POST` response: the transaction the wallet signs and sends.
#[derive(Debug, Serialize)]
pub struct TransactionRequest {
    pub transaction: String,
    pub message: String,
}

/// `GET /api/solana-pay/{token}` — wallet metadata for the payment.
pub async fn info(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(token): Path<String>,
) -> ApiResult<Json<RequestInfo>> {
    let invoice = decode_invoice(&token)?;
    let payable = invoice.payable_lamports(&state.db).await?;
    Ok(Json(RequestInfo {
        label: invoice.label(),
        icon: icon_url(&headers),
        message: invoice.description(payable),
    }))
}

/// The icon the wallet shows next to the label. The API serves it itself, so
/// the URL is absolute and points at a file the wallet can fetch.
pub async fn icon() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "image/svg+xml"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        ICON_SVG,
    )
}

/// A mark, not the full web logo: wallets render it at a few dozen pixels.
const ICON_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect width="64" height="64" rx="14" fill="#0b1220"/><path d="M23 47V17h11a10.5 10.5 0 0 1 0 21h-5" fill="none" stroke="#fff" stroke-width="6" stroke-linecap="round" stroke-linejoin="round"/><circle cx="29" cy="27" r="2.6" fill="#fff"/></svg>"##;

/// `https://<api-host>/api/solana-pay/icon.svg`, from the request the wallet
/// just made. Behind Vercel the scheme comes from the forwarded header.
fn icon_url(headers: &HeaderMap) -> Option<String> {
    let host = headers.get(header::HOST)?.to_str().ok()?;
    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_else(|| {
            if host.starts_with("localhost") || host.starts_with("127.0.0.1") {
                "http"
            } else {
                "https"
            }
        });
    Some(format!("{scheme}://{host}/api/solana-pay/icon.svg"))
}

/// `POST /api/solana-pay/{token}` — builds the transaction the wallet asked
/// for. Nothing is written anywhere: the wallet signs and sends it itself,
/// which is what creates the invoice account on chain.
pub async fn transaction(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Query(query): Query<AccountQuery>,
    Json(request): Json<AccountRequest>,
) -> ApiResult<Json<TransactionRequest>> {
    let invoice = decode_invoice(&token)?;
    let account = query
        .account
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or(&request.account);
    let payer = parse_pubkey(account, "account")?;

    // A discount code rides in the invoice, but the amount it prices is
    // resolved against the seller's own list. The address follows the amount:
    // the program derives a different invoice per price, so this pays exactly
    // what the buyer's screen shows.
    let payable = invoice.payable_lamports(&state.db).await?;
    let payment = invoice.address_for(payable)?;

    // Only known clusters, never the endpoint carried in the token: a code is
    // just data handed to us by a stranger, and this fetch must not become a
    // request to an arbitrary host.
    let endpoint = resolve_endpoint(None, &invoice.cluster)?;
    let rpc = RpcClient::new(endpoint);
    let blockhash = rpc.get_latest_blockhash().await.map_err(|error| {
        tracing::info!(%error, "latest blockhash unavailable for a solana pay request");
        ApiError::Internal("The network did not return a blockhash. Try again.".into())
    })?;

    let wire = settle_payment_transaction(SettlePaymentRequest {
        payer,
        payment,
        seller: invoice.seller()?,
        reference: invoice.reference()?,
        amount: payable,
        memo: &invoice.memo,
        blockhash: blockhash.to_bytes(),
    });

    Ok(Json(TransactionRequest {
        transaction: STANDARD.encode(wire),
        message: format!("Pay {} SOL to Payvand", format_lamports(payable)),
    }))
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
    fn seller(&self) -> ApiResult<Pubkey> {
        parse_pubkey(&self.seller, "seller")
    }

    fn reference(&self) -> ApiResult<Pubkey> {
        parse_pubkey(&self.reference, "reference")
    }

    /// The invoice address for a given price. The amount is part of the PDA
    /// seeds, so each price — full or discounted — has its own address.
    fn address_for(&self, amount: u64) -> ApiResult<Pubkey> {
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
    async fn payable_lamports(&self, db: &libsql::Database) -> ApiResult<u64> {
        let Some(discount) = &self.discount else {
            return Ok(self.amount_lamports);
        };
        let seller_user = self
            .seller_user
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                bad("This payment's discount cannot be checked. Open the payment page to pay.")
            })?;

        let resolved = resolve_for_seller(db, seller_user, &discount.code)
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

    fn label(&self) -> String {
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

    fn description(&self, payable: u64) -> String {
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

struct SettlePaymentRequest<'a> {
    payer: Pubkey,
    payment: Pubkey,
    seller: Pubkey,
    reference: Pubkey,
    amount: u64,
    memo: &'a str,
    blockhash: [u8; 32],
}

/// The unsigned transaction a wallet signs: one `settle_payment` instruction,
/// the wallet as fee payer and the only signer.
///
/// Built by hand in the legacy wire format because that is the one every
/// wallet agrees on. The fixture test below parses the bytes back, so a
/// change in layout cannot pass unnoticed.
fn settle_payment_transaction(request: SettlePaymentRequest<'_>) -> Vec<u8> {
    let program = program_id().expect("the program id in this crate is valid");
    let system_program =
        Pubkey::from_str(SYSTEM_PROGRAM_ID).expect("the system program id is valid");

    // Account order follows the runtime's rules: signers first, then the
    // writable accounts, then the readonly ones. The payer is the only
    // signer, so the header reads 1 required signature and 3 readonly
    // unsigned accounts (reference, system program, program).
    let keys = [
        request.payer,
        request.payment,
        request.seller,
        request.reference,
        system_program,
        program,
    ];

    let mut data = Vec::with_capacity(8 + 8 + 4 + request.memo.len());
    data.extend_from_slice(&settle_payment_discriminator());
    data.extend_from_slice(&request.amount.to_le_bytes());
    data.extend_from_slice(&(request.memo.len() as u32).to_le_bytes());
    data.extend_from_slice(request.memo.as_bytes());

    let mut message = Vec::with_capacity(3 + 1 + keys.len() * 32 + 32 + 32 + data.len());
    message.extend_from_slice(&[1, 0, 3]);
    push_compact_u16(&mut message, keys.len() as u16);
    for key in keys {
        message.extend_from_slice(key.as_ref());
    }
    message.extend_from_slice(&request.blockhash);
    push_compact_u16(&mut message, 1);
    message.push(5); // program id index
    push_compact_u16(&mut message, 5);
    message.extend_from_slice(&[0, 1, 2, 3, 4]);
    push_compact_u16(&mut message, data.len() as u16);
    message.extend_from_slice(&data);

    // No signatures yet: the spec has the wallet fill the payer's slot (and
    // treat the message's fee payer as the account it was called with).
    let mut transaction = Vec::with_capacity(1 + message.len());
    push_compact_u16(&mut transaction, 0);
    transaction.extend_from_slice(&message);
    transaction
}

/// Anchor's instruction discriminator: the first 8 bytes of
/// `sha256("global:settle_payment")`.
fn settle_payment_discriminator() -> [u8; 8] {
    let digest = Sha256::digest(b"global:settle_payment");
    digest[..8].try_into().expect("8 byte slice")
}

fn program_id() -> ApiResult<Pubkey> {
    Pubkey::from_str(PAYVAND_PROGRAM_ID)
        .map_err(|error| ApiError::Internal(format!("bad program id: {error}")))
}

/// Solana's compact-u16 ("shortvec") length prefix.
fn push_compact_u16(out: &mut Vec<u8>, mut value: u16) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        byte |= 0x80;
        out.push(byte);
    }
}

/// `0.01` rather than `0.010000000`: the number the seller typed.
fn format_lamports(lamports: u64) -> String {
    let whole = lamports / 1_000_000_000;
    let fraction = lamports % 1_000_000_000;
    if fraction == 0 {
        return whole.to_string();
    }
    let digits = format!("{fraction:09}");
    format!("{whole}.{}", digits.trim_end_matches('0'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_invoice() -> (Invoice, String) {
        let seller = Pubkey::new_from_array([2u8; 32]);
        let reference = Pubkey::new_from_array([3u8; 32]);
        let amount = 10_000_000u64;
        let (payment, _) = Pubkey::find_program_address(
            &[
                PAYMENT_SEED,
                seller.as_ref(),
                reference.as_ref(),
                &amount.to_le_bytes(),
            ],
            &program_id().expect("program id"),
        );
        let invoice = Invoice {
            v: QR_PAYLOAD_VERSION,
            id: payment.to_string(),
            seller: seller.to_string(),
            reference: reference.to_string(),
            amount_lamports: amount,
            memo: "Coffee".to_string(),
            name: "Flat white".to_string(),
            description: String::new(),
            seller_user: None,
            discount: None,
            cluster: "devnet".to_string(),
        };
        let token = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&invoice).expect("json"));
        (invoice, token)
    }

    #[test]
    fn decodes_the_token_from_a_qr_code() {
        let (invoice, token) = sample_invoice();
        let decoded = decode_invoice(&token).expect("token decodes");
        assert_eq!(decoded.id, invoice.id);
        assert_eq!(decoded.amount_lamports, 10_000_000);
        assert_eq!(decoded.label(), "Flat white");
    }

    #[test]
    fn rejects_an_amount_that_does_not_match_the_address() {
        let (mut invoice, _) = sample_invoice();
        invoice.amount_lamports += 1;
        let token = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&invoice).expect("json"));
        assert!(decode_invoice(&token).is_err());
    }

    #[test]
    fn rejects_an_unknown_payload_version() {
        let (mut invoice, _) = sample_invoice();
        invoice.v = 99;
        let token = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&invoice).expect("json"));
        assert!(decode_invoice(&token).is_err());
    }

    #[test]
    fn discriminator_matches_the_idl() {
        // `settle_payment` in apps/web/app/assets/payvand.idl.json.
        assert_eq!(
            settle_payment_discriminator(),
            [129, 7, 163, 250, 122, 226, 158, 249]
        );
    }

    #[test]
    fn builds_a_legacy_transaction_a_wallet_can_sign() {
        let payer = Pubkey::new_from_array([1u8; 32]);
        let payment = Pubkey::new_from_array([9u8; 32]);
        let seller = Pubkey::new_from_array([2u8; 32]);
        let reference = Pubkey::new_from_array([3u8; 32]);
        let blockhash = [7u8; 32];

        let wire = settle_payment_transaction(SettlePaymentRequest {
            payer,
            payment,
            seller,
            reference,
            amount: 10_000_000,
            memo: "Coffee",
            blockhash,
        });

        // Unsigned: no signature slots, just the message the wallet signs.
        assert_eq!(wire[0], 0);

        let message = &wire[1..];
        let mut offset = 0;
        assert_eq!(&message[offset..offset + 3], &[1, 0, 3]);
        offset += 3;

        assert_eq!(message[offset], 6);
        offset += 1;
        let keys = &message[offset..offset + 6 * 32];
        for (index, key) in [payer, payment, seller, reference].iter().enumerate() {
            assert_eq!(&keys[index * 32..(index + 1) * 32], key.as_ref());
        }
        offset += 6 * 32;

        assert_eq!(&message[offset..offset + 32], &blockhash);
        offset += 32;

        assert_eq!(message[offset], 1); // one instruction
        offset += 1;
        assert_eq!(message[offset], 5); // program id index
        offset += 1;
        assert_eq!(message[offset], 5); // account count
        offset += 1;
        assert_eq!(&message[offset..offset + 5], &[0, 1, 2, 3, 4]);
        offset += 5;

        let data_len = message[offset] as usize;
        offset += 1;
        let data = &message[offset..offset + data_len];
        assert_eq!(&data[..8], &settle_payment_discriminator());
        assert_eq!(&data[8..16], &10_000_000u64.to_le_bytes());
        assert_eq!(&data[16..20], &6u32.to_le_bytes());
        assert_eq!(&data[20..], b"Coffee");
        assert_eq!(offset + data_len, message.len());
    }

    #[test]
    fn formats_amounts_without_trailing_zeroes() {
        assert_eq!(format_lamports(10_000_000), "0.01");
        assert_eq!(format_lamports(1_000_000_000), "1");
        assert_eq!(format_lamports(1_500_000_000), "1.5");
    }
}
