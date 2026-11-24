//! Small pieces both payment paths need: the calendar month a budget resets
//! on, the fee a settlement owes, and the treasury moves that pay for it.

use anchor_lang::prelude::*;

use crate::constants::{
    MAX_NETWORK_FEE_BASE_UNITS, MAX_NETWORK_FEE_LAMPORTS, MAX_SOL_FEE_LAMPORTS,
    MAX_SOL_PRICE_MICROS, MAX_TOKEN_FEE_BASE_UNITS, MICROS, MINT_DECIMALS_OFFSET,
    MIN_SESSION_FEE_FLOAT, PRICE_ANCHOR_BAND_BPS, SHOP_FEE_BASE_BPS, TREASURY_SEED,
};
use crate::errors::PaymentError;
use crate::state::{Config, FeePolicy, ShopPolicy};

/// `year * 12 + month` (month 1-based) for the current clock, so a monthly
/// allowance resets on a real calendar-month boundary rather than every 30
/// days. Howard Hinnant's civil-from-days, without pulling in a date crate.
pub fn current_month_index() -> Result<u64> {
    let ts = Clock::get()?.unix_timestamp.max(0) as u64;
    let z = (ts / 86_400) as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    Ok(year as u64 * 12 + month as u64)
}

/// The fee a settlement of `amount` owes under `config`.
///
/// `mint` is `None` on the SOL paths and the invoice's mint on the token ones.
/// The rate is marginal: the part of the amount at or below the currency's
/// boundary pays `fee_bps_below`, the part above it pays `fee_bps_above`, so the
/// boundary is a change of slope and not a cliff — an invoice crossing it costs
/// one lamport's worth of rate more, never twice the fee.
///
/// The floor is what the payment costs us, expressed in the currency the seller
/// is charged in: the permanent record's rent plus the network fee we paid.
///
/// A token seller cannot be charged lamports, so both costs are the currency's
/// `flat` and `network_fee_*_base_units`, the treasury funds them in SOL, and the
/// fee is the larger of that sum and the percentage. A payment too small for the
/// percentage to cover the two costs is charged exactly those costs and nothing
/// else, and the treasury neither loses on it nor takes a margin for it. A
/// payment big enough for the percentage to cover them is charged the
/// percentage, and the difference is the protocol's revenue.
///
/// A SOL seller is charged in lamports, so its two costs are the marker's rent
/// (`receipt_rent`, paid straight to the marker by the sponsor) and
/// `network_fee_lamports`; the floor is their sum, applied the only way a lamport
/// amount can carry it — by taking both off the percentage. `fee` is therefore
/// the seller's deduction above the marker's rent, and the settlement splits it
/// into the sponsor's reimbursement and the platform's margin.
///
/// `multiplier_bps` is the shop's own policy when the invoice named one, and
/// `SHOP_FEE_BASE_BPS` otherwise. It scales the percentage only — the
/// boundaries, the flats and the cap are `Config`'s, so a shop is priced from
/// the same live numbers as every other and a refresh still has one place to
/// write. A `0` multiplier therefore charges no percentage while a token invoice
/// still pays the marker's flat: a free shop cannot make the treasury fund the
/// record.
///
/// The result is capped twice. `MAX_SOL_FEE_LAMPORTS` (0.1 SOL) or
/// `MAX_TOKEN_FEE_BASE_UNITS` (10 USDC/EURC) is the ceiling a percentage fee may
/// reach, so a large invoice pays a plausibly-sized fee rather than a slice of
/// itself that keeps growing — the ceiling is applied last, after the rate, the
/// multiplier and the rent, so nothing can carry the fee past it. Then
/// `amount - 1` keeps the guard on the price (`amount > rent + fee`) from being
/// satisfiable by taking the whole invoice; that one is a safety net, not a
/// policy.
pub fn fee_for(
    config: &Config,
    amount: u64,
    mint: Option<&Pubkey>,
    receipt_rent: u64,
    multiplier_bps: u16,
) -> Result<u64> {
    fee_for_signatures(config, amount, mint, receipt_rent, multiplier_bps, 1)
}

