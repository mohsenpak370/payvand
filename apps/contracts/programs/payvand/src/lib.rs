use anchor_lang::prelude::*;
use anchor_lang::Lamports;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer as TokenTransfer};

declare_id!("4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J");

/// Seed prefix for every payment PDA.
pub const PAYMENT_SEED: &[u8] = b"payment";
/// Maximum number of bytes stored in `Payment::memo`.
pub const PAYMENT_MAX_MEMO: usize = 96;

/// Seed prefix for the buyer's monthly budget PDA.
pub const BUDGET_SEED: &[u8] = b"budget";
/// `8` (discriminator) + `32 + 32 + 8 + 8 + 8 + 1` (buyer, session key,
/// monthly limit, spent, month index, bump).
pub const BUDGET_SPACE: usize = 8 + 32 + 32 + 8 + 8 + 8 + 1;
/// Lamports parked on the session key so its silent transactions can pay
/// their own network fees (~2000 fees at 5000 lamports each). Funded by
/// `open_budget` and topped back up to this level on every later open.
pub const SESSION_FEE_FLOAT: u64 = 10_000_000;

/// `8` (discriminator) + `32 + 32 + 8 + 4 + 96 + 1 + 32 + 8 + 8 + 1`.
pub const PAYMENT_SPACE: usize = 8 + 32 + 32 + 8 + 4 + PAYMENT_MAX_MEMO + 1 + 32 + 8 + 8 + 1;

/// Seed prefix for an invoice paid in an SPL token. A separate prefix from
/// `PAYMENT_SEED` on purpose: the account carries a mint, so its layout
/// differs, and keeping it apart leaves every SOL invoice already on chain
/// readable by the old code path.
pub const TOKEN_PAYMENT_SEED: &[u8] = b"token-payment";
/// `8` + `32 (seller) + 32 (reference) + 32 (mint) + 8 (amount) + 4 + 96
/// (memo) + 1 (state) + 32 (payer) + 8 + 8 + 1 (bump)`.
pub const TOKEN_PAYMENT_SPACE: usize = 8 + 32 + 32 + 32 + 8 + 4 + PAYMENT_MAX_MEMO + 1 + 32 + 8 + 8 + 1;

/// Seed prefix for a buyer's monthly token budget — one per buyer per mint.
pub const TOKEN_BUDGET_SEED: &[u8] = b"token-budget";
/// `8` + `32 (buyer) + 32 (session key) + 32 (mint) + 8 + 8 + 8 + 1 (bump)`.
pub const TOKEN_BUDGET_SPACE: usize = 8 + 32 + 32 + 32 + 8 + 8 + 8 + 1;
/// Lamports parked on a token budget so it can refund the invoice rent its
/// session key advances, without eating into the budget account's own rent.
pub const TOKEN_BUDGET_FEE_FLOAT: u64 = 10_000_000;

#[program]
pub mod payvand {
    use super::*;

    /// Buyer pays an invoice. The invoice account is created (and funded by
    /// the buyer) inside this same transaction, so the seller never has to
    /// sign anything to open a payment code — the QR is just an address plus
    /// its details.
    ///
    /// `amount` is part of the PDA seeds, which is what keeps this safe: an
    /// invoice can only ever be settled at the exact amount that was encoded
    /// into the QR code, because any other amount derives a different account
    /// address than the one the seller is watching.
    pub fn settle_payment(ctx: Context<SettlePayment>, amount: u64, memo: String) -> Result<()> {
        require!(amount > 0, PaymentError::InvalidAmount);
        require!(
            memo.as_bytes().len() <= PAYMENT_MAX_MEMO,
            PaymentError::MemoTooLong
        );
        // `Pubkey::default()` doubles as "not initialised yet"; it must never
        // be a real seller or the invoice could be re-opened after settling.
        require_keys_neq!(
            ctx.accounts.seller.key(),
            Pubkey::default(),
            PaymentError::InvalidSeller
        );

        let payment = &mut ctx.accounts.payment;

        if payment.seller == Pubkey::default() {
            // First touch: open the invoice and settle it in one go.
            payment.seller = ctx.accounts.seller.key();
            payment.reference = ctx.accounts.reference.key();
            payment.amount = amount;
            payment.memo = memo;
            payment.state = PaymentState::Pending;
            payment.payer = Pubkey::default();
            payment.created_at = Clock::get()?.unix_timestamp;
            payment.settled_at = 0;
            payment.bump = ctx.bumps.payment;
        } else {
            // The invoice already exists; the seeds above guarantee the seller,
            // reference and amount all match this exact invoice, so the only
            // thing left to check is that nobody paid twice.
            require!(
                payment.state == PaymentState::Pending,
                PaymentError::AlreadySettled
            );
        }

        // Lamports move from the payer straight to the seller.
        anchor_lang::system_program::transfer(
            CpiContext::new(
                anchor_lang::system_program::ID,
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.payer.to_account_info(),
                    to: ctx.accounts.seller.to_account_info(),
                },
            ),
            amount,
        )?;

