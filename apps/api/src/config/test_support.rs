//! Fixtures shared by the config tests.

use sha2::{Digest, Sha256};

use super::layout::decode_policy;
use super::model::{ConfigResponse, Rent};

/// A `Config` account as the program writes one: a 2%/1% marginal policy, a
/// 0.1 SOL and 10 token boundary, two flats, both mints, the network fees in
/// lamports and in each token, a seller-paid mode and a 12,000,000 float.
pub(super) fn policy_bytes() -> Vec<u8> {
    let mut data = Sha256::digest(b"account:Config")[..8].to_vec();
    data.extend_from_slice(&[6u8; 32]); // admin
    data.extend_from_slice(&200u16.to_le_bytes()); // fee_bps_below
    data.extend_from_slice(&100u16.to_le_bytes()); // fee_bps_above
    data.extend_from_slice(&100_000_000u64.to_le_bytes()); // sol_tier_lamports
    data.extend_from_slice(&10_000_000u64.to_le_bytes()); // token_tier_base_units
    data.extend_from_slice(&62_782u64.to_le_bytes()); // flat_usdc_base_units
    data.extend_from_slice(&57_346u64.to_le_bytes()); // flat_eurc_base_units
    data.extend_from_slice(&[4u8; 32]); // usdc_mint
    data.extend_from_slice(&[5u8; 32]); // eurc_mint
    data.extend_from_slice(&5_000u64.to_le_bytes()); // network_fee_lamports
    data.extend_from_slice(&351u64.to_le_bytes()); // network_fee_usdc_base_units
    data.extend_from_slice(&320u64.to_le_bytes()); // network_fee_eurc_base_units
    data.push(1); // fee_mode: BuyerPays
    data.extend_from_slice(&12_000_000u64.to_le_bytes());
    data.extend_from_slice(&1_760_000_000i64.to_le_bytes()); // flats_updated_at
    data.push(251); // bump
    data.push(250); // treasury_bump
    data.push(4); // version
    data
}

/// A full answer: the sample policy with plausible rents beside it.
pub(super) fn sample_config() -> ConfigResponse {
    decode_policy(&policy_bytes(), "devnet")
        .expect("a policy")
        .with_rent(Rent {
            payment: 1_000_000,
            token_payment: 1_100_000,
            payment_receipt: 890_880,
            token_account: 2_039_280,
            budget: 890_880,
            token_budget: 1_000_000,
        })
}

pub(super) fn base64_encode(bytes: &[u8]) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine};
    STANDARD.encode(bytes)
}