/// The network fee a settlement of `signatures` signatures costs, and therefore
/// what it charges the seller and refunds the sponsor.
///
/// `FeePolicy::network_fee_lamports` is one signature's cost. A settle that a
/// sponsor signs as fee payer carries two — the payer's and the sponsor's — and
/// one that pays its own way carries one, so the cost is multiplied rather than
/// guessed. Charging and refunding the same number is what keeps a refund from
/// handing back more lamports than the path spent.
pub fn fee_for_signatures(
    config: &Config,
    amount: u64,
    mint: Option<&Pubkey>,
    receipt_rent: u64,
    multiplier_bps: u16,
    signatures: u64,
) -> Result<u64> {
    let policy = &config.policy;
    let signatures = signatures.max(1);
    let (boundary, flat, network) = match mint {
        // The SOL floor's two terms are both lamports, added after the rate is
        // taken down by the marker's rent rather than before it.
        None => (
            policy.sol_tier_lamports as u128,
            0u128,
            (policy.network_fee_lamports as u128)
                .checked_mul(signatures as u128)
                .ok_or(PaymentError::FeeOverflow)?,
        ),
        Some(mint) if *mint == policy.usdc_mint => (
            policy.token_tier_base_units as u128,
            policy.flat_usdc_base_units as u128,
            (policy.network_fee_usdc_base_units as u128)
                .checked_mul(signatures as u128)
                .ok_or(PaymentError::FeeOverflow)?,
        ),
        Some(mint) if *mint == policy.eurc_mint => (
            policy.token_tier_base_units as u128,
            policy.flat_eurc_base_units as u128,
            (policy.network_fee_eurc_base_units as u128)
                .checked_mul(signatures as u128)
                .ok_or(PaymentError::FeeOverflow)?,
        ),
        // A mint the policy does not price has no live conversion to charge a
        // lamport cost in, so it pays the percentage alone.
        Some(_) => (policy.token_tier_base_units as u128, 0u128, 0u128),
    };

    let amount = amount as u128;
    let below = amount.min(boundary);
    let above = amount.saturating_sub(boundary);
    let proportional = below
        .checked_mul(policy.fee_bps_below as u128)
        .and_then(|first| {
            above
                .checked_mul(policy.fee_bps_above as u128)
                .and_then(|second| first.checked_add(second))
        })
        .ok_or(PaymentError::FeeOverflow)?
        / 10_000;
    // The shop's discount, applied to the rate before the carriers below read
    // it: the flat is the marker's rent and is never scaled, and the SOL path's
    // rent subtraction is not either, so a shop can reach zero percentage but
    // not shift the record onto the treasury.
    let proportional = proportional
        .checked_mul(multiplier_bps.min(SHOP_FEE_BASE_BPS) as u128)
        .ok_or(PaymentError::FeeOverflow)?
        / SHOP_FEE_BASE_BPS as u128;

    let fee = match mint {
        // The floor is everything the payment costs us, in this mint's units:
        // the record's rent and the network fee the sponsor paid.
        Some(_) => proportional.max(flat.saturating_add(network)),
        // The rent is already off the percentage (the seller pays it to the
        // marker through this same split), so the fee only has to carry the
        // network fee from here.
        None => proportional
            .saturating_sub(receipt_rent as u128)
            .max(network),
    };
    let ceiling = match mint {
        None => MAX_SOL_FEE_LAMPORTS as u128,
        Some(_) => MAX_TOKEN_FEE_BASE_UNITS as u128,
    };
    Ok(fee.min(ceiling).min(amount.saturating_sub(1)) as u64)
}

/// The multiplier a settlement charges: the shop's own when the invoice named
/// its policy, `SHOP_FEE_BASE_BPS` when it did not.
///
/// The account is optional, which is the whole reason every invoice printed
/// before shop rates existed still settles: an invoice that names none pays
/// `Config`'s rate exactly as it always did. A policy whose recorded seller is
/// not the invoice's seller is refused by name — the seeds prove an account is
/// the canonical PDA for its own contents, not that it belongs to this
/// settlement's seller, so that comparison is what stops one shop pricing
/// another's invoice at its own discount.
pub fn shop_multiplier(shop_policy: Option<&ShopPolicy>, seller: &Pubkey) -> Result<u16> {
    match shop_policy {
        None => Ok(SHOP_FEE_BASE_BPS),
        Some(policy) => {
            require_keys_eq!(
                policy.seller,
                *seller,
                PaymentError::ShopPolicySellerMismatch
            );
            Ok(policy.multiplier_bps)
        }
    }
}

/// The multiplier `set_shop_policy` accepts: `0..=10_000`.
///
/// The cap is checked on write rather than at settle time, so there is no
/// out-of-band state to handle: a shop can discount itself to nothing and can
/// never surcharge a buyer above `Config`'s rate, and every settlement reads a
/// number that is already known to be in range.
pub fn validate_shop_multiplier(multiplier_bps: u16) -> Result<()> {
    require!(
        multiplier_bps <= SHOP_FEE_BASE_BPS,
        PaymentError::ShopFeeTooHigh
    );
    Ok(())
}

/// The decimals of the mint an invoice is priced in.
///
/// Every token context keeps its mint an `UncheckedAccount`, so that an
/// unsupported mint is refused with the program's own error instead of Anchor's
/// generic deserialization failure — which means the transfer CPIs cannot read
/// decimals from a checked account, and this is the one place they do. Both
/// token programs keep the base mint layout, `SPL_MINT_LEN` already pins the
/// length, and `MINT_DECIMALS_OFFSET` is where `decimals` sits in it.
pub fn mint_decimals(mint: &UncheckedAccount) -> Result<u8> {
    let data = mint.try_borrow_data()?;
    data.get(MINT_DECIMALS_OFFSET)
        .copied()
        .ok_or_else(|| error!(PaymentError::Token2022Unsupported))
}

