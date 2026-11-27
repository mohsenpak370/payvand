//! One read of the fee policy and the six rent minimums from a cluster.

use solana_pubkey::Pubkey;
use std::str::FromStr;

use super::layout::{
    decode_policy, BUDGET_SPACE, CONFIG_SEED, PAYMENT_RECEIPT_SPACE, PAYMENT_SPACE,
    TOKEN_ACCOUNT_SPACE, TOKEN_BUDGET_SPACE, TOKEN_PAYMENT_SPACE,
};
use super::model::{ConfigResponse, Rent};
use crate::payments::{self, ApiError, ApiResult};

/// Reads the policy account and the six rent minimums from `endpoint`.
///
/// A cluster that cannot answer is an error rather than a guess: a quote built
/// on an invented fee is a transaction that fails its pre-flight, or charges the
/// wrong amount.
pub async fn read_config(endpoint: &str, cluster: &str) -> ApiResult<ConfigResponse> {
    let program = Pubkey::from_str(payments::PAYVAND_PROGRAM_ID)
        .map_err(|error| ApiError::Internal(format!("bad program id: {error}")))?;
    let address = Pubkey::find_program_address(&[CONFIG_SEED], &program).0;

    let rpc = payments::payment_rpc(endpoint.to_owned());
    let account = match rpc.get_account(&address).await {
        Ok(account) => account,
        Err(error) => {
            tracing::info!(%error, %address, "the fee policy is not readable");
            return Err(ApiError::Internal(
                "The fee policy is not readable. Try again.".into(),
            ));
        }
    };

    let policy = decode_policy(&account.data, cluster).ok_or_else(|| {
        ApiError::Internal("The fee policy account is not one this program wrote.".into())
    })?;

    let (payment, token_payment, payment_receipt, token_account, budget, token_budget) = tokio::join!(
        rpc.get_minimum_balance_for_rent_exemption(PAYMENT_SPACE as usize),
        rpc.get_minimum_balance_for_rent_exemption(TOKEN_PAYMENT_SPACE as usize),
        rpc.get_minimum_balance_for_rent_exemption(PAYMENT_RECEIPT_SPACE as usize),
        rpc.get_minimum_balance_for_rent_exemption(TOKEN_ACCOUNT_SPACE as usize),
        rpc.get_minimum_balance_for_rent_exemption(BUDGET_SPACE as usize),
        rpc.get_minimum_balance_for_rent_exemption(TOKEN_BUDGET_SPACE as usize),
    );
    let rent = |value: Result<u64, _>| {
        value.map_err(|error| {
            tracing::info!(%error, "a rent minimum is not readable");
            ApiError::Internal("The cluster's rents are not readable. Try again.".into())
        })
    };

    let rents = Rent {
        payment: rent(payment)?,
        token_payment: rent(token_payment)?,
        payment_receipt: rent(payment_receipt)?,
        token_account: rent(token_account)?,
        budget: rent(budget)?,
        token_budget: rent(token_budget)?,
    };
    // A cluster that answers zero for a rent is not answering: no account of any
    // size is free, and a quote built on zero would understate what the payer
    // gets back and what the seller funds.
    for (size, lamports) in [
        (PAYMENT_SPACE, rents.payment),
        (TOKEN_PAYMENT_SPACE, rents.token_payment),
        (PAYMENT_RECEIPT_SPACE, rents.payment_receipt),
        (TOKEN_ACCOUNT_SPACE, rents.token_account),
        (BUDGET_SPACE, rents.budget),
        (TOKEN_BUDGET_SPACE, rents.token_budget),
    ] {
        if lamports == 0 {
            tracing::info!(size, "a rent minimum came back as zero");
            return Err(ApiError::Internal(
                "The cluster's rents are not readable. Try again.".into(),
            ));
        }
    }

    Ok(policy.with_rent(rents))
}

#[cfg(test)]
mod tests;
