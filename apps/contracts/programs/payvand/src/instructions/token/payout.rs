//! Moving a settled invoice's tokens: the price less the fee to the seller, and
//! the fee to the treasury's own account for the mint.
//!
//! Exactly `amount` leaves the source. On the wallet path the buyer signs for
//! its own token account; on the allowance path the budget PDA signs for its
//! vault, which is the only difference between the two, so it is the one thing
//! the caller passes in.

use anchor_lang::prelude::*;
// The interface CPI, not the SPL Token one: `transfer_checked` is one
// instruction on either token program, so the mint policy alone decides which
// invoices settle. See the module note in `token/mod.rs`.
use anchor_spl::token_interface::{self, TransferChecked as TokenTransferChecked};

/// The accounts both transfers share.
pub(crate) struct TokenPayout<'info> {
    pub(crate) token_program: Pubkey,
    pub(crate) from: AccountInfo<'info>,
    pub(crate) mint: AccountInfo<'info>,
    pub(crate) seller_token_account: AccountInfo<'info>,
    pub(crate) treasury_token_account: AccountInfo<'info>,
    pub(crate) authority: AccountInfo<'info>,
}

impl<'info> TokenPayout<'info> {
    /// Sends `amount - fee` to the seller and `fee` (when there is one) to the
    /// treasury. `signer_seeds` is empty when `authority` signed the
    /// transaction itself, and the budget PDA's seeds when it is the vault's
    /// owner.
    pub(crate) fn pay(
        &self,
        amount: u64,
        fee: u64,
        decimals: u8,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()> {
        self.transfer(
            &self.seller_token_account,
            amount - fee,
            decimals,
            signer_seeds,
        )?;
        if fee > 0 {
            self.transfer(&self.treasury_token_account, fee, decimals, signer_seeds)?;
        }
        Ok(())
    }

    fn transfer(
        &self,
        to: &AccountInfo<'info>,
        amount: u64,
        decimals: u8,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()> {
        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                self.token_program,
                TokenTransferChecked {
                    from: self.from.clone(),
                    mint: self.mint.clone(),
                    to: to.clone(),
                    authority: self.authority.clone(),
                },
                signer_seeds,
            ),
            amount,
            decimals,
        )
    }
}