        let payment = &mut ctx.accounts.payment;
        payment.state = PaymentState::Settled;
        payment.payer = ctx.accounts.payer.key();
        payment.settled_at = Clock::get()?.unix_timestamp;

        msg!(
            "payment settled: {} lamports from {}",
            amount,
            payment.payer
        );
        Ok(())
    }

    /// Seller reclaims the rent lamports locked in a settled invoice. This is
    /// optional housekeeping — it is the one signature a seller may ever make,
    /// and only when they want the rent back.
    pub fn close_payment(_ctx: Context<ClosePayment>, amount: u64) -> Result<()> {
        msg!("payment closed: {} lamports invoice", amount);
        Ok(())
    }

    /// Buyer funds this month's spending budget and registers a session key.
    /// This is the ONE wallet signature the whole feature needs: afterwards
    /// the session key — an app-generated keypair stored on the device — can
    /// settle invoices from the pot without touching the wallet again.
    pub fn open_budget(ctx: Context<OpenBudget>, monthly_limit: u64) -> Result<()> {
        require!(monthly_limit > 0, PaymentError::InvalidAmount);
        let session = ctx.accounts.session_key.key();
        require_keys_neq!(session, ctx.accounts.buyer.key(), PaymentError::InvalidSessionKey);
        require_keys_neq!(session, Pubkey::default(), PaymentError::InvalidSessionKey);

        // Top the session key's fee float back up (only when it is short).
        let float_gap = SESSION_FEE_FLOAT.saturating_sub(ctx.accounts.session_key.lamports());
        if float_gap > 0 {
            anchor_lang::system_program::transfer(
                CpiContext::new(
                    anchor_lang::system_program::ID,
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.buyer.to_account_info(),
                        to: ctx.accounts.session_key.to_account_info(),
                    },
                ),
                float_gap,
            )?;
        }

        // Top the pot up to the limit: a returning buyer pays only the
        // difference between the limit and what is left unspent.
        let rent_min = Rent::get()?.minimum_balance(BUDGET_SPACE);
        let pot = ctx.accounts.budget.to_account_info().lamports().saturating_sub(rent_min);
        let top_up = monthly_limit.saturating_sub(pot);
        if top_up > 0 {
            anchor_lang::system_program::transfer(
                CpiContext::new(
                    anchor_lang::system_program::ID,
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.buyer.to_account_info(),
                        to: ctx.accounts.budget.to_account_info(),
                    },
                ),
                top_up,
            )?;
        }

        let budget = &mut ctx.accounts.budget;
        budget.buyer = ctx.accounts.buyer.key();
        budget.session_key = session;
        budget.monthly_limit = monthly_limit;
        budget.spent = 0;
        budget.month_index = current_month_index()?;
        // close_budget validates `bump = budget.bump`; without this the
        // stored bump stays 0 and closing can never pass its seeds check.
        budget.bump = ctx.bumps.budget;
        Ok(())
    }

    /// Silent payment: the session key settles an invoice straight out of the
    /// buyer's budget — no wallet popup, because the only signature here is
    /// the local session key's. The pot pays the seller and refunds the
    /// invoice rent it just advanced from the session key's float.
    ///
    /// The monthly cap and the pot balance are both enforced here, and the
    /// amount still rides in the payment PDA's seeds, so a QR code can only
    /// ever be settled for exactly the price it shows — the same guarantee
    /// `settle_payment` gives.
    pub fn pay_from_budget(ctx: Context<PayFromBudget>, amount: u64, memo: String) -> Result<()> {
        require!(amount > 0, PaymentError::InvalidAmount);
        require!(
            memo.as_bytes().len() <= PAYMENT_MAX_MEMO,
            PaymentError::MemoTooLong
        );
        require_keys_neq!(
            ctx.accounts.seller.key(),
            Pubkey::default(),
            PaymentError::InvalidSeller
        );

        // Only the session key registered at open time may spend, and the
        // budget is addressed through its own seeds (`[budget, buyer]`), so
        // no other wallet's budget can ever be named here.
        require_keys_eq!(
            ctx.accounts.session_key.key(),
            ctx.accounts.budget.session_key,
            PaymentError::InvalidSessionKey
        );

        // Monthly cap, lazily reset on the calendar-month boundary.
        let now = current_month_index()?;
        let (monthly_limit, mut spent, month_index) = {
            let b = &ctx.accounts.budget;
            (b.monthly_limit, b.spent, b.month_index)
        };
        if month_index != now {
            spent = 0;
        }
        require!(
            spent.saturating_add(amount) <= monthly_limit,
            PaymentError::BudgetExhausted
        );

        // The pot must cover the price plus the invoice rent that
        // `init_if_needed` just advanced from the session key's float.
        let invoice_rent = Rent::get()?.minimum_balance(PAYMENT_SPACE);
        let budget_rent = Rent::get()?.minimum_balance(BUDGET_SPACE);
        let pot = ctx
            .accounts
            .budget
            .to_account_info()
            .lamports()
            .saturating_sub(budget_rent);
        require!(
            pot >= amount.saturating_add(invoice_rent),
            PaymentError::InsufficientBudget
        );

        // An invoice that already exists at this address is bound to this
        // seller, reference and amount by its seeds — the only question left
        // is whether someone already paid it.
        if ctx.accounts.payment.seller != Pubkey::default() {
            require!(
                ctx.accounts.payment.state == PaymentState::Pending,
                PaymentError::AlreadySettled
            );
        }

        // Move the money: pot → seller (the price) and pot → session key
        // (refund of the invoice rent it advanced).
        let total = amount.saturating_add(invoice_rent);
        ctx.accounts.budget.to_account_info().sub_lamports(total)?;
        ctx.accounts.seller.to_account_info().add_lamports(amount)?;
        ctx.accounts
            .session_key
            .to_account_info()
            .add_lamports(invoice_rent)?;

        // Record the invoice (first touch) and mark it settled.
        {
            let payment = &mut ctx.accounts.payment;
            if payment.seller == Pubkey::default() {
                payment.seller = ctx.accounts.seller.key();
                payment.reference = ctx.accounts.reference.key();
                payment.amount = amount;
                payment.memo = memo;
                payment.state = PaymentState::Pending;
                payment.payer = Pubkey::default();
                payment.created_at = Clock::get()?.unix_timestamp;
                payment.settled_at = 0;
                payment.bump = ctx.bumps.payment;
            }
            payment.state = PaymentState::Settled;
            payment.payer = ctx.accounts.session_key.key();
            payment.settled_at = Clock::get()?.unix_timestamp;
        }
        {
            let budget = &mut ctx.accounts.budget;
            budget.spent = spent.saturating_add(amount);
            budget.month_index = now;
        }

        msg!(
            "payment settled from budget: {} lamports, session {}",
            amount,
            ctx.accounts.session_key.key()
        );
        Ok(())
    }

    /// Reclaim whatever is left in the budget and retire the session key.
    /// One wallet signature — the buyer gets every lamport back.
    pub fn close_budget(_ctx: Context<CloseBudget>) -> Result<()> {
        msg!("budget closed; remaining lamports returned to the buyer");
        Ok(())
    }

    // ---- SPL token payments --------------------------------------------

    /// Buyer pays a token-denominated invoice straight from their wallet.
    ///
    /// Same guarantee as `settle_payment`: the amount rides in the PDA seeds,
    /// so an invoice can only ever be settled for the exact price its QR
    /// shows. The mint is in the seeds too, so a USDC invoice and a SOL
    /// invoice that happen to share a reference and a price are different
    /// accounts and cannot pay for each other.
    pub fn settle_token_payment(
        ctx: Context<SettleTokenPayment>,
        amount: u64,
        memo: String,
    ) -> Result<()> {
        require!(amount > 0, PaymentError::InvalidAmount);
        require!(
            memo.as_bytes().len() <= PAYMENT_MAX_MEMO,
            PaymentError::MemoTooLong
        );
        require_keys_neq!(
            ctx.accounts.seller.key(),
            Pubkey::default(),
            PaymentError::InvalidSeller
        );

        open_token_invoice(
            &mut ctx.accounts.payment,
            ctx.accounts.seller.key(),
            ctx.accounts.reference.key(),
            ctx.accounts.mint.key(),
            amount,
            memo,
            ctx.bumps.payment,
        )?;

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                TokenTransfer {
                    from: ctx.accounts.payer_token_account.to_account_info(),
                    to: ctx.accounts.seller_token_account.to_account_info(),
                    authority: ctx.accounts.payer.to_account_info(),
                },
            ),
            amount,
        )?;

        settle_token_invoice(&mut ctx.accounts.payment, ctx.accounts.payer.key())?;

        msg!(
            "token payment settled: {} of {} from {}",
            amount,
            ctx.accounts.mint.key(),
            ctx.accounts.payer.key()
        );
        Ok(())
    }

    /// Seller reclaims the rent of a settled token invoice. Mirrors
    /// `close_payment`, and is the only signature a seller ever makes.
    pub fn close_token_payment(_ctx: Context<CloseTokenPayment>, amount: u64) -> Result<()> {
        msg!("token payment closed: {} base units invoice", amount);
        Ok(())
    }

    /// Buyer opens (or tops up) this month's budget for one token and
    /// registers a session key for it. One wallet signature per token; after
    /// that the session key settles token invoices from the vault without
    /// touching the wallet again.
    pub fn open_token_budget(ctx: Context<OpenTokenBudget>, monthly_limit: u64) -> Result<()> {
        require!(monthly_limit > 0, PaymentError::InvalidAmount);
        let session = ctx.accounts.session_key.key();
        require_keys_neq!(
            session,
            ctx.accounts.buyer.key(),
            PaymentError::InvalidSessionKey
        );
        require_keys_neq!(
            session,
            Pubkey::default(),
            PaymentError::InvalidSessionKey
        );

        // The session key pays the network fee of every silent payment, so it
        // keeps a float just like it does for SOL.
        let float_gap = SESSION_FEE_FLOAT.saturating_sub(ctx.accounts.session_key.lamports());
        if float_gap > 0 {
            anchor_lang::system_program::transfer(
                CpiContext::new(
                    anchor_lang::system_program::ID,
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.buyer.to_account_info(),
                        to: ctx.accounts.session_key.to_account_info(),
                    },
                ),
                float_gap,
            )?;
        }

        // The budget account itself refunds the invoice rent its session key
        // advances, so it holds a float on top of its own rent.
        let budget_rent = Rent::get()?.minimum_balance(TOKEN_BUDGET_SPACE);
        let wanted = budget_rent.saturating_add(TOKEN_BUDGET_FEE_FLOAT);
        let held = ctx.accounts.budget.to_account_info().lamports();
        if wanted > held {
            anchor_lang::system_program::transfer(
                CpiContext::new(
                    anchor_lang::system_program::ID,
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.buyer.to_account_info(),
                        to: ctx.accounts.budget.to_account_info(),
                    },
                ),
                wanted - held,
            )?;
        }

        // Top the vault up to the limit: a returning buyer moves only the
        // difference between the limit and what the vault still holds.
        let held_tokens = ctx.accounts.vault.amount;
        let top_up = monthly_limit.saturating_sub(held_tokens);
        if top_up > 0 {
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.key(),
                    TokenTransfer {
                        from: ctx.accounts.buyer_token_account.to_account_info(),
                        to: ctx.accounts.vault.to_account_info(),
                        authority: ctx.accounts.buyer.to_account_info(),
                    },
                ),
                top_up,
            )?;
        }

        let budget = &mut ctx.accounts.budget;
        budget.buyer = ctx.accounts.buyer.key();
        budget.session_key = session;
        budget.mint = ctx.accounts.mint.key();
        budget.monthly_limit = monthly_limit;
        budget.spent = 0;
        budget.month_index = current_month_index()?;
        budget.bump = ctx.bumps.budget;
        Ok(())
    }

    /// Silent token payment: the session key settles a token invoice out of
    /// the buyer's vault, with no wallet popup. The monthly cap, the vault
    /// balance and the amount in the invoice seeds are all enforced here, and
    /// only the session key registered at open time may spend.
    pub fn pay_from_token_budget(
        ctx: Context<PayFromTokenBudget>,
        amount: u64,
        memo: String,
    ) -> Result<()> {
        require!(amount > 0, PaymentError::InvalidAmount);
        require!(
            memo.as_bytes().len() <= PAYMENT_MAX_MEMO,
            PaymentError::MemoTooLong
        );
        require_keys_neq!(
            ctx.accounts.seller.key(),
            Pubkey::default(),
            PaymentError::InvalidSeller
        );
        require_keys_eq!(
            ctx.accounts.session_key.key(),
            ctx.accounts.budget.session_key,
            PaymentError::InvalidSessionKey
        );
        require_keys_eq!(
            ctx.accounts.mint.key(),
            ctx.accounts.budget.mint,
            PaymentError::MintMismatch
        );

        let now = current_month_index()?;
        let (monthly_limit, mut spent, month_index) = {
            let b = &ctx.accounts.budget;
            (b.monthly_limit, b.spent, b.month_index)
        };
        if month_index != now {
            spent = 0;
        }
        require!(
            spent.saturating_add(amount) <= monthly_limit,
            PaymentError::BudgetExhausted
        );
        require!(
            ctx.accounts.vault.amount >= amount,
            PaymentError::InsufficientBudget
        );

        open_token_invoice(
            &mut ctx.accounts.payment,
            ctx.accounts.seller.key(),
            ctx.accounts.reference.key(),
            ctx.accounts.mint.key(),
            amount,
            memo,
            ctx.bumps.payment,
        )?;

        // Vault → seller, signed by the budget PDA.
        let buyer = ctx.accounts.budget.buyer;
        let mint = ctx.accounts.budget.mint;
        let bump = ctx.accounts.budget.bump;
        let seeds: &[&[u8]] = &[
            TOKEN_BUDGET_SEED,
            buyer.as_ref(),
            mint.as_ref(),
            &[bump],
        ];
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.key(),
                TokenTransfer {
                    from: ctx.accounts.vault.to_account_info(),
                    to: ctx.accounts.seller_token_account.to_account_info(),
                    authority: ctx.accounts.budget.to_account_info(),
                },
                &[seeds],
            ),
            amount,
        )?;

        settle_token_invoice(
            &mut ctx.accounts.payment,
            ctx.accounts.session_key.key(),
        )?;

        // Refund the invoice rent the session key just advanced, out of the
        // float the budget keeps for exactly this.
        let invoice_rent = Rent::get()?.minimum_balance(TOKEN_PAYMENT_SPACE);
        let budget_rent = Rent::get()?.minimum_balance(TOKEN_BUDGET_SPACE);
        let spare = ctx
            .accounts
            .budget
            .to_account_info()
            .lamports()
            .saturating_sub(budget_rent);
        let refund = invoice_rent.min(spare);
        if refund > 0 {
            ctx.accounts.budget.to_account_info().sub_lamports(refund)?;
            ctx.accounts
                .session_key
                .to_account_info()
                .add_lamports(refund)?;
        }

        let budget = &mut ctx.accounts.budget;
        budget.spent = spent.saturating_add(amount);
        budget.month_index = now;

        msg!(
            "token payment settled from budget: {} of {}, session {}",
            amount,
            mint,
            ctx.accounts.session_key.key()
        );
        Ok(())
    }

    /// Drain the token vault back to the buyer and retire the budget. One
    /// wallet signature; every token and lamport comes back.
    pub fn close_token_budget(ctx: Context<CloseTokenBudget>) -> Result<()> {
        let buyer = ctx.accounts.budget.buyer;
        let mint = ctx.accounts.budget.mint;
        let bump = ctx.accounts.budget.bump;
        let seeds: &[&[u8]] = &[
            TOKEN_BUDGET_SEED,
            buyer.as_ref(),
            mint.as_ref(),
            &[bump],
        ];

        // Whatever is left in the vault goes back to the buyer's own account.
        let remaining = ctx.accounts.vault.amount;
        if remaining > 0 {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.key(),
                    TokenTransfer {
                        from: ctx.accounts.vault.to_account_info(),
                        to: ctx.accounts.buyer_token_account.to_account_info(),
                        authority: ctx.accounts.budget.to_account_info(),
                    },
                    &[seeds],
                ),
                remaining,
            )?;
        }

        // Close the vault itself and send its rent to the buyer.
        token::close_account(CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            token::CloseAccount {
                account: ctx.accounts.vault.to_account_info(),
                destination: ctx.accounts.buyer.to_account_info(),
                authority: ctx.accounts.budget.to_account_info(),
            },
            &[seeds],
        ))?;

        msg!("token budget closed; vault returned to the buyer");
        Ok(())
    }
}

