//! PDA seeds shared with the Anchor program.

use solana_pubkey::Pubkey;

/// Seed prefix for SOL invoices, mirroring the Anchor program.
pub const PAYMENT_SEED: &[u8] = b"payment";
/// Seed prefix for token invoices. A different prefix, and the mint is one of
/// its seeds, so a USDC invoice and a SOL invoice never share an address.
pub const TOKEN_PAYMENT_SEED: &[u8] = b"token-payment";
/// Seed prefix for a settled invoice's permanent receipt. Keyed by the invoice
/// address, so the record is reachable from the address in the QR code alone —
/// which is all a watcher ever holds once the invoice account is closed.
pub const PAYMENT_RECEIPT_SEED: &[u8] = b"payment-receipt";

/// `["payment-receipt", invoice]`, mirroring the Anchor program.
pub fn receipt_pda(program: &Pubkey, invoice: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[PAYMENT_RECEIPT_SEED, invoice.as_ref()], program).0
}
