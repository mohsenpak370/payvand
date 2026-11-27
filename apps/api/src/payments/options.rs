//! Turns requested currencies into priced options and derives their PDAs.

use solana_pubkey::Pubkey;

use super::{
    config::{mint_for, program_id},
    create::CreatePaymentRequest,
    error::{bad, parse_pubkey, ApiResult},
    model::{PaymentOption, SOL_CURRENCY, SOL_DECIMALS, STABLE_DECIMALS},
    pda::{PAYMENT_SEED, TOKEN_PAYMENT_SEED},
};

/// SOL, USDC and EURC — more than that makes the QR too dense to scan.
pub const MAX_OPTIONS: usize = 3;

/// Turns the requested currencies into priced options, deriving the invoice
/// address for each one. Addresses are derived here rather than taken from the
/// request: the seeds (`seller`, `reference`, currency, amount) are the whole
/// safety story of a QR code, so the API computes them from the same facts it
/// validated instead of trusting an address a client sent.
pub fn build_options(request: &CreatePaymentRequest) -> ApiResult<Vec<PaymentOption>> {
    let program = program_id()?;
    let seller = parse_pubkey(&request.seller, "seller")?;
    let reference = parse_pubkey(&request.reference, "reference")?;

    // No options means an older client: one SOL price, as before.
    if request.options.is_empty() {
        let amount = request.amount_lamports;
        if amount == 0 {
            return Err(bad("amount_lamports must be greater than 0"));
        }
        let (pda, _) = Pubkey::find_program_address(
            &[
                PAYMENT_SEED,
                seller.as_ref(),
                reference.as_ref(),
                &amount.to_le_bytes(),
            ],
            &program,
        );
        return Ok(vec![PaymentOption {
            currency: SOL_CURRENCY.to_string(),
            mint: String::new(),
            decimals: SOL_DECIMALS,
            amount,
            payment_pda: pda.to_string(),
        }]);
    }

    if request.options.len() > MAX_OPTIONS {
        return Err(bad(format!(
            "an invoice can accept at most {MAX_OPTIONS} currencies"
        )));
    }

    let mut options: Vec<PaymentOption> = Vec::with_capacity(request.options.len());
    for requested in &request.options {
        let currency = requested.currency.trim().to_ascii_lowercase();
        if requested.amount == 0 {
            return Err(bad(format!("the {currency} amount must be greater than 0")));
        }
        if options.iter().any(|option| option.currency == currency) {
            return Err(bad(format!("{currency} is offered twice")));
        }

        if currency == SOL_CURRENCY {
            let (pda, _) = Pubkey::find_program_address(
                &[
                    PAYMENT_SEED,
                    seller.as_ref(),
                    reference.as_ref(),
                    &requested.amount.to_le_bytes(),
                ],
                &program,
            );
            options.push(PaymentOption {
                currency,
                mint: String::new(),
                decimals: SOL_DECIMALS,
                amount: requested.amount,
                payment_pda: pda.to_string(),
            });
            continue;
        }

        let mint = mint_for(&currency, &request.cluster).ok_or_else(|| {
            bad(format!(
                "{currency} cannot be charged on the {} network",
                request.cluster
            ))
        })?;
        let (pda, _) = Pubkey::find_program_address(
            &[
                TOKEN_PAYMENT_SEED,
                seller.as_ref(),
                reference.as_ref(),
                mint.as_ref(),
                &requested.amount.to_le_bytes(),
            ],
            &program,
        );
        options.push(PaymentOption {
            currency,
            mint: mint.to_string(),
            decimals: STABLE_DECIMALS,
            amount: requested.amount,
            payment_pda: pda.to_string(),
        });
    }

    Ok(options)
}

#[cfg(test)]
mod tests;