/// First touch of a token invoice: fill it in. An invoice that already exists
/// at this address is bound to this seller, reference, mint and amount by its
/// seeds, so the only question left is whether someone already paid it.
fn open_token_invoice<'info>(
    payment: &mut Account<'info, TokenPayment>,
    seller: Pubkey,
    reference: Pubkey,
    mint: Pubkey,
    amount: u64,
    memo: String,
    bump: u8,
) -> Result<()> {
    if payment.seller == Pubkey::default() {
        payment.seller = seller;
        payment.reference = reference;
        payment.mint = mint;
        payment.amount = amount;
        payment.memo = memo;
        payment.state = PaymentState::Pending;
        payment.payer = Pubkey::default();
        payment.created_at = Clock::get()?.unix_timestamp;
        payment.settled_at = 0;
        payment.bump = bump;
        Ok(())
    } else {
        require!(
            payment.state == PaymentState::Pending,
            PaymentError::AlreadySettled
        );
        Ok(())
    }
}

/// Mark a token invoice paid. Callers have already moved the tokens.
fn settle_token_invoice<'info>(
    payment: &mut Account<'info, TokenPayment>,
    payer: Pubkey,
) -> Result<()> {
    payment.state = PaymentState::Settled;
    payment.payer = payer;
    payment.settled_at = Clock::get()?.unix_timestamp;
    Ok(())
}

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct SettlePayment<'info> {
    /// Pays for the invoice account (rent) and the transfer itself.
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init_if_needed,
        payer = payer,
        space = PAYMENT_SPACE,
        seeds = [
            PAYMENT_SEED,
            seller.key().as_ref(),
            reference.key().as_ref(),
            amount.to_le_bytes().as_ref()
        ],
        bump
    )]
    pub payment: Account<'info, Payment>,

    /// CHECK: The address the QR code names as the payee; it is the only
    /// account allowed to receive the lamports.
    #[account(mut)]
    pub seller: UncheckedAccount<'info>,

    /// CHECK: Arbitrary 32-byte value chosen when the QR was made. It is only
    /// used as a PDA seed and as an address that watchers can look up.
    pub reference: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

