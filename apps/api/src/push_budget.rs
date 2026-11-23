//! Reading allowances straight off the cluster.
//!
//! A reminder is about on-chain state the API does not keep: whether a budget's
//! pot is short after its month turned over. The cron answers that with one
//! `getMultipleAccounts` call, which needs the account layouts and the addresses
//! they live at — the program's seeds and SPL's token and associated-token
//! programs.
//!
//! Everything here is pure: bytes in, numbers out. No database, no RPC, no
//! notification, which is why it is separate from the sending half in `push`.

use std::str::FromStr;

use solana_pubkey::Pubkey;

/// Mirrors `BUDGET_SPACE` in the program: 8 + 32 + 32 + 8 + 8 + 8 + 1.
const BUDGET_SPACE: usize = 97;
/// `monthly_limit`, `spent` and `month_index`, after the 8-byte discriminator
/// and the buyer and session-key addresses.
const BUDGET_LIMIT_OFFSET: usize = 72;
const BUDGET_MONTH_OFFSET: usize = 88;

/// Mirrors `TOKEN_BUDGET_SPACE` in the program: 8 + buyer 32 + session key 32 +
/// mint 32 + limit 8 + spent 8 + month index 8 + bump 1.
const TOKEN_BUDGET_SPACE: usize = 129;
/// `monthly_limit` and `month_index`, after the discriminator and the buyer,
/// session-key and mint addresses.
const TOKEN_BUDGET_LIMIT_OFFSET: usize = 104;
const TOKEN_BUDGET_MONTH_OFFSET: usize = 120;
/// The seed prefix the program derives a token budget from: `["token-budget",
/// buyer, mint]`.
const TOKEN_BUDGET_SEED: &[u8] = b"token-budget";
/// SPL's token program and associated-token program, which is all this needs of
/// them: the address a token budget's vault sits at.
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
/// An SPL token account's `amount`, after the mint and owner each take 32 bytes.
const TOKEN_AMOUNT_OFFSET: usize = 64;

/// A budget account's monthly figures and what its pot holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetFacts {
    pub limit: u64,
    pub month_index: i64,
    /// What is left to spend: lamports above a SOL budget's rent-exempt
    /// reserve, or a token budget vault's balance in the mint's base units.
    pub pot: u64,
}

/// The same figures for a token budget: the vault's token balance is the pot,
/// with no rent to subtract because the vault's rent is paid in SOL.
pub fn token_budget_facts(data: &[u8], pot: u64) -> Option<BudgetFacts> {
    if data.len() < TOKEN_BUDGET_SPACE {
        return None;
    }
    Some(BudgetFacts {
        limit: read_u64(data, TOKEN_BUDGET_LIMIT_OFFSET)?,
        month_index: read_u64(data, TOKEN_BUDGET_MONTH_OFFSET)? as i64,
        pot,
    })
}

/// An SPL token account's amount, or `None` when the bytes are not one. A
/// missing vault reads as zero in the caller, which is the truth: nothing was
/// ever topped up.
pub fn token_amount(data: &[u8]) -> Option<u64> {
    if data.len() < TOKEN_AMOUNT_OFFSET + 8 {
        return None;
    }
    read_u64(data, TOKEN_AMOUNT_OFFSET)
}

/// Reads the fields a nudge needs out of a budget account. `lamports` is what
/// the account holds now; the pot is that minus its rent, which is where
/// `open_budget` tops up to.
pub fn budget_facts(data: &[u8], lamports: u64) -> Option<BudgetFacts> {
    if data.len() < BUDGET_SPACE {
        return None;
    }
    Some(BudgetFacts {
        limit: read_u64(data, BUDGET_LIMIT_OFFSET)?,
        month_index: read_u64(data, BUDGET_MONTH_OFFSET)? as i64,
        pot: pot_lamports(lamports, BUDGET_SPACE),
    })
}

/// Lamports above the rent-exempt reserve the account was charged for. Derived
/// rather than read, so one small RPC call covers every budget at once instead
/// of a balance read per account.
pub fn pot_lamports(lamports: u64, data_len: usize) -> u64 {
    lamports.saturating_sub(rent_exempt_minimum(data_len))
}

/// `Rent::minimum_balance`: two years of `3_480` lamports per byte plus the
/// 128-byte account overhead. The same number the cluster reports.
pub fn rent_exempt_minimum(data_len: usize) -> u64 {
    ((128 + data_len) as u64) * 3_480 * 2
}

/// True when the month turned over under a pot that still holds less than the
/// limit — the one case a refill would change. A wallet that spent nothing keeps
/// its full pot and is left alone.
pub fn needs_nudge(facts: &BudgetFacts, now: i64) -> bool {
    facts.limit > 0 && facts.month_index != now && facts.pot < facts.limit
}

/// `year * 12 + month`, the program's `current_month_index`, from unix seconds.
/// The UI mirrors this in `budgetMath.ts`; the two have to agree on when a month
/// starts, which is the calendar month in UTC.
pub fn current_month_index(unix_seconds: i64) -> i64 {
    let days = unix_seconds.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    year * 12 + month
}

fn read_u64(data: &[u8], offset: usize) -> Option<u64> {
    data.get(offset..offset + 8)
        .map(|bytes| u64::from_le_bytes(bytes.try_into().expect("eight bytes")))
}

/// `["budget", buyer]` under the Payvand program.
pub fn budget_address(buyer: &Pubkey, program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"budget", buyer.as_ref()], program).0
}

/// `["token-budget", buyer, mint]` under the Payvand program.
pub fn token_budget_address(buyer: &Pubkey, mint: &Pubkey, program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[TOKEN_BUDGET_SEED, buyer.as_ref(), mint.as_ref()], program).0
}

