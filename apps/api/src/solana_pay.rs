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
//!
//! This file is the public surface. Implementation and unit tests live in the
//! `solana_pay/` directory:
//!
//! - `handler`: `GET`/`POST /api/solana-pay/{token}` and the wallet icon
//! - `invoice`: decoding and verifying the token, and pricing its discount
//! - `wire`: the unsigned `settle_payment` transaction a wallet signs
//! - `model`: the request and response bodies of the spec

mod handler;
mod invoice;
mod model;
mod wire;

pub use handler::{icon, info, transaction};
pub use invoice::{decode_invoice, Invoice};
pub use model::{AccountQuery, AccountRequest, RequestInfo, TransactionRequest};
