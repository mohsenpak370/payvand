//! The shape `GET /api/config` answers with, and the query it takes.

use serde::{Deserialize, Serialize};

/// What the browser receives. Field names are the ones the client reads, so the
/// shape is one contract rather than a translation.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ConfigResponse {
    pub cluster: String,
    pub program_id: String,
    pub admin: String,
    pub fee_mode: String,
    pub session_fee_float: u64,
    pub fees: Fees,
    pub mints: Mints,
    pub spaces: Spaces,
    pub rent: Rent,
}

/// `FeePolicy` plus the two per-signature network fees beside it: the numbers
/// `fee_for` reads at settlement.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Fees {
    pub fee_bps_below: u16,
    pub fee_bps_above: u16,
    pub sol_tier_lamports: u64,
    pub token_tier_base_units: u64,
    pub flat_usdc_base_units: u64,
    pub flat_eurc_base_units: u64,
    pub network_fee_lamports: u64,
    pub network_fee_usdc_base_units: u64,
    pub network_fee_eurc_base_units: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Mints {
    pub usdc: String,
    pub eurc: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Spaces {
    pub payment: u64,
    pub token_payment: u64,
    pub payment_receipt: u64,
    pub token_account: u64,
    pub budget: u64,
    pub token_budget: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Rent {
    pub payment: u64,
    pub token_payment: u64,
    pub payment_receipt: u64,
    pub token_account: u64,
    pub budget: u64,
    pub token_budget: u64,
}

#[derive(Debug, Deserialize)]
pub struct ConfigQuery {
    /// Which cluster's policy to answer; the deployment's own when omitted.
    pub cluster: Option<String>,
}
