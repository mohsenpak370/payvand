//! Provisioning the treasury's token account for a mint.
//!
//! A one-time admin step per mint: without it, every invoice in that mint fails
//! to settle, because the fee has nowhere to land.

use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
// Interface types and CPIs, not `Account<TokenAccount>`: see the module note
// in `token/mod.rs` on why that makes `Token2022Unsupported` the error a
// Token-2022 caller sees.
use anchor_spl::token_interface::{TokenAccount, TokenInterface};

use crate::constants::*;
use crate::errors::PaymentError;
use crate::state::Config;

/// Opens the treasury's token account for a mint, paid by the admin.
///
/// `settle_token_payment` sends the fee into `treasury_token_account`, so a mint
/// the treasury has no account for makes every invoice in it unpurchasable — a
/// settle that fails on the buyer's signature with `AccountNotInitialized`.
/// The treasury is a PDA: nobody can create its associated account as a wallet
/// owner would, and only this program can sign for it. This is that one call.
///
/// Idempotent on purpose, so provisioning a mint twice is a no-op and the step
/// can be repeated safely by a deploy script.
#[derive(Accounts)]
pub struct OpenTreasuryTokenAccount<'info> {
    /// Pays the account's rent. The treasury's own lamports are the pool the
    /// fee is measured against, so they are not spent on accounts.
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump, has_one = admin)]
    pub config: Account<'info, Config>,

    #[account(seeds = [TREASURY_SEED], bump = config.treasury_bump)]
    pub treasury: SystemAccount<'info>,

    /// CHECK: The mint to open the treasury's account for. Unchecked only to
    /// refuse a Token-2022 mint by name; the ATA program still proves it is a
    /// mint when the account is created.
    #[account(
        constraint = mint.owner == &anchor_spl::token::ID
            && mint.data_len() == SPL_MINT_LEN
            @ PaymentError::Token2022Unsupported
    )]
    pub mint: UncheckedAccount<'info>,

    /// The fee's destination in this mint: the treasury's own account, created
    /// here when it is missing and left alone when it is not.
    #[account(
        init_if_needed,
        payer = admin,
        associated_token::mint = mint,
        associated_token::authority = treasury
    )]
    pub treasury_token_account: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

/// The account checks are the instruction: `init_if_needed` opens the
/// treasury's own account for the mint, paid by the admin, and Anchor proves it
/// is exactly that account (the ATA for this mint and the treasury PDA) before
/// the handler runs. Nothing is left to do, and a second call changes nothing.
pub fn open_treasury_token_account_handler(_ctx: Context<OpenTreasuryTokenAccount>) -> Result<()> {
    Ok(())
}