/// The associated token account a token budget owns for its mint: where
/// `open_token_budget` puts the pot, and therefore where a reminder looks for
/// what is left.
pub fn token_vault_address(budget: &Pubkey, mint: &Pubkey) -> Pubkey {
    let token_program = Pubkey::from_str(TOKEN_PROGRAM_ID).expect("SPL token program id");
    let associated =
        Pubkey::from_str(ASSOCIATED_TOKEN_PROGRAM_ID).expect("SPL associated token program id");
    Pubkey::find_program_address(
        &[budget.as_ref(), token_program.as_ref(), mint.as_ref()],
        &associated,
    )
    .0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(limit: u64, month_index: i64, pot: u64) -> BudgetFacts {
        BudgetFacts {
            limit,
            month_index,
            pot,
        }
    }

    #[test]
    fn a_rolled_over_month_with_a_short_pot_is_worth_a_nudge() {
        // Spent the month dry: the counter reset, the pot did not.
        assert!(needs_nudge(&facts(100_000_000, 24_322, 0), 24_323));
        // Still some left, but not the full limit.
        assert!(needs_nudge(&facts(100_000_000, 24_322, 20_000_000), 24_323));
    }

    #[test]
    fn nothing_is_sent_when_the_pot_is_already_full_or_the_month_has_not_turned() {
        // Nothing was spent, so a new month changes nothing.
        assert!(!needs_nudge(
            &facts(100_000_000, 24_322, 100_000_000),
            24_323
        ));
        // Mid-month: the in-app banner covers it, not a notification.
        assert!(!needs_nudge(&facts(100_000_000, 24_323, 0), 24_323));
        // An empty limit is an allowance nobody set up.
        assert!(!needs_nudge(&facts(0, 24_322, 0), 24_323));
    }

    #[test]
    fn a_token_budget_reads_its_limit_month_and_vault_pot() {
        let mut data = vec![0u8; TOKEN_BUDGET_SPACE];
        data[TOKEN_BUDGET_LIMIT_OFFSET..TOKEN_BUDGET_LIMIT_OFFSET + 8]
            .copy_from_slice(&20_000_000u64.to_le_bytes());
        data[TOKEN_BUDGET_MONTH_OFFSET..TOKEN_BUDGET_MONTH_OFFSET + 8]
            .copy_from_slice(&24_322u64.to_le_bytes());

        let read = token_budget_facts(&data, 4_000_000).expect("facts");
        assert_eq!(read, facts(20_000_000, 24_322, 4_000_000));
        // The pot is the vault's balance: the tokens do not sit in this account.
        assert_eq!(read.pot, 4_000_000);
        // A truncated account is not a budget.
        assert!(token_budget_facts(&data[..TOKEN_BUDGET_SPACE - 1], 0).is_none());
    }

    #[test]
    fn a_token_allowance_alone_is_enough_to_remind() {
        // A SOL allowance that spent nothing, beside a USDC one that did.
        let budgets = [
            facts(100_000_000, 24_322, 100_000_000),
            facts(20_000_000, 24_322, 0),
        ];
        assert!(!needs_nudge(&budgets[0], 24_323));
        assert!(budgets.iter().any(|facts| needs_nudge(facts, 24_323)));

        // Every pot full: nothing to say, whatever the currency.
        let full = [
            facts(100_000_000, 24_322, 100_000_000),
            facts(20_000_000, 24_322, 20_000_000),
        ];
        assert!(!full.iter().any(|facts| needs_nudge(facts, 24_323)));
    }

    #[test]
    fn reads_a_token_account_amount_at_its_offset() {
        let mut data = vec![0u8; 165];
        data[TOKEN_AMOUNT_OFFSET..TOKEN_AMOUNT_OFFSET + 8]
            .copy_from_slice(&1_865_207u64.to_le_bytes());
        assert_eq!(token_amount(&data), Some(1_865_207));
        // Shorter than the fixed fields before the amount: not a token account.
        assert!(token_amount(&data[..TOKEN_AMOUNT_OFFSET + 7]).is_none());
    }

    #[test]
    fn reads_the_figures_out_of_a_budget_account() {
        let mut data = vec![0u8; BUDGET_SPACE];
        data[..8].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        data[BUDGET_LIMIT_OFFSET..BUDGET_LIMIT_OFFSET + 8]
            .copy_from_slice(&50_000_000u64.to_le_bytes());
        data[BUDGET_MONTH_OFFSET..BUDGET_MONTH_OFFSET + 8]
            .copy_from_slice(&24_322u64.to_le_bytes());

        let rent = rent_exempt_minimum(BUDGET_SPACE);
        let read = budget_facts(&data, rent + 7_000_000).expect("facts");
        assert_eq!(read.limit, 50_000_000);
        assert_eq!(read.month_index, 24_322);
        assert_eq!(read.pot, 7_000_000);
        assert_eq!(rent, 1_566_000);
        // A truncated account is not a budget.
        assert!(budget_facts(&data[..90], rent).is_none());
    }

    #[test]
    fn counts_months_the_way_the_program_does() {
        // 2026-11-23T00:00:00Z, 2026-12-23T00:00:00Z and 2026-01-01T00:00:00Z.
        assert_eq!(current_month_index(1_795_392_000), 24_323);
        assert_eq!(current_month_index(1_795_392_000), 2026 * 12 + 11);
        assert_eq!(current_month_index(1_797_984_000), 2026 * 12 + 12);
        assert_eq!(current_month_index(1_767_225_600), 2026 * 12 + 1);
    }
}
