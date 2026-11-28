//! The settle a request names, checked before anything is rebuilt from it.

use std::str::FromStr;

use solana_sdk::pubkey::Pubkey;

use crate::payments::{bad, currency_for_mint, ApiResult};

use super::model::SettleFields;

fn key(address: &str, what: &str) -> ApiResult<Pubkey> {
    Pubkey::from_str(address.trim())
        .map_err(|_| bad(format!("That {what} is not a valid Solana address.")))
}

/// The parameters the rebuilt message is made of, after validation.
pub(super) struct Params {
    pub(super) buyer: Pubkey,
    pub(super) seller: Pubkey,
    pub(super) reference: Pubkey,
    /// `None` is a SOL invoice.
    pub(super) mint: Option<Pubkey>,
    pub(super) amount: u64,
    pub(super) memo: String,
}

impl Params {
    /// Everything a request may name, checked against what the program will
    /// accept: a known stablecoin mint for the cluster, a positive price, a
    /// memo the invoice can hold, and real keys. The settle the relayer signs is
    /// built from these fields alone, never from bytes a caller sent.
    pub(super) fn read(request: &SettleFields) -> ApiResult<Self> {
        if request.amount == 0 {
            return Err(bad("The price must be greater than zero."));
        }
        if request.memo.len() > 96 {
            return Err(bad("The invoice note is too long."));
        }
        // No mint is a SOL invoice: the price itself is lamports, so there is
        // nothing to check against the cluster's stablecoins.
        let mint = if request.mint.trim().is_empty() {
            None
        } else {
            let mint = key(&request.mint, "mint")?;
            if currency_for_mint(&mint, &request.cluster).is_none() {
                return Err(bad("Sponsored payments cover USDC, EURC and SOL only."));
            }
            Some(mint)
        };
        Ok(Self {
            buyer: key(&request.buyer, "buyer")?,
            seller: key(&request.seller, "seller")?,
            reference: key(&request.reference, "reference")?,
            mint,
            amount: request.amount,
            memo: request.memo.clone(),
        })
    }
}

/// A token settle the tests build messages from: USDC on devnet.
#[cfg(test)]
pub(super) fn fixture(memo: &str) -> Params {
    Params {
        buyer: Pubkey::new_from_array([1; 32]),
        seller: Pubkey::new_from_array([2; 32]),
        reference: Pubkey::new_from_array([3; 32]),
        mint: Some(crate::payments::mint_for("usdc", "devnet").expect("USDC on devnet")),
        amount: 4_500_000,
        memo: memo.into(),
    }
}

#[cfg(test)]
mod tests;
