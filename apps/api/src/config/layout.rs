//! The program's `Config` account, decoded by offset, and the account sizes a
//! quote asks the cluster's rent for.

use sha2::{Digest, Sha256};
use solana_pubkey::Pubkey;

use super::model::{ConfigResponse, Fees, Mints, Rent, Spaces};
use crate::payments;

/// `CONFIG_SEED`: `seeds = [b"config"]` under the Payvand program.
pub(super) const CONFIG_SEED: &[u8] = b"config";
/// `CONFIG_SPACE` in the program: the account this endpoint reads.
pub(super) const CONFIG_SPACE: usize = 184;

/// The account sizes a quote asks the cluster's rent for. The names are the
/// ones the response and the client both use, so a size is never guessed twice.
pub(super) const PAYMENT_SPACE: u64 = 230;
pub(super) const TOKEN_PAYMENT_SPACE: u64 = 262;
pub(super) const PAYMENT_RECEIPT_SPACE: u64 = 48;
pub(super) const TOKEN_ACCOUNT_SPACE: u64 = 165;
pub(super) const BUDGET_SPACE: u64 = 97;
pub(super) const TOKEN_BUDGET_SPACE: u64 = 129;

/// A `Config` account decoded: every field of the answer except the rents, which
/// the account does not hold.
///
/// Keeping them out is deliberate — the only way to build a `ConfigResponse` is
/// `with_rent`, so a decoded policy can never be handed out with zeroed rents
/// beside it.
pub(super) struct Policy {
    cluster: String,
    program_id: String,
    admin: String,
    fee_mode: String,
    session_fee_float: u64,
    fees: Fees,
    mints: Mints,
    spaces: Spaces,
}

impl Policy {
    /// The answer: this policy with the rents read from the same cluster.
    pub(super) fn with_rent(self, rent: Rent) -> ConfigResponse {
        ConfigResponse {
            cluster: self.cluster,
            program_id: self.program_id,
            admin: self.admin,
            fee_mode: self.fee_mode,
            session_fee_float: self.session_fee_float,
            fees: self.fees,
            mints: self.mints,
            spaces: self.spaces,
            rent,
        }
    }
}

/// The `Config` account, read by offset.
///
/// Mirrors the program's layout: the discriminator, the admin, the `FeePolicy`
/// (two rates, their two boundaries, the two per-currency flats, the two mints,
/// the network fee in lamports and in each token), the fee mode, the session
/// float and the bump/version bytes this endpoint does not need.
pub(super) fn decode_policy(data: &[u8], cluster: &str) -> Option<Policy> {
    if data.len() < CONFIG_SPACE {
        return None;
    }
    let tagged = Sha256::digest(b"account:Config");
    if data[..8] != tagged[..8] {
        return None;
    }

    let read_u16 = |offset: usize| u16::from_le_bytes([data[offset], data[offset + 1]]);
    let read_u64 = |offset: usize| {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&data[offset..offset + 8]);
        u64::from_le_bytes(bytes)
    };
    let read_key = |offset: usize| {
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&data[offset..offset + 32]);
        Pubkey::new_from_array(bytes)
    };

    let admin = read_key(8);
    let usdc = read_key(76);
    let eurc = read_key(108);
    // The policy names its own mints; a deployment whose account predates them
    // falls back to the cluster's Circle addresses rather than an empty string.
    let mint_or = |named: Pubkey, currency: &str| {
        if named == Pubkey::default() {
            payments::mint_for(currency, cluster)
                .map(|mint| mint.to_string())
                .unwrap_or_default()
        } else {
            named.to_string()
        }
    };

    Some(Policy {
        cluster: cluster.to_string(),
        program_id: payments::PAYVAND_PROGRAM_ID.to_string(),
        admin: admin.to_string(),
        fee_mode: if data[164] == 1 { "buyer" } else { "seller" }.to_string(),
        session_fee_float: read_u64(165),
        mints: Mints {
            usdc: mint_or(usdc, "usdc"),
            eurc: mint_or(eurc, "eurc"),
        },
        fees: Fees {
            fee_bps_below: read_u16(40),
            fee_bps_above: read_u16(42),
            sol_tier_lamports: read_u64(44),
            token_tier_base_units: read_u64(52),
            flat_usdc_base_units: read_u64(60),
            flat_eurc_base_units: read_u64(68),
            network_fee_lamports: read_u64(140),
            network_fee_usdc_base_units: read_u64(148),
            network_fee_eurc_base_units: read_u64(156),
        },
        spaces: Spaces {
            payment: PAYMENT_SPACE,
            token_payment: TOKEN_PAYMENT_SPACE,
            payment_receipt: PAYMENT_RECEIPT_SPACE,
            token_account: TOKEN_ACCOUNT_SPACE,
            budget: BUDGET_SPACE,
            token_budget: TOKEN_BUDGET_SPACE,
        },
    })
}

#[cfg(test)]
mod tests;