/// The flat a marker's rent implies at a SOL price in micros, in the mint's base
/// units.
///
/// `receipt_rent` lamports are `receipt_rent / 1e9` SOL, which at
/// `price_micros` a unit is `receipt_rent * price_micros / 1e9` whole units, and
/// a six-decimal stablecoin is a million base units a unit — so the two
/// millions cancel and nothing else has to be assumed about the mint. That last
/// step is why only the named, six-decimal currencies carry a flat.
pub fn flat_for_rent(receipt_rent: u64, price_micros: u64) -> Result<u64> {
    let product = (receipt_rent as u128)
        .checked_mul(price_micros as u128)
        .ok_or(PaymentError::FeeOverflow)?;
    let flat = product / (MICROS * 1_000);
    u64::try_from(flat).map_err(|_| PaymentError::FeeOverflow.into())
}

/// A price `refresh_fee_price` will accept: positive, and under a ceiling that
/// only a unit mistake would cross.
pub fn validate_price(price_micros: u64) -> Result<()> {
    require!(
        price_micros > 0 && price_micros <= MAX_SOL_PRICE_MICROS,
        PaymentError::InvalidFeePrice
    );
    Ok(())
}

/// Bounds a permissionless refresh against the admin's anchor price.
///
/// The band is measured from `anchor_micros`, the price the admin wrote, and
/// never from the flat the last refresh left behind. That is the whole point: a
/// band relative to the current flat can be walked, one call per cooldown, so a
/// caller could ratchet the flats until the record fee no longer matched the rent
/// the treasury refunds. Anchored, every accepted price lies in the same window
/// and the flats it writes stay within `PRICE_ANCHOR_BAND_BPS` of the admin's
/// price however many refreshes run.
///
/// An anchor of zero means the admin has not priced this deployment yet, and
/// nothing may be written against it.
pub fn ensure_anchor_band(anchor_micros: u64, price_micros: u64) -> Result<()> {
    require!(anchor_micros > 0, PaymentError::InvalidFeePrice);
    let anchor = anchor_micros as u128;
    let price = price_micros as u128;
    let low = anchor * (10_000 - PRICE_ANCHOR_BAND_BPS as u128) / 10_000;
    let high = anchor * (10_000 + PRICE_ANCHOR_BAND_BPS as u128) / 10_000;
    require!(price >= low && price <= high, PaymentError::InvalidFeePrice);
    Ok(())
}

/// Policy validation shared by `initialize_config`, `update_config` and
/// `migrate_config`.
pub fn validate_fee_policy(policy: &FeePolicy) -> Result<()> {
    require!(
        policy.fee_bps_below <= 10_000 && policy.fee_bps_above <= 10_000,
        PaymentError::InvalidFeeBps
    );
    // The network fee is a cost we reimburse, so a policy cannot name one the
    // pool would never recover. Both bounds are unit-mistake guards: a real fee
    // is a signature's worth of lamports, not a fraction of a SOL.
    require!(
        policy.network_fee_lamports <= MAX_NETWORK_FEE_LAMPORTS
            && policy.network_fee_usdc_base_units <= MAX_NETWORK_FEE_BASE_UNITS
            && policy.network_fee_eurc_base_units <= MAX_NETWORK_FEE_BASE_UNITS,
        PaymentError::InvalidNetworkFee
    );
    Ok(())
}

/// The session float's floor, shared by the same three instructions.
pub fn validate_session_float(session_fee_float: u64) -> Result<()> {
    require!(
        session_fee_float >= MIN_SESSION_FEE_FLOAT,
        PaymentError::InvalidSessionFloat
    );
    Ok(())
}

/// Refuses to settle unless the treasury can fund the records it is about to
/// refund and keep its own rent-exempt minimum, so a settle never leaves the
/// treasury short.
///
/// Only the token paths call this since revision 23: a token settlement cannot
/// charge its non-signing seller lamports, so the treasury still funds the
/// marker's rent there. The SOL paths pay it out of the seller's proceeds and
/// ask the treasury for nothing.
pub fn ensure_treasury_funded(treasury: &AccountInfo, receipt_rent: u64) -> Result<()> {
    let floor = Rent::get()?.minimum_balance(0);
    require!(
        treasury.lamports() >= receipt_rent.saturating_add(floor),
        PaymentError::TreasuryUnderfunded
    );
    Ok(())
}

/// Moves lamports out of the treasury. The treasury is a system account, so the
/// program cannot subtract its lamports directly; it signs a system transfer
/// with the seed it was derived from instead.
pub fn pay_from_treasury<'info>(
    treasury: &AccountInfo<'info>,
    destination: &AccountInfo<'info>,
    amount: u64,
    bump: u8,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    anchor_lang::system_program::transfer(
        CpiContext::new_with_signer(
            anchor_lang::system_program::ID,
            anchor_lang::system_program::Transfer {
                from: treasury.clone(),
                to: destination.clone(),
            },
            &[&[TREASURY_SEED, &[bump][..]]],
        ),
        amount,
    )
}