/// The buyer's monthly spending pot: lamports held by the program plus the
/// rules that make the session key's signatures safe. Seeded by the buyer's
/// wallet, so there is exactly one per buyer.
#[account]
pub struct Budget {
    /// Main wallet — funds, refills and closes the pot. The only wallet
    /// that ever signs for this feature.
    pub buyer: Pubkey,
    /// App-generated key allowed to spend from the pot without the wallet.
    pub session_key: Pubkey,
    /// Ceiling for one calendar month, in lamports.
    pub monthly_limit: u64,
    /// Spent so far in `month_index`.
    pub spent: u64,
    /// `year * 12 + month` that `spent` belongs to; a new month resets it.
    pub month_index: u64,
    pub bump: u8,
}

#[derive(Accounts)]
pub struct OpenBudget<'info> {
    /// Main wallet: the one signature the buyer ever makes.
    #[account(mut)]
    pub buyer: Signer<'info>,

    #[account(
        init_if_needed,
        payer = buyer,
        space = BUDGET_SPACE,
        seeds = [BUDGET_SEED, buyer.key().as_ref()],
        bump
    )]
    pub budget: Account<'info, Budget>,

    /// CHECK: App-generated session key. Receives the fee float here, and is
    /// the only key allowed to spend from the budget afterwards.
    #[account(mut)]
    pub session_key: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct PayFromBudget<'info> {
    /// Session key: fee payer and invoice-rent payer (refunded from the
    /// budget in this same transaction). Never the buyer's wallet.
    #[account(mut)]
    pub session_key: Signer<'info>,

    #[account(
        mut,
        seeds = [BUDGET_SEED, budget.buyer.as_ref()],
        bump = budget.bump
    )]
    pub budget: Account<'info, Budget>,

    #[account(
        init_if_needed,
        payer = session_key,
        space = PAYMENT_SPACE,
        seeds = [
            PAYMENT_SEED,
            seller.key().as_ref(),
            reference.key().as_ref(),
            amount.to_le_bytes().as_ref()
        ],
        bump
    )]
    pub payment: Account<'info, Payment>,

    /// CHECK: The address the QR code names as the payee; the only account
    /// allowed to receive the lamports.
    #[account(mut)]
    pub seller: UncheckedAccount<'info>,

    /// CHECK: Arbitrary 32-byte value chosen when the QR was made; a PDA
    /// seed and an address watchers can look up.
    pub reference: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CloseBudget<'info> {
    #[account(
        mut,
        close = buyer,
        seeds = [BUDGET_SEED, buyer.key().as_ref()],
        bump = budget.bump
    )]
    pub budget: Account<'info, Budget>,

    #[account(mut)]
    pub buyer: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct ClosePayment<'info> {
    #[account(
        mut,
        seeds = [
            PAYMENT_SEED,
            payment.seller.as_ref(),
            payment.reference.as_ref(),
            amount.to_le_bytes().as_ref()
        ],
        bump = payment.bump,
        close = seller
    )]
    pub payment: Account<'info, Payment>,

    #[account(mut)]
    pub seller: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct SettleTokenPayment<'info> {
    /// Pays the invoice rent, the seller's token account rent if it does not
    /// exist yet, and the fee. The tokens themselves come out of
    /// `payer_token_account`.
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init_if_needed,
        payer = payer,
        space = TOKEN_PAYMENT_SPACE,
        seeds = [
            TOKEN_PAYMENT_SEED,
            seller.key().as_ref(),
            reference.key().as_ref(),
            mint.key().as_ref(),
            amount.to_le_bytes().as_ref()
        ],
        bump
    )]
    pub payment: Account<'info, TokenPayment>,

    /// CHECK: The address the QR code names as the payee. It receives only
    /// through its own associated token account, checked below.
    pub seller: UncheckedAccount<'info>,

    /// Created on the fly when the seller has never held this token, so a
    /// buyer is never blocked by the recipient's account setup.
    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = seller
    )]
    pub seller_token_account: Account<'info, TokenAccount>,

    #[account(mut, token::mint = mint, token::authority = payer)]
    pub payer_token_account: Account<'info, TokenAccount>,

    pub mint: Account<'info, Mint>,

    /// CHECK: Arbitrary 32-byte value chosen when the QR was made; a PDA seed
    /// and an address watchers can look up.
    pub reference: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct CloseTokenPayment<'info> {
    #[account(
        mut,
        seeds = [
            TOKEN_PAYMENT_SEED,
            payment.seller.as_ref(),
            payment.reference.as_ref(),
            payment.mint.as_ref(),
            amount.to_le_bytes().as_ref()
        ],
        bump = payment.bump,
        close = seller
    )]
    pub payment: Account<'info, TokenPayment>,

    #[account(mut)]
    pub seller: Signer<'info>,
}

