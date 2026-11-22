//! A shop's rate: the multiplier the shop's own seller sets on `Config`'s fee
//! policy.
//!
//! The account is deliberately not created with the shop row. A shop is an API
//! row and its key is generated when the row is made, so the on-chain policy can
//! be written by the seller whenever they choose — and an invoice whose policy
//! was never written settles at `Config`'s rate, because the settle paths take
//! this account as optional.

use anchor_lang::prelude::*;

use crate::constants::{SHOP_POLICY_SEED, SHOP_POLICY_SPACE};
use crate::helpers::validate_shop_multiplier;
use crate::state::ShopPolicy;

#[derive(Accounts)]
pub struct SetShopPolicy<'info> {
    /// The shop's seller: the only key that may write this shop's rate, and the
    /// one that pays the account's rent. It is also the first seed, so no
    /// caller can write a policy attributed to a wallet that did not sign.
    #[account(mut)]
    pub seller: Signer<'info>,

    /// CHECK: The key the API generates and keeps beside its `shops` row. It is
    /// only ever a PDA seed — the program never reads it — so it needs no data
    /// and no owner, and it must not be a signer (a shop is not a wallet).
    pub shop_key: UncheckedAccount<'info>,

    /// Created on the first write and rewritten on every later one, so a seller
    /// changing a rate does not need a second instruction or a close.
    #[account(
        init_if_needed,
        payer = seller,
        space = SHOP_POLICY_SPACE,
        seeds = [SHOP_POLICY_SEED, seller.key().as_ref(), shop_key.key().as_ref()],
        bump
    )]
    pub shop_policy: Account<'info, ShopPolicy>,

    pub system_program: Program<'info, System>,
}

/// Writes `multiplier_bps` as the shop's rate, in basis points on `Config`'s two
/// rates.
///
/// `10_000` is `Config` unchanged and `0` charges no percentage at all — the
/// marker's flat still applies on a token invoice, so a free shop cannot make the
/// treasury fund the record. Anything above `10_000` is refused here with
/// `ShopFeeTooHigh`, which is what keeps every settlement inside the band without
/// a `Config` field to read: a shop may discount itself, never surcharge a buyer.
pub fn set_shop_policy_handler(ctx: Context<SetShopPolicy>, multiplier_bps: u16) -> Result<()> {
    validate_shop_multiplier(multiplier_bps)?;

    let policy = &mut ctx.accounts.shop_policy;
    policy.seller = ctx.accounts.seller.key();
    policy.shop_key = ctx.accounts.shop_key.key();
    policy.multiplier_bps = multiplier_bps;
    policy.bump = ctx.bumps.shop_policy;

    msg!(
        "shop policy set: {} bps for shop {} of {}",
        multiplier_bps,
        policy.shop_key,
        policy.seller
    );
    Ok(())
}