#[derive(Accounts)]
pub struct OpenTokenBudget<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,

    #[account(
        init_if_needed,
        payer = buyer,
        space = TOKEN_BUDGET_SPACE,
        seeds = [TOKEN_BUDGET_SEED, buyer.key().as_ref(), mint.key().as_ref()],
        bump
    )]
    pub budget: Account<'info, TokenBudget>,

    pub mint: Account<'info, Mint>,

    /// Where the budget's tokens come from — the buyer's own account.
    #[account(mut, token::mint = mint, token::authority = buyer)]
    pub buyer_token_account: Account<'info, TokenAccount>,

    /// The pot itself: a token account owned by the budget PDA, so only this
    /// program can ever move what is inside it.
    #[account(
        init_if_needed,
        payer = buyer,
        associated_token::mint = mint,
        associated_token::authority = budget
    )]
    pub vault: Account<'info, TokenAccount>,

    /// CHECK: App-generated session key. Receives the fee float here, and is
    /// the only key allowed to spend from this budget afterwards.
    #[account(mut)]
    pub session_key: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct PayFromTokenBudget<'info> {
    /// Session key: fee payer and invoice-rent payer (refunded from the
    /// budget's float in this same transaction). Never the buyer's wallet.
    #[account(mut)]
    pub session_key: Signer<'info>,

    #[account(
        mut,
        seeds = [TOKEN_BUDGET_SEED, budget.buyer.as_ref(), budget.mint.as_ref()],
        bump = budget.bump
    )]
    pub budget: Account<'info, TokenBudget>,

    #[account(
        mut,
        token::mint = mint,
        token::authority = budget
    )]
    pub vault: Account<'info, TokenAccount>,

    pub mint: Account<'info, Mint>,

    /// CHECK: The address the QR code names as the payee.
    pub seller: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = session_key,
        associated_token::mint = mint,
        associated_token::authority = seller
    )]
    pub seller_token_account: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = session_key,
        space = TOKEN_PAYMENT_SPACE,
        seeds = [
            TOKEN_PAYMENT_SEED,
            seller.key().as_ref(),
            reference.key().as_ref(),
            mint.key().as_ref(),
            amount.to_le_bytes().as_ref()
        ],
        bump
    )]
    pub payment: Account<'info, TokenPayment>,

    /// CHECK: Arbitrary 32-byte value chosen when the QR was made.
    pub reference: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CloseTokenBudget<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,

    #[account(
        mut,
        close = buyer,
        seeds = [TOKEN_BUDGET_SEED, buyer.key().as_ref(), budget.mint.as_ref()],
        bump = budget.bump
    )]
    pub budget: Account<'info, TokenBudget>,

    pub mint: Account<'info, Mint>,

    #[account(mut, token::mint = mint, token::authority = budget)]
    pub vault: Account<'info, TokenAccount>,

    #[account(mut, token::mint = mint, token::authority = buyer)]
    pub buyer_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[account]
#[derive(Debug)]
pub struct Payment {
    /// Who is being paid.
    pub seller: Pubkey,
    /// Free-form lookup key chosen when the invoice was opened.
    pub reference: Pubkey,
    /// Price in lamports.
    pub amount: u64,
    /// Human readable invoice note.
    pub memo: String,
    pub state: PaymentState,
    /// Set once the invoice has been settled.
    pub payer: Pubkey,
    pub created_at: i64,
    pub settled_at: i64,
    pub bump: u8,
}

/// An invoice paid in an SPL token. A separate account type from `Payment`
/// because it records the mint, and keeping it separate leaves every SOL
/// invoice already on chain readable.
#[account]
#[derive(Debug)]
pub struct TokenPayment {
    /// Who is being paid.
    pub seller: Pubkey,
    /// Free-form lookup key chosen when the invoice was opened.
    pub reference: Pubkey,
    /// The token this invoice is priced in.
    pub mint: Pubkey,
    /// Price in the mint's base units.
    pub amount: u64,
    /// Human readable invoice note.
    pub memo: String,
    pub state: PaymentState,
    /// Set once the invoice has been settled.
    pub payer: Pubkey,
    pub created_at: i64,
    pub settled_at: i64,
    pub bump: u8,
}

/// A buyer's monthly spending pot for one token: the vault holds the tokens
/// and this account holds the rules the session key's signatures obey.
#[account]
#[derive(Debug)]
pub struct TokenBudget {
    /// Main wallet — funds, refills and closes the pot.
    pub buyer: Pubkey,
    /// App-generated key allowed to spend from the vault without the wallet.
    pub session_key: Pubkey,
    /// The token this budget is denominated in.
    pub mint: Pubkey,
    /// Ceiling for one calendar month, in the mint's base units.
    pub monthly_limit: u64,
    /// Spent so far in `month_index`.
    pub spent: u64,
    /// `year * 12 + month` that `spent` belongs to.
    pub month_index: u64,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaymentState {
    Pending,
    Settled,
}

#[error_code]
pub enum PaymentError {
    #[msg("Payment amount must be greater than zero")]
    InvalidAmount,
    #[msg("Memo exceeds the maximum length")]
    MemoTooLong,
    #[msg("This payment has already been settled")]
    AlreadySettled,
    #[msg("The seller account does not match the payment record")]
    SellerMismatch,
    #[msg("The payment is not in the expected state")]
    NotPending,
    #[msg("The seller address is not valid")]
    InvalidSeller,
    #[msg("The monthly spending limit has been reached")]
    BudgetExhausted,
    #[msg("The budget does not hold enough SOL for this payment")]
    InsufficientBudget,
    #[msg("The session key is not the one registered for this budget")]
    InvalidSessionKey,
    #[msg("The token does not match this budget")]
    MintMismatch,
}

/// `year * 12 + month` (month 1-based) for the current clock, so a monthly
/// allowance resets on a real calendar-month boundary rather than every 30
/// days. Howard Hinnant's civil-from-days, without pulling in a date crate.
fn current_month_index() -> Result<u64> {
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