//! Contract test harness: an in-process bank plus the settlement paths for the
//! SOL and token instructions, including the Phase 1 receipt and replay rules.
//!
//! The tests run the program's real SBF binary — `target/deploy/payvand.so` —
//! inside `solana-program-test`, so they need no cluster, no
//! `solana-test-validator` process and no wallet, but they do need the program
//! built first:
//!
//! ```bash
//! cd apps/contracts && cargo build-sbf --manifest-path programs/payvand/Cargo.toml
//! cargo test --manifest-path apps/contracts/Cargo.toml
//! ```
//!
//! `task test:contracts`, `task anchor:test` and CI all build the program
//! before running the suite. The harness fails loudly (instead of skipping or
//! running an old artifact) when the `.so` is missing, older than `src/`, or
//! built for a target this runtime cannot execute.
//!
//! Use `cargo build-sbf` (arch v0) and not only `anchor build`: Anchor 1.2.1
//! builds the eBPF target (arch v1), which `solana-program-test` 3.1.10 cannot
//! execute — the suite would otherwise fail with a bare `UnsupportedProgramId`.
//!
//! The runtime ships no SPL: the deployed token and associated-token programs
//! the token flows need are committed as ELFs under `tests/fixtures/` and
//! registered by `program_test()` (see the README there).
//!
//! PDA helpers below derive addresses from the program's own `constants`
//! module, so a seed change breaks the tests instead of silently passing
//! against a copy.
//!
// `solana-program-test` marks its API "agave-unstable" with deprecation
// warnings; this suite is written against that API deliberately.
#![allow(deprecated)]

use anchor_lang::prelude::Rent;
use anchor_lang::{
    system_program::ID as SYSTEM_PROGRAM_ID, AccountDeserialize, AnchorDeserialize,
    AnchorSerialize, Discriminator,
};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use payvand::constants::{
    BUDGET_SEED, BUDGET_SPACE, CONFIG_SEED, CONFIG_SPACE, CONFIG_SPACE_V1, CONFIG_SPACE_V2,
    CONFIG_SPACE_V3, CONFIG_VERSION, FEE_ANCHOR_SEED, FLATS_REFRESH_SECS, MAX_NETWORK_FEE_LAMPORTS,
    MAX_SOL_FEE_LAMPORTS, MAX_TOKEN_FEE_BASE_UNITS, MIN_SESSION_FEE_FLOAT, PAYMENT_MAX_MEMO,
    PAYMENT_RECEIPT_SEED, PAYMENT_RECEIPT_SPACE, PAYMENT_SEED, PAYMENT_SPACE,
    PRICE_ANCHOR_BAND_BPS, TREASURY_RESERVE_SETTLEMENTS, TREASURY_SEED,
};
use payvand::constants::{
    SESSION_FEE_FLOAT, TOKEN_BUDGET_SEED, TOKEN_BUDGET_SPACE, TOKEN_PAYMENT_SEED,
    TOKEN_PAYMENT_SPACE,
};
use payvand::events::{BudgetOpened, ConfigUpdated, PaymentSettled};
use payvand::helpers::{fee_for, fee_for_signatures};
use payvand::state::{
    Budget, Config, FeeMode, FeePolicy, Payment, PaymentReceipt, PaymentState, TokenBudget,
    TokenPayment,
};
use sha2::{Digest, Sha256};
use solana_account::Account;
use solana_clock::Clock;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_pubkey::Pubkey;
use solana_sdk_ids::{bpf_loader, bpf_loader_upgradeable};
use solana_signer::Signer;
use solana_transaction::Transaction;

const LAMPORTS_PER_SOL: u64 = 1_000_000_000;
/// ELF `e_machine` of the legacy SBF target, what `cargo build-sbf` (arch v0)
/// emits and what this runtime executes.
const SBF_MACHINE: u16 = 0x0107;
/// ELF `e_machine` of the eBPF target (arch v1), which `anchor build` emits and
/// `solana-program-test` 3.1.10 cannot load.
const EBPF_MACHINE: u16 = 0x00f7;
/// One signature, no priority fee: what a `send` transaction costs in
/// `solana-program-test`.
const TX_FEE: u64 = 5_000;
/// `PaymentError::AlreadySettled`, counting from 6000 in `errors.rs`.
const ALREADY_SETTLED: &str = "Custom(6002)";
/// `PaymentError::InvoiceTooSmall`.
const INVOICE_TOO_SMALL: &str = "Custom(6003)";
/// `PaymentError::InvalidConfigAdmin`.
const INVALID_CONFIG_ADMIN: &str = "Custom(6010)";
/// `PaymentError::InvalidFeeBps`.
const INVALID_FEE_BPS: &str = "Custom(6012)";
/// `PaymentError::TreasuryUnderfunded`.
const TREASURY_UNDERFUNDED: &str = "Custom(6014)";
/// `PaymentError::FeeModeUnsupported`.
const FEE_MODE_UNSUPPORTED: &str = "Custom(6015)";
/// `PaymentError::Token2022Unsupported`: a mint the SPL Token program does not
/// own is refused by name rather than inside a token CPI.
const TOKEN_2022_UNSUPPORTED: &str = "Custom(6017)";
/// `PaymentError::InvalidConfigVersion`: a config that is not the layout
/// `migrate_config` upgrades.
const INVALID_CONFIG_VERSION: &str = "Custom(6018)";
/// `PaymentError::InvalidFeePrice`.
const INVALID_FEE_PRICE: &str = "Custom(6019)";
/// `PaymentError::FeePriceTooSoon`.
const FEE_PRICE_TOO_SOON: &str = "Custom(6020)";
/// `PaymentError::InvalidNetworkFee`: a policy whose network fee is larger than
/// the program will reimburse.
const INVALID_NETWORK_FEE: &str = "Custom(6023)";
/// `PaymentError::BudgetExhausted`: the month's limit, not the pot.
const BUDGET_EXHAUSTED: &str = "Custom(6005)";
/// `PaymentError::InsufficientBudget`: the pot cannot cover this one.
const INSUFFICIENT_BUDGET: &str = "Custom(6006)";
/// `PaymentError::MintMismatch`: the mint named is not the budget's.
const MINT_MISMATCH: &str = "Custom(6008)";
/// Anchor's `ConstraintRaw`: a non-`init` `associated_token::` constraint
/// compares the account against the derived address, so a decoy fails here.
const NOT_ASSOCIATED_TOKEN_ACCOUNT: &str = "Custom(2009)";
/// The policy every test starts from unless it says otherwise.
const TEST_FEE_BPS: u16 = 200;
const TEST_FIXED_FEE: u64 = 0;
const TEST_SESSION_FLOAT: u64 = 10_000_000;
/// Enough for many receipt-rent refunds.
const TREASURY_LAMPORTS: u64 = LAMPORTS_PER_SOL;

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// Directory `anchor build` / `cargo build-sbf` writes the deployable program
/// into.
fn deploy_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/deploy")
}

/// Makes the program discoverable and fails with an actionable message instead
/// of letting `ProgramTest` panic with a bare "unable to locate" or, worse,
/// running an artifact that predates the current source.
fn locate_program() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let deploy_dir = deploy_dir();
        let program = deploy_dir.join("payvand.so");
        assert!(
            program.exists(),
            "missing {}: run `cd apps/contracts && anchor build` (or `cargo build-sbf`) first",
            program.display()
        );

        // A stale `.so` would test yesterday's program and report green.
        let newest_source = std::fs::read_dir(format!("{}/src", env!("CARGO_MANIFEST_DIR")))
            .expect("cannot read the program's src directory")
            .filter_map(Result::ok)
            .filter_map(|entry| entry.metadata().and_then(|m| m.modified()).ok())
            .max()
            .expect("the program's src directory is empty");
        let built = std::fs::metadata(&program)
            .and_then(|m| m.modified())
            .expect("cannot read the program artifact's timestamp");
        assert!(
            built >= newest_source,
            "{} is older than the program source: rebuild with `cargo build-sbf --manifest-path programs/payvand/Cargo.toml`",
            program.display()
        );

        // `anchor build` emits the eBPF target (e_machine 0x00f7); this runtime
        // executes only the legacy SBF target (0x0107) and reports anything else
        // as a bare `UnsupportedProgramId` from the first settle, which reads
        // like a program bug. Say what actually happened instead.
        let header = std::fs::read(&program).expect("cannot read the program artifact");
        let machine = u16::from_le_bytes([header[18], header[19]]);
        assert!(
            machine == SBF_MACHINE,
            "{} is built for the {} target (e_machine 0x{machine:04x}), which solana-program-test cannot execute: \
             rebuild it with `cargo build-sbf --manifest-path programs/payvand/Cargo.toml` (arch v0)",
            program.display(),
            if machine == EBPF_MACHINE { "eBPF" } else { "unknown" }
        );

        // Read by `ProgramTest::default()` when it decides to prefer the SBF
        // artifact over a native processor; must be set before construction.
        std::env::set_var("SBF_OUT_DIR", &deploy_dir);
    });
}

/// A program-test instance running the built program under its declared ID.
///
/// `None` means "no native processor": `solana-program-test` loads
/// `payvand.so` from `SBF_OUT_DIR`, so the tests execute the deployable SBF
/// binary through the same entrypoint the cluster uses.
fn program_test() -> ProgramTest {
    locate_program();
    let mut test = ProgramTest::new("payvand", payvand::id(), None);
    test.add_account(token_program(), spl_fixture("spl_token"));
    test.add_account(
        associated_token_program(),
        spl_fixture("spl_associated_token_account"),
    );
    test
}

/// One of the deployed SPL programs committed under `tests/fixtures/`.
///
/// The runtime has no built-in SPL, and `ProgramTest::add_program` can only find
/// artifacts our own build produces in `SBF_OUT_DIR`. Registering the dumped ELF
/// as an executable account is what that helper does internally, so the token
/// CPIs below run the real programs with no network access at test time.
///
/// The dumps are the one input here that outlives this toolchain, so the header
/// is checked rather than assumed: an ELF this runtime cannot load fails every
/// token test as a bare `UnsupportedProgramId` from the first transfer, which
/// reads like a program bug. `tests/fixtures/README.md` says how to re-dump.
fn spl_fixture(name: &str) -> Account {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(format!("{name}.so"));
    let data = std::fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "missing SPL fixture {} ({error}): see tests/fixtures/README.md",
            path.display()
        )
    });
    assert!(
        data.len() > 20 && data.starts_with(b"\x7fELF"),
        "{} is not an ELF: re-dump it as tests/fixtures/README.md explains",
        path.display()
    );
    let machine = u16::from_le_bytes([data[18], data[19]]);
    assert!(
        machine == SBF_MACHINE || machine == EBPF_MACHINE,
        "{} is built for an unknown target (e_machine 0x{machine:04x}): re-dump it as tests/fixtures/README.md explains",
        path.display()
    );
    Account {
        lamports: Rent::default().minimum_balance(data.len()),
        data,
        owner: bpf_loader::id(),
        executable: true,
        rent_epoch: 0,
    }
}

/// An empty system account holding `lamports`.
fn system_account(lamports: u64) -> Account {
    Account {
        lamports,
        data: Vec::new(),
        owner: SYSTEM_PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    }
}

/// Anchor instruction discriminator: `sha256("global:<name>")[..8]`.
fn discriminator(name: &str) -> [u8; 8] {
    let digest = Sha256::digest(format!("global:{name}").as_bytes());
    let mut out = [0u8; 8];
    out.copy_from_slice(&digest[..8]);
    out
}

/// Anchor account discriminator: `sha256("account:<name>")[..8]`.
fn account_discriminator(name: &str) -> [u8; 8] {
    let digest = Sha256::digest(format!("account:{name}").as_bytes());
    let mut out = [0u8; 8];
    out.copy_from_slice(&digest[..8]);
    out
}

fn address(text: &str) -> Pubkey {
    text.parse().expect("the test's fixed address should parse")
}

/// The SPL token program: owner of every mint and token account, and the program
/// the transfers and closes CPI into.
fn token_program() -> Pubkey {
    address("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA")
}

/// The associated-token program, which creates the budget's vault and the
/// treasury's own token account. A seller's account is theirs to open.
fn associated_token_program() -> Pubkey {
    address("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL")
}

/// A mint account in the SPL layout: authority, supply, decimals, initialized.
fn spl_mint_account(authority: Pubkey, decimals: u8) -> Account {
    let mut data = vec![0u8; 82];
    data[0..4].copy_from_slice(&1u32.to_le_bytes());
    data[4..36].copy_from_slice(authority.as_ref());
    data[44] = decimals;
    data[45] = 1;
    Account {
        lamports: Rent::default().minimum_balance(data.len()),
        data,
        owner: token_program(),
        executable: false,
        rent_epoch: 0,
    }
}

/// A token account in the SPL layout: mint, authority, amount, initialized.
fn spl_token_account(mint: Pubkey, authority: Pubkey, amount: u64) -> Account {
    let mut data = vec![0u8; 165];
    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(authority.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1;
    Account {
        lamports: Rent::default().minimum_balance(data.len()),
        data,
        owner: token_program(),
        executable: false,
        rent_epoch: 0,
    }
}

/// The same account frozen: `AccountState::Frozen` in the SPL layout's state
/// byte, which is what the mint's freeze authority would write. Every transfer
/// into or out of it is refused, a zero one included.
fn frozen_token_account(mint: Pubkey, authority: Pubkey) -> Account {
    let mut account = spl_token_account(mint, authority, 0);
    account.data[108] = 2;
    account
}

/// Token-2022: the other token program on Solana, which this program does not
/// support.
fn token_2022_program() -> Pubkey {
    address("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb")
}

/// A mint in the layout a Token-2022 mint with no extensions has, owned by
/// Token-2022: exactly what a caller of a Token-2022 invoice passes.
fn token_2022_mint_account(authority: Pubkey, decimals: u8) -> Account {
    let mut account = spl_mint_account(authority, decimals);
    account.owner = token_2022_program();
    account
}

/// A token account owned by Token-2022.
fn token_2022_token_account(mint: Pubkey, authority: Pubkey, amount: u64) -> Account {
    let mut account = spl_token_account(mint, authority, amount);
    account.owner = token_2022_program();
    account
}

/// Borsh-encoded instruction data: discriminator, then the arguments.
fn instruction_data(name: &str, args: &[u8]) -> Vec<u8> {
    let mut data = discriminator(name).to_vec();
    data.extend_from_slice(args);
    data
}

/// `amount` as Borsh `u64`, followed by `memo` as a Borsh string.
fn amount_and_memo(amount: u64, memo: &str) -> Vec<u8> {
    let mut data = amount.to_le_bytes().to_vec();
    data.extend_from_slice(&(memo.len() as u32).to_le_bytes());
    data.extend_from_slice(memo.as_bytes());
    data
}

/// `["payment", seller, reference, amount]` — the same seeds the program uses.
fn payment_pda(seller: &Pubkey, reference: &Pubkey, amount: u64) -> Pubkey {
    let (pda, _bump) = Pubkey::find_program_address(
        &[
            PAYMENT_SEED,
            seller.as_ref(),
            reference.as_ref(),
            &amount.to_le_bytes(),
        ],
        &payvand::id(),
    );
    pda
}

/// `["payment-receipt", invoice]` — keyed by the invoice address so a watcher
/// that only holds the QR code's address can still find the record.
fn receipt_pda(invoice: &Pubkey) -> Pubkey {
    let (pda, _bump) =
        Pubkey::find_program_address(&[PAYMENT_RECEIPT_SEED, invoice.as_ref()], &payvand::id());
    pda
}

/// `["budget", buyer]` — the same seeds the program uses.
fn budget_pda(buyer: &Pubkey) -> Pubkey {
    let (pda, _bump) = Pubkey::find_program_address(&[BUDGET_SEED, buyer.as_ref()], &payvand::id());
    pda
}

/// `["token-budget", buyer, mint]` and its canonical bump, so a test that writes
/// the account by hand stores the bump the program's seeds check expects.
fn token_budget_pda(buyer: &Pubkey, mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[TOKEN_BUDGET_SEED, buyer.as_ref(), mint.as_ref()],
        &payvand::id(),
    )
}

/// The canonical associated token account for `wallet` and `mint` under the
/// legacy token program.
fn associated_token_address(wallet: Pubkey, mint: Pubkey) -> Pubkey {
    associated_token_address_for_program(token_program(), wallet, mint)
}

/// The same derivation for a chosen token program: Token-2022 accounts live at
/// `[wallet, token-2022 program, mint]`, which is what a Token-2022 attempt
/// hands the program.
fn associated_token_address_for_program(
    token_program: Pubkey,
    wallet: Pubkey,
    mint: Pubkey,
) -> Pubkey {
    Pubkey::find_program_address(
        &[wallet.as_ref(), token_program.as_ref(), mint.as_ref()],
        &associated_token_program(),
    )
    .0
}

/// `["config"]` — the singleton fee policy.
fn config_pda() -> Pubkey {
    Pubkey::find_program_address(&[CONFIG_SEED], &payvand::id()).0
}

/// `["treasury"]` — the only account a fee may be sent to.
fn treasury_pda() -> Pubkey {
    Pubkey::find_program_address(&[TREASURY_SEED], &payvand::id()).0
}

/// `["fee-anchor"]` — the admin's SOL price, the band a permissionless refresh
/// is measured against.
fn fee_anchor_pda() -> Pubkey {
    Pubkey::find_program_address(&[FEE_ANCHOR_SEED], &payvand::id()).0
}

/// This program's `ProgramData` address under the upgradeable loader.
fn program_data_address() -> Pubkey {
    Pubkey::find_program_address(&[payvand::id().as_ref()], &bpf_loader_upgradeable::id()).0
}

/// A `ProgramData` account whose upgrade authority is `authority`. ProgramTest
/// loads the program itself under the plain loader, so the suite writes this
/// account by hand to stand in for a deployed, upgradeable program.
fn program_data_account(authority: Option<Pubkey>) -> Account {
    let state =
        anchor_lang::solana_program::bpf_loader_upgradeable::UpgradeableLoaderState::ProgramData {
            slot: 0,
            upgrade_authority_address: authority,
        };
    let metadata =
        anchor_lang::solana_program::bpf_loader_upgradeable::UpgradeableLoaderState::size_of_programdata_metadata();
    let mut data = bincode::serialize(&state).expect("ProgramData state serializes");
    data.resize(metadata, 0);
    Account {
        lamports: Rent::default().minimum_balance(data.len()),
        data,
        owner: bpf_loader_upgradeable::id(),
        executable: false,
        rent_epoch: 0,
    }
}

/// The USDC mint the per-currency flat applies to. No invoice in this suite is
/// minted from it, so only the tests that name it pay a flat.
fn usdc_mint() -> Pubkey {
    Pubkey::new_from_array([1u8; 32])
}

fn eurc_mint() -> Pubkey {
    Pubkey::new_from_array([2u8; 32])
}

/// A policy that charges `fee_bps` on everything: a boundary of 0 leaves every
/// amount above it, so the two marginal rates collapse into one flat rate, and
/// `fixed_fee_lamports` becomes the flat on both test token currencies.
fn test_policy_of(fee_bps: u16, fixed_fee_lamports: u64) -> FeePolicy {
    FeePolicy {
        fee_bps_below: fee_bps,
        fee_bps_above: fee_bps,
        sol_tier_lamports: 0,
        token_tier_base_units: 0,
        flat_usdc_base_units: fixed_fee_lamports,
        flat_eurc_base_units: fixed_fee_lamports,
        usdc_mint: usdc_mint(),
        eurc_mint: eurc_mint(),
        // No network fee: the default suite is about the rate and the rents, and
        // a test that cares about the fee names one.
        network_fee_lamports: 0,
        network_fee_usdc_base_units: 0,
        network_fee_eurc_base_units: 0,
    }
}

/// The production policy the tests that need real tiers install: 1% up to
/// 0.1 SOL / 10 tokens, 0.5% above it, and the marker's rent as a flat in each
/// token currency.
fn tiered_policy() -> FeePolicy {
    FeePolicy {
        fee_bps_below: 100,
        fee_bps_above: 50,
        sol_tier_lamports: 100_000_000,
        token_tier_base_units: 10_000_000,
        flat_usdc_base_units: 89_400,
        flat_eurc_base_units: 89_400,
        usdc_mint: usdc_mint(),
        eurc_mint: eurc_mint(),
        network_fee_lamports: 0,
        network_fee_usdc_base_units: 0,
        network_fee_eurc_base_units: 0,
    }
}

/// Borsh `initialize_config` / `update_config` / `migrate_config` arguments: the
/// policy, then the fee mode and the session float.
fn fee_policy_args(
    fee_bps: u16,
    fixed_fee_lamports: u64,
    fee_mode: FeeMode,
    session_fee_float: u64,
) -> Vec<u8> {
    let mode = match fee_mode {
        FeeMode::SellerPays => 0u8,
        FeeMode::BuyerPays => 1u8,
    };
    let mut data = Vec::new();
    test_policy_of(fee_bps, fixed_fee_lamports)
        .serialize(&mut data)
        .expect("the policy serializes");
    data.push(mode);
    data.extend_from_slice(&session_fee_float.to_le_bytes());
    data
}

fn initialize_config_instruction(
    admin: Pubkey,
    fee_bps: u16,
    fixed_fee_lamports: u64,
    fee_mode: FeeMode,
    session_fee_float: u64,
) -> Instruction {
    initialize_config_instruction_to(
        admin,
        program_data_address(),
        fee_bps,
        fixed_fee_lamports,
        fee_mode,
        session_fee_float,
    )
}

/// `initialize_config` with a caller-chosen `ProgramData` account, so a test
/// can prove the seeds constraint refuses a decoy.
fn initialize_config_instruction_to(
    admin: Pubkey,
    program_data: Pubkey,
    fee_bps: u16,
    fixed_fee_lamports: u64,
    fee_mode: FeeMode,
    session_fee_float: u64,
) -> Instruction {
    Instruction {
        program_id: payvand::id(),
        accounts: vec![
            AccountMeta::new(admin, true),
            AccountMeta::new_readonly(program_data, false),
            AccountMeta::new(config_pda(), false),
            AccountMeta::new(treasury_pda(), false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: instruction_data(
            "initialize_config",
            &fee_policy_args(fee_bps, fixed_fee_lamports, fee_mode, session_fee_float),
        ),
    }
}

fn update_config_instruction(
    admin: Pubkey,
    fee_bps: u16,
    fixed_fee_lamports: u64,
    fee_mode: FeeMode,
    session_fee_float: u64,
) -> Instruction {
    Instruction {
        program_id: payvand::id(),
        accounts: vec![
            AccountMeta::new_readonly(admin, true),
            AccountMeta::new(config_pda(), false),
        ],
        data: instruction_data(
            "update_config",
            &fee_policy_args(fee_bps, fixed_fee_lamports, fee_mode, session_fee_float),
        ),
    }
}

/// `migrate_config(policy)` on the config PDA, with the system program so the
/// admin can pay the rent the taller account needs.
fn migrate_config_instruction(admin: Pubkey, policy: FeePolicy) -> Instruction {
    let mut args = Vec::new();
    policy.serialize(&mut args).expect("the policy serializes");
    Instruction {
        program_id: payvand::id(),
        accounts: vec![
            AccountMeta::new(admin, true),
            AccountMeta::new(config_pda(), false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: instruction_data("migrate_config", &args),
    }
}

/// `refresh_fee_price` arguments — the SOL price in USD then in EUR, micros per
/// SOL — and its accounts: any signer, the config it rewrites, and the admin's
/// price anchor that bounds it.
fn refresh_fee_price_instruction(
    caller: Pubkey,
    sol_usd_micros: u64,
    sol_eur_micros: u64,
) -> Instruction {
    let mut args = Vec::new();
    args.extend_from_slice(&sol_usd_micros.to_le_bytes());
    args.extend_from_slice(&sol_eur_micros.to_le_bytes());
    Instruction {
        program_id: payvand::id(),
        accounts: vec![
            AccountMeta::new_readonly(caller, true),
            AccountMeta::new(config_pda(), false),
            AccountMeta::new_readonly(fee_anchor_pda(), false),
        ],
        data: instruction_data("refresh_fee_price", &args),
    }
}

/// `set_fee_price_anchor(usd, eur)`, signed by the admin: creates the anchor on
/// the first call and re-pegs both flats at that price.
fn set_fee_price_anchor_instruction(
    admin: Pubkey,
    sol_usd_micros: u64,
    sol_eur_micros: u64,
) -> Instruction {
    let mut args = Vec::new();
    args.extend_from_slice(&sol_usd_micros.to_le_bytes());
    args.extend_from_slice(&sol_eur_micros.to_le_bytes());
    Instruction {
        program_id: payvand::id(),
        accounts: vec![
            AccountMeta::new(admin, true),
            AccountMeta::new(config_pda(), false),
            AccountMeta::new(fee_anchor_pda(), false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: instruction_data("set_fee_price_anchor", &args),
    }
}

/// `price_micros` raised by `percent`, rounded down: the step a caller trying to
/// walk the flats would send next.
fn price_above(price_micros: u64, percent: u64) -> u64 {
    price_micros * (100 + percent) / 100
}

/// What `fee_for` charges as a token flat at a price: the marker's rent in SOL,
/// times the price, in six-decimal base units.
fn flat_at(receipt_rent: u64, price_micros: u64) -> u64 {
    (receipt_rent as u128 * price_micros as u128 / 1_000_000_000) as u64
}

/// A version 1 `Config` account: the 62-byte layout the live devnet config still
/// has, and the shape `migrate_config` upgrades from.
fn v1_config_account(
    admin: Pubkey,
    bump: u8,
    treasury_bump: u8,
    session_fee_float: u64,
) -> Account {
    let mut data = Config::DISCRIMINATOR.to_vec();
    data.extend_from_slice(admin.as_ref());
    data.extend_from_slice(&200u16.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.push(0); // FeeMode::SellerPays
    data.extend_from_slice(&session_fee_float.to_le_bytes());
    data.push(bump);
    data.push(treasury_bump);
    data.push(1); // version
    assert_eq!(data.len(), CONFIG_SPACE_V1);
    Account {
        lamports: Rent::default().minimum_balance(data.len()),
        data,
        owner: payvand::id(),
        executable: false,
        rent_epoch: 0,
    }
}

/// `update_config` carrying a whole policy, for the fields the `(rate, flat)`
/// helper shape cannot express — the network fee above all.
fn update_config_with_policy_instruction(admin: Pubkey, policy: FeePolicy) -> Instruction {
    let mut args = Vec::new();
    policy.serialize(&mut args).expect("the policy serializes");
    args.push(0); // FeeMode::SellerPays
    args.extend_from_slice(&TEST_SESSION_FLOAT.to_le_bytes());
    Instruction {
        program_id: payvand::id(),
        accounts: vec![
            AccountMeta::new_readonly(admin, true),
            AccountMeta::new(config_pda(), false),
        ],
        data: instruction_data("update_config", &args),
    }
}

/// A version 3 `Config` account: the same layout the suite migrates from, with
/// the `100`-byte `FeePolicy` that predates the network fee.
fn v3_config_account(
    admin: Pubkey,
    bump: u8,
    treasury_bump: u8,
    session_fee_float: u64,
    policy: &FeePolicy,
) -> Account {
    let mut data = Config::DISCRIMINATOR.to_vec();
    data.extend_from_slice(admin.as_ref());
    data.extend_from_slice(&policy.fee_bps_below.to_le_bytes());
    data.extend_from_slice(&policy.fee_bps_above.to_le_bytes());
    data.extend_from_slice(&policy.sol_tier_lamports.to_le_bytes());
    data.extend_from_slice(&policy.token_tier_base_units.to_le_bytes());
    data.extend_from_slice(&policy.flat_usdc_base_units.to_le_bytes());
    data.extend_from_slice(&policy.flat_eurc_base_units.to_le_bytes());
    data.extend_from_slice(policy.usdc_mint.as_ref());
    data.extend_from_slice(policy.eurc_mint.as_ref());
    data.push(0); // FeeMode::SellerPays
    data.extend_from_slice(&session_fee_float.to_le_bytes());
    data.extend_from_slice(&0i64.to_le_bytes()); // flats_updated_at
    data.push(bump);
    data.push(treasury_bump);
    data.push(3); // version
    assert_eq!(data.len(), CONFIG_SPACE_V3);
    Account {
        lamports: Rent::default().minimum_balance(data.len()),
        data,
        owner: payvand::id(),
        executable: false,
        rent_epoch: 0,
    }
}

fn withdraw_treasury_instruction(admin: Pubkey, amount: u64) -> Instruction {
    Instruction {
        program_id: payvand::id(),
        accounts: vec![
            AccountMeta::new(admin, true),
            AccountMeta::new_readonly(config_pda(), false),
            AccountMeta::new(treasury_pda(), false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: instruction_data("withdraw_treasury", &amount.to_le_bytes()),
    }
}

/// `settle_payment` with the accounts in the order the program declares them.
fn settle_instruction(
    payer: Pubkey,
    seller: Pubkey,
    reference: Pubkey,
    amount: u64,
    memo: &str,
) -> Instruction {
    settle_instruction_with_treasury(payer, seller, reference, amount, memo, treasury_pda())
}

/// `settle_payment` with a caller-chosen account in the treasury slot, so a
/// test can prove the seeds constraint refuses a stand-in.
fn settle_instruction_with_treasury(
    payer: Pubkey,
    seller: Pubkey,
    reference: Pubkey,
    amount: u64,
    memo: &str,
    treasury: Pubkey,
) -> Instruction {
    settle_instruction_with_sponsor(payer, payer, seller, reference, amount, memo, treasury)
}

/// `settle_payment` with the two roles split: `payer` pays the price, `sponsor`
/// advances both records and pays the transaction's fee, and the price repays it
/// in the same instruction. A relayer settlement is this call with a key the
/// payer does not hold.
#[allow(clippy::too_many_arguments)]
fn settle_instruction_with_sponsor(
    payer: Pubkey,
    sponsor: Pubkey,
    seller: Pubkey,
    reference: Pubkey,
    amount: u64,
    memo: &str,
    treasury: Pubkey,
) -> Instruction {
    let invoice = payment_pda(&seller, &reference, amount);
    Instruction {
        program_id: payvand::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(invoice, false),
            AccountMeta::new(seller, false),
            AccountMeta::new(receipt_pda(&invoice), false),
            AccountMeta::new_readonly(reference, false),
            AccountMeta::new_readonly(config_pda(), false),
            AccountMeta::new(treasury, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new(sponsor, true),
        ],
        data: instruction_data("settle_payment", &amount_and_memo(amount, memo)),
    }
}

/// `open_budget`, in the account order the program declares them. The same
/// instruction both opens the budget and refills it, which is why the counter
/// rules below matter.
fn open_budget_instruction(buyer: Pubkey, session: Pubkey, monthly_limit: u64) -> Instruction {
    Instruction {
        program_id: payvand::id(),
        accounts: vec![
            AccountMeta::new(buyer, true),
            AccountMeta::new(budget_pda(&buyer), false),
            AccountMeta::new(session, false),
            AccountMeta::new_readonly(config_pda(), false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: instruction_data("open_budget", &monthly_limit.to_le_bytes()),
    }
}

/// `close_budget`, in the account order the program declares them.
fn close_budget_instruction(buyer: Pubkey) -> Instruction {
    Instruction {
        program_id: payvand::id(),
        accounts: vec![
            AccountMeta::new(budget_pda(&buyer), false),
            AccountMeta::new(buyer, true),
        ],
        data: instruction_data("close_budget", &[]),
    }
}

/// `open_token_budget`, in the account order the program declares them.
fn open_token_budget_instruction(
    buyer: Pubkey,
    budget: Pubkey,
    mint: Pubkey,
    buyer_token_account: Pubkey,
    vault: Pubkey,
    session: Pubkey,
    monthly_limit: u64,
) -> Instruction {
    Instruction {
        program_id: payvand::id(),
        accounts: vec![
            AccountMeta::new(buyer, true),
            AccountMeta::new(budget, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(buyer_token_account, false),
            AccountMeta::new(vault, false),
            AccountMeta::new(session, false),
            AccountMeta::new_readonly(token_program(), false),
            AccountMeta::new_readonly(associated_token_program(), false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: instruction_data("open_token_budget", &monthly_limit.to_le_bytes()),
    }
}

/// `["token-payment", seller, reference, mint, amount]` — the invoice address
/// the token paths derive, and the receipt's seed.
fn token_payment_pda(seller: &Pubkey, reference: &Pubkey, mint: &Pubkey, amount: u64) -> Pubkey {
    Pubkey::find_program_address(
        &[
            TOKEN_PAYMENT_SEED,
            seller.as_ref(),
            reference.as_ref(),
            mint.as_ref(),
            &amount.to_le_bytes(),
        ],
        &payvand::id(),
    )
    .0
}

/// `pay_from_token_budget`, in the account order the program declares them. The
/// mint and vault are arguments rather than read from the world, so a test can
/// pass a stand-in for either.
fn pay_from_token_budget_instruction(
    world: &TokenWorld,
    mint: Pubkey,
    vault: Pubkey,
    seller: Pubkey,
    reference: Pubkey,
    amount: u64,
    memo: &str,
) -> Instruction {
    let invoice = token_payment_pda(&seller, &reference, &mint, amount);
    Instruction {
        program_id: payvand::id(),
        accounts: vec![
            AccountMeta::new(world.session.pubkey(), true),
            AccountMeta::new(world.budget, false),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(seller, false),
            AccountMeta::new(associated_token_address(seller, mint), false),
            AccountMeta::new(invoice, false),
            AccountMeta::new(receipt_pda(&invoice), false),
            AccountMeta::new_readonly(reference, false),
            AccountMeta::new_readonly(config_pda(), false),
            AccountMeta::new(treasury_pda(), false),
            AccountMeta::new(associated_token_address(treasury_pda(), mint), false),
            AccountMeta::new_readonly(token_program(), false),
            AccountMeta::new_readonly(associated_token_program(), false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: instruction_data("pay_from_token_budget", &amount_and_memo(amount, memo)),
    }
}

/// `close_token_budget`, in the account order the program declares them.
fn close_token_budget_instruction(
    buyer: Pubkey,
    budget: Pubkey,
    mint: Pubkey,
    vault: Pubkey,
    buyer_token_account: Pubkey,
) -> Instruction {
    Instruction {
        program_id: payvand::id(),
        accounts: vec![
            AccountMeta::new(buyer, true),
            AccountMeta::new(budget, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(vault, false),
            AccountMeta::new(buyer_token_account, false),
            AccountMeta::new_readonly(token_program(), false),
        ],
        data: instruction_data("close_token_budget", &[]),
    }
}

/// `open_treasury_token_account`: the treasury's own account for `mint`, paid by
/// the admin, which is where `settle_token_payment` sends the fee.
fn open_treasury_token_account_instruction(admin: Pubkey, mint: Pubkey) -> Instruction {
    Instruction {
        program_id: payvand::id(),
        accounts: vec![
            AccountMeta::new(admin, true),
            AccountMeta::new_readonly(config_pda(), false),
            AccountMeta::new_readonly(treasury_pda(), false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(associated_token_address(treasury_pda(), mint), false),
            AccountMeta::new_readonly(token_program(), false),
            AccountMeta::new_readonly(associated_token_program(), false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: instruction_data("open_treasury_token_account", &[]),
    }
}

/// `settle_token_payment`: the payer's own path, where the buyer carries nothing
/// but the price and the seller's account must already exist.
fn settle_token_payment_instruction(
    payer: Pubkey,
    seller: Pubkey,
    reference: Pubkey,
    mint: Pubkey,
    amount: u64,
    memo: &str,
) -> Instruction {
    settle_token_payment_instruction_with_sponsor(
        token_program(),
        payer,
        // A funded wallet is its own sponsor: one key, one signature, and the
        // refunds land where they always did.
        payer,
        seller,
        reference,
        mint,
        amount,
        memo,
    )
}

/// The same instruction for a caller-chosen token program. A Token-2022 attempt
/// is this call with the Token-2022 program and Token-2022 accounts, which is
/// how the program's refusal is exercised.
fn settle_token_payment_instruction_with_token_program(
    token_program: Pubkey,
    payer: Pubkey,
    seller: Pubkey,
    reference: Pubkey,
    mint: Pubkey,
    amount: u64,
    memo: &str,
) -> Instruction {
    settle_token_payment_instruction_with_sponsor(
        token_program,
        payer,
        payer,
        seller,
        reference,
        mint,
        amount,
        memo,
    )
}

/// `settle_token_payment` with the two roles split: `payer` signs for the buyer's
/// token account, `sponsor` advances the record rents and is paid the treasury's
/// refund of them. A relayer settlement is this call with a key the buyer does
/// not hold.
#[allow(clippy::too_many_arguments)]
fn settle_token_payment_instruction_with_sponsor(
    token_program: Pubkey,
    payer: Pubkey,
    sponsor: Pubkey,
    seller: Pubkey,
    reference: Pubkey,
    mint: Pubkey,
    amount: u64,
    memo: &str,
) -> Instruction {
    let invoice = token_payment_pda(&seller, &reference, &mint, amount);
    let ata = |wallet: Pubkey| associated_token_address_for_program(token_program, wallet, mint);
    Instruction {
        program_id: payvand::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(invoice, false),
            AccountMeta::new(seller, false),
            AccountMeta::new(ata(seller), false),
            AccountMeta::new(ata(payer), false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(receipt_pda(&invoice), false),
            AccountMeta::new_readonly(reference, false),
            AccountMeta::new_readonly(config_pda(), false),
            AccountMeta::new(treasury_pda(), false),
            AccountMeta::new(ata(treasury_pda()), false),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new(sponsor, true),
        ],
        data: instruction_data("settle_token_payment", &amount_and_memo(amount, memo)),
    }
}

/// The amount a token account holds: bytes 64..72 of the SPL layout.
async fn token_balance(context: &ProgramTestContext, address: Pubkey) -> u64 {
    let account = account(context, address)
        .await
        .unwrap_or_else(|| panic!("{address} is not a token account"));
    u64::from_le_bytes(account.data[64..72].try_into().expect("8 bytes"))
}

/// A `TokenBudget` account as the program would have written it after spending
/// `spent` in `month_index`. Writing the counter is cheaper than paying two real
/// invoices and keeps the refill test about the refill rule; the payments
/// themselves run for real in the token tests at the end of this file.
fn written_token_budget(
    buyer: Pubkey,
    session: Pubkey,
    mint: Pubkey,
    monthly_limit: u64,
    spent: u64,
    month_index: u64,
    bump: u8,
) -> Account {
    let mut data = account_discriminator("TokenBudget").to_vec();
    TokenBudget {
        buyer,
        session_key: session,
        mint,
        monthly_limit,
        spent,
        month_index,
        bump,
    }
    .serialize(&mut data)
    .expect("the token budget serializes");
    Account {
        lamports: Rent::default().minimum_balance(TOKEN_BUDGET_SPACE),
        data,
        owner: payvand::id(),
        executable: false,
        rent_epoch: 0,
    }
}

/// `pay_from_budget`, in the account order the program declares them.
fn pay_from_budget_instruction(
    session: Pubkey,
    budget: Pubkey,
    seller: Pubkey,
    reference: Pubkey,
    amount: u64,
    memo: &str,
) -> Instruction {
    let invoice = payment_pda(&seller, &reference, amount);
    Instruction {
        program_id: payvand::id(),
        accounts: vec![
            AccountMeta::new(session, true),
            AccountMeta::new(budget, false),
            AccountMeta::new(invoice, false),
            AccountMeta::new(seller, false),
            AccountMeta::new(receipt_pda(&invoice), false),
            AccountMeta::new_readonly(reference, false),
            AccountMeta::new_readonly(config_pda(), false),
            AccountMeta::new(treasury_pda(), false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: instruction_data("pay_from_budget", &amount_and_memo(amount, memo)),
    }
}

/// One `pay_from_budget` with a fresh seller and reference, so each call gets
/// its own invoice address and nothing is deduped or replayed. The session key is
/// the fee payer, as it is in the client: the float exists to cover these fees.
async fn pay_from_budget(
    context: &mut ProgramTestContext,
    budget: Pubkey,
    session: &Keypair,
    amount: u64,
    tag: &str,
) -> Result<(), String> {
    let seller = Keypair::new().pubkey();
    let reference = Keypair::new().pubkey();
    send_with_fee_payer(
        context,
        &[pay_from_budget_instruction(
            session.pubkey(),
            budget,
            seller,
            reference,
            amount,
            tag,
        )],
        session,
    )
    .await
}

/// Sends `instructions` with the test payer as fee payer, plus `signers`.
/// Advances the blockhash. Without this, re-sending an identical instruction
/// produces an identical transaction, which the bank dedupes as "already
/// processed" instead of executing it a second time.
async fn refresh_blockhash(context: &mut ProgramTestContext) {
    context
        .get_new_latest_blockhash()
        .await
        .expect("could not refresh the blockhash");
}

/// Moves the bank's clock forward, so a cooldown that reads `unix_timestamp`
/// can be walked past without waiting. Also refreshes the blockhash, since an
/// instruction repeated after this would otherwise be deduped as already
/// processed.
async fn advance_clock(context: &mut ProgramTestContext, seconds: i64) {
    let mut clock: Clock = context
        .banks_client
        .get_sysvar()
        .await
        .expect("the clock sysvar should be readable");
    clock.unix_timestamp += seconds;
    context.set_sysvar(&clock);
    refresh_blockhash(context).await;
}

async fn send(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    signers: &[&Keypair],
) -> Result<(), String> {
    let blockhash = context.last_blockhash;
    let payer = context.payer.pubkey();
    let mut all_signers: Vec<&Keypair> = vec![&context.payer];
    all_signers.extend_from_slice(signers);
    let tx =
        Transaction::new_signed_with_payer(instructions, Some(&payer), &all_signers, blockhash);
    context
        .banks_client
        .process_transaction(tx)
        .await
        .map_err(|error| format!("{error:?}"))
}

/// `send`, plus the runtime's log for the transaction: what the program actually
/// ran, so a test can pin a CPI that must not happen rather than only balances
/// that look the same either way.
async fn send_with_logs(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    signers: &[&Keypair],
) -> Result<Vec<String>, String> {
    let blockhash = context.last_blockhash;
    let payer = context.payer.pubkey();
    let mut all_signers: Vec<&Keypair> = vec![&context.payer];
    all_signers.extend_from_slice(signers);
    let tx =
        Transaction::new_signed_with_payer(instructions, Some(&payer), &all_signers, blockhash);
    let outcome = context
        .banks_client
        .process_transaction_with_metadata(tx)
        .await
        .map_err(|error| format!("{error:?}"))?;
    outcome.result.map_err(|error| format!("{error:?}"))?;
    Ok(outcome.metadata.map(|m| m.log_messages).unwrap_or_default())
}

/// How many times a transaction's log shows the system program being invoked.
/// Every CPI into it appears this way — the two account creations, the price
/// transfer, the fee transfer and the treasury's refund — so a settlement that
/// skipped one invocation definitely skipped that transfer.
fn system_program_invocations(log: &[String]) -> usize {
    const INVOKE: &str = "Program 11111111111111111111111111111111 invoke";
    log.iter().filter(|line| line.contains(INVOKE)).count()
}

/// Like `send`, but with the given key paying the fee — which is what the client
/// does for an allowance payment: the session key is its fee payer and only
/// signer, and the float exists to cover these fees.
async fn send_with_fee_payer(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    payer: &Keypair,
) -> Result<(), String> {
    let blockhash = context.last_blockhash;
    let tx = Transaction::new_signed_with_payer(
        instructions,
        Some(&payer.pubkey()),
        &[payer],
        blockhash,
    );
    context
        .banks_client
        .process_transaction(tx)
        .await
        .map_err(|error| format!("{error:?}"))
}

async fn account(context: &ProgramTestContext, address: Pubkey) -> Option<Account> {
    context
        .banks_client
        .get_account(address)
        .await
        .expect("account lookup failed")
}

async fn lamports(context: &ProgramTestContext, address: Pubkey) -> u64 {
    account(context, address).await.map_or(0, |a| a.lamports)
}

async fn receipt(context: &ProgramTestContext, address: Pubkey) -> PaymentReceipt {
    let account = account(context, address)
        .await
        .expect("payment receipt missing");
    PaymentReceipt::try_deserialize(&mut account.data.as_slice())
        .expect("payment receipt did not decode")
}

async fn budget(context: &ProgramTestContext, address: Pubkey) -> Budget {
    let account = account(context, address)
        .await
        .expect("budget account missing");
    Budget::try_deserialize(&mut account.data.as_slice()).expect("budget did not decode")
}

async fn token_budget(context: &ProgramTestContext, address: Pubkey) -> TokenBudget {
    let account = account(context, address)
        .await
        .expect("token budget account missing");
    TokenBudget::try_deserialize(&mut account.data.as_slice()).expect("token budget did not decode")
}

async fn config(context: &ProgramTestContext, address: Pubkey) -> Config {
    let account = account(context, address)
        .await
        .expect("config account missing");
    Config::try_deserialize(&mut account.data.as_slice()).expect("config did not decode")
}

/// What `init` puts in a receipt account. The payer fronts it and the seller
/// ends up paying for it: the seller's proceeds are reduced by exactly this, so
/// the payer is made whole without the treasury moving a lamport.
fn receipt_rent() -> u64 {
    Rent::default().minimum_balance(PAYMENT_RECEIPT_SPACE)
}

/// What `init` puts in a payment account, and what its close hands the seller.
fn invoice_rent() -> u64 {
    Rent::default().minimum_balance(PAYMENT_SPACE)
}

/// Anchor's `ConstraintHasOne`: `set_fee_price_anchor` is refused when the
/// signer is not the config's admin.
const HAS_ONE_FAILED: &str = "Custom(2001)";
/// The account constraint on a seller's token account that was never opened:
/// Anchor reports the missing account rather than creating it.
const MISSING_TOKEN_ACCOUNT: &str = "Custom(3012)";

/// The treasury's own rent-exempt minimum: it holds no data.
fn treasury_rent() -> u64 {
    Rent::default().minimum_balance(0)
}

/// What one token settlement draws from the pool: the invoice's rent and the
/// marker's, both advanced by the payer and refunded by the treasury.
fn token_refund() -> u64 {
    token_invoice_rent() + receipt_rent()
}

/// The lamports `withdraw_treasury` always leaves behind: the refunds it reserves
/// for token settlements, plus the account's own rent-exempt minimum.
fn treasury_reserve() -> u64 {
    token_refund() * TREASURY_RESERVE_SETTLEMENTS + treasury_rent()
}

/// What the budget account's own rent costs, and therefore what the program
/// subtracts before calling the rest the pot.
fn budget_rent() -> u64 {
    Rent::default().minimum_balance(BUDGET_SPACE)
}

/// The seller-side fee under the default test policy: the rate, less the
/// marker's rent, which the SOL path charges out of the price directly — so a
/// payment too small for the rate to cover the record pays no fee at all.
fn fee(amount: u64) -> u64 {
    let proportional = (amount as u128 * TEST_FEE_BPS as u128 / 10_000) as u64;
    proportional
        .saturating_sub(receipt_rent())
        .min(MAX_SOL_FEE_LAMPORTS)
}

/// The same under the token paths, where the record's rent is the currency's
/// flat and the fee is the larger of it and the rate.
fn token_fee(amount: u64) -> u64 {
    let proportional = (amount as u128 * TEST_FEE_BPS as u128 / 10_000) as u64;
    proportional
        .max(TEST_FIXED_FEE)
        .min(MAX_TOKEN_FEE_BASE_UNITS)
        .min(amount - 1)
}

/// A `Config` with the layout `initialize_config` writes and the given rate, for
/// the fee arithmetic that has no account to read a policy from.
fn test_policy(fee_bps: u16, fixed_fee_lamports: u64) -> Config {
    Config {
        admin: Pubkey::default(),
        policy: test_policy_of(fee_bps, fixed_fee_lamports),
        fee_mode: FeeMode::SellerPays,
        session_fee_float: TEST_SESSION_FLOAT,
        flats_updated_at: 0,
        bump: 255,
        treasury_bump: 254,
        version: CONFIG_VERSION,
    }
}

/// A `Config` carrying `policy`, for fee arithmetic that needs no account.
fn config_with_policy(policy: FeePolicy) -> Config {
    Config {
        policy,
        ..test_policy(0, 0)
    }
}

/// The smallest amount the policy accepts: the first one above
/// `invoice_rent + receipt_rent + fee`.
fn min_payable() -> u64 {
    let mut amount = invoice_rent() + receipt_rent();
    loop {
        if amount > invoice_rent() + receipt_rent() + fee(amount) {
            return amount;
        }
        amount += 1;
    }
}

/// A program-test instance with the fee policy initialized, so settlements can
/// run. The upgrade authority is `authority`, so tests can also exercise the
/// rejection paths.
async fn started_with_policy(
    authority: Option<Pubkey>,
    fee_bps: u16,
    fixed_fee_lamports: u64,
    fee_mode: FeeMode,
    treasury_lamports: u64,
    extra_accounts: &[(Pubkey, u64)],
) -> (ProgramTestContext, Keypair) {
    let mut test = program_test();
    // The admin pays `Config`'s rent and signs `initialize_config`.
    let admin = Keypair::new();
    test.add_account(admin.pubkey(), system_account(LAMPORTS_PER_SOL / 10));
    test.add_account(treasury_pda(), system_account(treasury_lamports));
    for (address, lamports) in extra_accounts {
        test.add_account(*address, system_account(*lamports));
    }
    let mut context = test.start_with_context().await;

    // The program itself is loaded under the plain loader here, so stand in for
    // the upgradeable loader's `ProgramData` account with the authority the
    // policy is initialized under.
    let authority = authority.unwrap_or_else(|| admin.pubkey());
    context.set_account(
        &program_data_address(),
        &program_data_account(Some(authority)).into(),
    );

    let initialize = initialize_config_instruction(
        admin.pubkey(),
        fee_bps,
        fixed_fee_lamports,
        fee_mode,
        TEST_SESSION_FLOAT,
    );
    send(&mut context, &[initialize], &[&admin])
        .await
        .expect("initialize_config failed");

    (context, admin)
}

/// The default instance: 2% seller fee, no fixed fee, funded treasury.
async fn started() -> ProgramTestContext {
    started_with_policy(
        None,
        TEST_FEE_BPS,
        TEST_FIXED_FEE,
        FeeMode::SellerPays,
        TREASURY_LAMPORTS,
        &[],
    )
    .await
    .0
}

// ---------------------------------------------------------------------------
// settle_payment
// ---------------------------------------------------------------------------

#[tokio::test]
async fn settle_payment_writes_a_receipt_and_closes_the_invoice() {
    let mut context = started().await;
    let payer = context.payer.pubkey();
    let seller = Keypair::new().pubkey();
    let reference = Keypair::new().pubkey();
    let amount = LAMPORTS_PER_SOL / 10;
    let memo = "coffee";
    let invoice = payment_pda(&seller, &reference, amount);
    let receipt_address = receipt_pda(&invoice);
    let payer_before = lamports(&context, payer).await;
    let treasury_before = lamports(&context, treasury_pda()).await;

    send(
        &mut context,
        &[settle_instruction(payer, seller, reference, amount, memo)],
        &[],
    )
    .await
    .expect("settle failed");

    // The seller absorbs the fee and the permanent marker's rent: `amount - rent
    // - receipt rent - fee` now and the invoice rent when the account closes.
    assert_eq!(
        lamports(&context, seller).await,
        amount - fee(amount) - receipt_rent()
    );

    // The invoice account is gone — settled invoices do not linger on chain.
    assert!(
        account(&context, invoice).await.is_none(),
        "the invoice account should be closed by the settle transaction"
    );

    // The marker survives the invoice and is exactly the layout the program
    // allocates: the guard, and the pointer back to the invoice it closed.
    let written = receipt(&context, receipt_address).await;
    assert_eq!(written.invoice, invoice);
    assert!(written.settled_at > 0, "the guard must be stamped");
    let receipt_account = account(&context, receipt_address)
        .await
        .expect("receipt account missing");
    assert_eq!(receipt_account.data.len(), PAYMENT_RECEIPT_SPACE);

    // The payer paid exactly the price plus the network fee. It advanced the
    // marker's rent at `init`, and that came back out of the seller's proceeds.
    let paid = payer_before - lamports(&context, payer).await;
    assert_eq!(paid, amount + TX_FEE);

    // The treasury is unchanged but for the fee: the SOL path no longer funds
    // the receipt.
    let treasury_delta = lamports(&context, treasury_pda()).await as i128 - treasury_before as i128;
    assert_eq!(treasury_delta, fee(amount) as i128);
}

/// What the bank charges for one signature, and therefore what a policy names as
/// `network_fee_lamports`: the per-signature cost, not the settle's total.
const SOL_SIGNATURE_FEE: u64 = 5_000;
/// A sponsored settle carries two signatures — the payer's and the relayer's —
/// so the seller is charged, and the relayer refunded, twice one signature.
const SPONSORED_SOL_FEE: u64 = 2 * SOL_SIGNATURE_FEE;

/// The small-payment half of the model on the SOL path: the payer spends the
/// price and not one lamport more, the sponsor gets back both rents and the
/// signature it paid, and a payment too small for the percentage to cover those
/// costs charges the seller exactly those costs — the platform keeps nothing.
#[tokio::test]
async fn a_small_sol_settle_charges_only_the_rent_and_the_network_fee() {
    let (mut context, admin) = started_with_policy(
        None,
        TEST_FEE_BPS,
        0,
        FeeMode::SellerPays,
        TREASURY_LAMPORTS,
        &[],
    )
    .await;
    let mut policy = test_policy_of(TEST_FEE_BPS, 0);
    policy.network_fee_lamports = SOL_SIGNATURE_FEE;
    send(
        &mut context,
        &[update_config_with_policy_instruction(
            admin.pubkey(),
            policy,
        )],
        &[&admin],
    )
    .await
    .expect("update_config failed");

    // 2% of this is 80,000, well below the 1,224,960 + 10,000 the costs come to.
    let amount = 4_000_000;
    // A payer holding the price and nothing else: no rent, no fee, no float.
    let buyer = Keypair::new();
    context.set_account(&buyer.pubkey(), &system_account(amount).into());
    let seller = Keypair::new().pubkey();
    let relayer = context.payer.pubkey();
    let relayer_before = lamports(&context, relayer).await;
    let treasury_before = lamports(&context, treasury_pda()).await;

    send(
        &mut context,
        &[settle_instruction_with_sponsor(
            buyer.pubkey(),
            relayer,
            seller,
            Keypair::new().pubkey(),
            amount,
            "coffee",
            treasury_pda(),
        )],
        &[&buyer],
    )
    .await
    .expect("a sponsored SOL settle should succeed");

    assert_eq!(
        lamports(&context, buyer.pubkey()).await,
        0,
        "the payer must need no lamports beyond the price"
    );
    // The rent is paid, the fee is covered, and no percentage was charged.
    assert_eq!(
        lamports(&context, seller).await,
        amount - receipt_rent() - SPONSORED_SOL_FEE
    );
    assert_eq!(
        lamports(&context, treasury_pda()).await as i128 - treasury_before as i128,
        0,
        "the platform keeps nothing on a small payment"
    );
    // The relayer paid two signatures and got exactly that back out of the
    // price, plus both rents it advanced: its net cost is zero.
    assert_eq!(lamports(&context, relayer).await, relayer_before);
}

/// The large-payment half: when the percentage is larger than the costs, the
/// seller pays exactly the percentage, the sponsor is still made whole, and the
/// platform keeps the percentage less the two costs it covered.
#[tokio::test]
async fn a_large_sol_settle_charges_only_the_platform_fee() {
    let (mut context, admin) = started_with_policy(
        None,
        TEST_FEE_BPS,
        0,
        FeeMode::SellerPays,
        TREASURY_LAMPORTS,
        &[],
    )
    .await;
    let mut policy = test_policy_of(TEST_FEE_BPS, 0);
    policy.network_fee_lamports = SOL_SIGNATURE_FEE;
    send(
        &mut context,
        &[update_config_with_policy_instruction(
            admin.pubkey(),
            policy,
        )],
        &[&admin],
    )
    .await
    .expect("update_config failed");

    let amount = LAMPORTS_PER_SOL / 10;
    let buyer = Keypair::new();
    context.set_account(&buyer.pubkey(), &system_account(amount).into());
    let seller = Keypair::new().pubkey();
    let relayer = context.payer.pubkey();
    let relayer_before = lamports(&context, relayer).await;
    let treasury_before = lamports(&context, treasury_pda()).await;
    let rate = amount * TEST_FEE_BPS as u64 / 10_000;

    send(
        &mut context,
        &[settle_instruction_with_sponsor(
            buyer.pubkey(),
            relayer,
            seller,
            Keypair::new().pubkey(),
            amount,
            "a big sale",
            treasury_pda(),
        )],
        &[&buyer],
    )
    .await
    .expect("a sponsored SOL settle should succeed");

    assert_eq!(
        lamports(&context, buyer.pubkey()).await,
        0,
        "the payer must need no lamports beyond the price"
    );
    assert_eq!(
        lamports(&context, seller).await,
        amount - rate,
        "the seller's bill is the percentage, rent and fee included"
    );
    assert_eq!(
        lamports(&context, treasury_pda()).await as i128 - treasury_before as i128,
        (rate - receipt_rent() - SPONSORED_SOL_FEE) as i128,
        "the platform keeps the percentage less the costs it covered"
    );
    assert_eq!(lamports(&context, relayer).await, relayer_before);
}

/// Task 3.1's zero-fee rule on the SOL wallet path: at 0% the settlement runs one
/// system-program invocation fewer than the same settlement at 2%, because the
/// `if fee > 0` guard skips the fee transfer instead of sending one for zero.
/// (The token paths pin the same rule by freezing the treasury's account; a zero
/// lamport transfer would succeed, so this path is pinned by what ran.)
#[tokio::test]
async fn a_zero_fee_settle_payment_sends_no_fee_transfer() {
    let amount = LAMPORTS_PER_SOL / 10;

    // The same settlement twice, differing only in the policy: the bank charges
    // the policy `initialize_config` installs, so each case gets its own context.
    let mut free = started_with_policy(None, 0, 0, FeeMode::SellerPays, TREASURY_LAMPORTS, &[])
        .await
        .0;
    let treasury_before = lamports(&free, treasury_pda()).await;
    let payer = free.payer.pubkey();
    let payer_before = lamports(&free, payer).await;
    let free_seller = Keypair::new().pubkey();
    let zero_fee_logs = send_with_logs(
        &mut free,
        &[settle_instruction(
            payer,
            free_seller,
            Keypair::new().pubkey(),
            amount,
            "no fee",
        )],
        &[],
    )
    .await
    .expect("a zero-fee settlement must succeed");

    // Nothing was sent to the treasury, and the seller absorbed no fee — only
    // the permanent marker's rent, which no policy waives.
    assert_eq!(lamports(&free, free_seller).await, amount - receipt_rent());
    assert_eq!(
        lamports(&free, treasury_pda()).await as i128 - treasury_before as i128,
        0,
        "a 0% settlement moves nothing in or out of the treasury"
    );
    assert_eq!(payer_before - lamports(&free, payer).await, amount + TX_FEE);

    // The control at 2%: the same shape, plus the fee transfer to the treasury.
    let mut charged = started().await;
    let fee_payer = charged.payer.pubkey();
    let fee_seller = Keypair::new().pubkey();
    let fee_logs = send_with_logs(
        &mut charged,
        &[settle_instruction(
            fee_payer,
            fee_seller,
            Keypair::new().pubkey(),
            amount,
            "fee",
        )],
        &[],
    )
    .await
    .expect("the 2% settlement must succeed");

    assert_eq!(
        system_program_invocations(&fee_logs),
        system_program_invocations(&zero_fee_logs) + 1,
        "the 2% settlement should make exactly one system invocation the 0% one \
         does not: the fee transfer to the treasury (0% log: {zero_fee_logs:?})"
    );
}

#[tokio::test]
async fn settle_payment_replay_is_rejected_as_already_settled() {
    let mut context = started().await;
    let payer = context.payer.pubkey();
    let seller = Keypair::new().pubkey();
    let reference = Keypair::new().pubkey();
    let amount = LAMPORTS_PER_SOL / 10;
    let settle = settle_instruction(payer, seller, reference, amount, "coffee");

    send(&mut context, std::slice::from_ref(&settle), &[])
        .await
        .expect("first settle failed");
    let seller_after_first = lamports(&context, seller).await;

    // Same seeds, same accounts, a second signature from the same wallet: the
    // receipt's `settled_at` is the only thing standing in the way. The new
    // blockhash keeps this a distinct transaction rather than a duplicate.
    refresh_blockhash(&mut context).await;
    let replay = send(&mut context, &[settle], &[]).await;
    let error = replay.expect_err("a settled invoice must not settle twice");
    assert!(
        error.contains(ALREADY_SETTLED),
        "expected {ALREADY_SETTLED}, got {error}"
    );
    assert_eq!(
        lamports(&context, seller).await,
        seller_after_first,
        "a rejected replay must not move lamports"
    );
}

#[tokio::test]
async fn settle_payment_rejects_an_amount_that_cannot_cover_the_rent() {
    let mut context = started().await;
    let payer = context.payer.pubkey();
    let seller = Keypair::new().pubkey();
    let reference = Keypair::new().pubkey();
    let floor = min_payable();

    // One lamport below the floor: the fee plus the invoice rent would swallow
    // the whole price, so the program refuses it before moving lamports.
    let too_small = send(
        &mut context,
        &[settle_instruction(
            payer,
            seller,
            reference,
            floor - 1,
            "dust",
        )],
        &[],
    )
    .await;
    let error = too_small.expect_err("an invoice below the rent-plus-fee floor must be refused");
    assert!(
        error.contains(INVOICE_TOO_SMALL),
        "expected {INVOICE_TOO_SMALL}, got {error}"
    );

    // The floor itself is a real invoice.
    send(
        &mut context,
        &[settle_instruction(payer, seller, reference, floor, "floor")],
        &[],
    )
    .await
    .expect("the smallest valid invoice must settle");
    assert_eq!(
        lamports(&context, seller).await,
        floor - fee(floor) - receipt_rent()
    );
}

// ---------------------------------------------------------------------------
// open_budget + pay_from_budget
// ---------------------------------------------------------------------------

#[tokio::test]
async fn pay_from_budget_settles_the_invoice_and_records_the_spend() {
    let monthly_limit = LAMPORTS_PER_SOL / 4;
    let session = Keypair::new();
    // The session key pays its own network fees and invoice rent, so it starts
    // with a balance and the default policy is already initialized.
    let (mut context, _admin) = started_with_policy(
        None,
        TEST_FEE_BPS,
        TEST_FIXED_FEE,
        FeeMode::SellerPays,
        TREASURY_LAMPORTS,
        &[(session.pubkey(), LAMPORTS_PER_SOL)],
    )
    .await;

    let buyer = context.payer.pubkey();
    let budget_address = budget_pda(&buyer);

    let open = Instruction {
        program_id: payvand::id(),
        accounts: vec![
            AccountMeta::new(buyer, true),
            AccountMeta::new(budget_address, false),
            AccountMeta::new(session.pubkey(), false),
            AccountMeta::new_readonly(config_pda(), false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: instruction_data("open_budget", &monthly_limit.to_le_bytes()),
    };
    send(&mut context, &[open], &[])
        .await
        .expect("open_budget failed");

    // The budget exists, is empty-spent, and holds the limit as its pot.
    let opened = budget(&context, budget_address).await;
    assert_eq!(opened.buyer, buyer);
    assert_eq!(opened.session_key, session.pubkey());
    assert_eq!(opened.monthly_limit, monthly_limit);
    assert_eq!(opened.spent, 0);
    let pot_before = lamports(&context, budget_address).await;
    assert!(pot_before >= monthly_limit);

    let seller = Keypair::new().pubkey();
    let reference = Keypair::new().pubkey();
    let amount = LAMPORTS_PER_SOL / 20;
    let memo = "silent coffee";
    let invoice = payment_pda(&seller, &reference, amount);
    let receipt_address = receipt_pda(&invoice);

    let pay = Instruction {
        program_id: payvand::id(),
        accounts: vec![
            AccountMeta::new(session.pubkey(), true),
            AccountMeta::new(budget_address, false),
            AccountMeta::new(invoice, false),
            AccountMeta::new(seller, false),
            AccountMeta::new(receipt_address, false),
            AccountMeta::new_readonly(reference, false),
            AccountMeta::new_readonly(config_pda(), false),
            AccountMeta::new(treasury_pda(), false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: instruction_data("pay_from_budget", &amount_and_memo(amount, memo)),
    };
    // The session key is the fee payer, as it is in the client.
    send_with_fee_payer(&mut context, &[pay], &session)
        .await
        .expect("pay_from_budget failed");

    // The seller absorbs the fee and the marker's rent, and the budget records
    // exactly what it spent on the invoice.
    assert_eq!(
        lamports(&context, seller).await,
        amount - fee(amount) - receipt_rent()
    );
    let spent = budget(&context, budget_address).await;
    assert_eq!(spent.spent, amount);
    assert_eq!(spent.monthly_limit, monthly_limit);

    // The pot pays exactly the price: the session key is refunded both rents and
    // the seller's share already carries them.
    let pot_after = lamports(&context, budget_address).await;
    assert_eq!(pot_before - pot_after, amount);

    // The treasury keeps the fee and funds nothing.
    let treasury_delta =
        lamports(&context, treasury_pda()).await as i128 - TREASURY_LAMPORTS as i128;
    assert_eq!(treasury_delta, fee(amount) as i128);

    // The invoice is closed and the receipt records the session key as payer.
    assert!(
        account(&context, invoice).await.is_none(),
        "the invoice account should be closed by the settle transaction"
    );
    let written = receipt(&context, receipt_address).await;
    assert_eq!(written.invoice, invoice);
    assert!(written.settled_at > 0);

    // The session key advanced both rents and was refunded both, so the only
    // thing it is out is its signature's fee — the float is what covers that.
    let session_balance = lamports(&context, session.pubkey()).await;
    assert_eq!(session_balance, LAMPORTS_PER_SOL - TX_FEE);
}

/// The limit is a ceiling, not a suggestion: once `spent` reaches it, the next
/// invoice is refused for the month, with the error that names the month rather
/// than the pot.
#[tokio::test]
async fn pay_from_budget_stops_at_the_monthly_limit() {
    let amount = LAMPORTS_PER_SOL / 20;
    let limit = amount * 2;
    let session = Keypair::new();
    let (mut context, _admin) = started_with_policy(
        None,
        TEST_FEE_BPS,
        TEST_FIXED_FEE,
        FeeMode::SellerPays,
        TREASURY_LAMPORTS,
        &[(session.pubkey(), LAMPORTS_PER_SOL)],
    )
    .await;

    let buyer = context.payer.pubkey();
    let budget_address = budget_pda(&buyer);
    send(
        &mut context,
        &[open_budget_instruction(buyer, session.pubkey(), limit)],
        &[],
    )
    .await
    .expect("open_budget failed");

    // No slack is added to the pot on purpose: it holds exactly the limit, so
    // the payment that reaches the limit also drains it to its last lamport.
    // The treasury credits the receipt-rent refund before the pot is checked,
    // which is what makes that possible.
    assert_eq!(
        lamports(&context, budget_address).await - budget_rent(),
        limit
    );

    pay_from_budget(&mut context, budget_address, &session, amount, "first")
        .await
        .expect("the first payment failed");
    pay_from_budget(
        &mut context,
        budget_address,
        &session,
        amount,
        "to the limit",
    )
    .await
    .expect("the payment that reaches the limit should settle");
    assert_eq!(budget(&context, budget_address).await.spent, limit);
    assert_eq!(
        lamports(&context, budget_address).await,
        budget_rent(),
        "the whole limit should be spendable, down to the pot's last lamport"
    );

    let over = pay_from_budget(&mut context, budget_address, &session, amount, "over")
        .await
        .expect_err("a payment past the limit must be refused");
    assert!(
        over.contains(BUDGET_EXHAUSTED),
        "expected BudgetExhausted, got {over}"
    );
    assert_eq!(
        budget(&context, budget_address).await.spent,
        limit,
        "a refused payment must not move the counter"
    );
}

/// The month is only one bound: the session key can never spend more than the
/// pot actually holds, even inside the limit, and that has its own error so the
/// page can say which resource is missing.
#[tokio::test]
async fn pay_from_budget_cannot_spend_more_than_the_pot() {
    let amount = LAMPORTS_PER_SOL / 20;
    let session = Keypair::new();
    let (mut context, _admin) = started_with_policy(
        None,
        TEST_FEE_BPS,
        TEST_FIXED_FEE,
        FeeMode::SellerPays,
        TREASURY_LAMPORTS,
        &[(session.pubkey(), LAMPORTS_PER_SOL)],
    )
    .await;

    let buyer = context.payer.pubkey();
    let budget_address = budget_pda(&buyer);
    send(
        &mut context,
        &[open_budget_instruction(buyer, session.pubkey(), amount * 4)],
        &[],
    )
    .await
    .expect("open_budget failed");

    // The month would allow it; the pot holds half the price.
    let held = account(&context, budget_address)
        .await
        .expect("budget account missing");
    context.set_account(
        &budget_address,
        &Account {
            lamports: budget_rent() + amount / 2,
            ..held
        }
        .into(),
    );

    let refused = pay_from_budget(&mut context, budget_address, &session, amount, "too big")
        .await
        .expect_err("a payment larger than the pot must be refused");
    assert!(
        refused.contains(INSUFFICIENT_BUDGET),
        "expected InsufficientBudget, got {refused}"
    );
    assert_eq!(budget(&context, budget_address).await.spent, 0);
}

/// A refill is `open_budget` again on the same account, so a mid-month top-up
/// must keep the counter it started with: resetting it would hand the buyer a
/// fresh allowance without a new month, and they could refill past their own
/// limit. Only a calendar-month change starts the count over.
#[tokio::test]
async fn open_budget_keeps_spent_until_the_month_changes() {
    let session = Keypair::new();
    let (mut context, _admin) = started_with_policy(
        None,
        TEST_FEE_BPS,
        TEST_FIXED_FEE,
        FeeMode::SellerPays,
        TREASURY_LAMPORTS,
        &[(session.pubkey(), LAMPORTS_PER_SOL)],
    )
    .await;

    let buyer = context.payer.pubkey();
    let budget_address = budget_pda(&buyer);
    let first_limit = LAMPORTS_PER_SOL / 4;
    send(
        &mut context,
        &[open_budget_instruction(
            buyer,
            session.pubkey(),
            first_limit,
        )],
        &[],
    )
    .await
    .expect("open_budget failed");
    assert_eq!(budget(&context, budget_address).await.spent, 0);

    // Spend part of the allowance, so the counter is no longer zero.
    let seller = Keypair::new().pubkey();
    let reference = Keypair::new().pubkey();
    let amount = LAMPORTS_PER_SOL / 20;
    refresh_blockhash(&mut context).await;
    send(
        &mut context,
        &[pay_from_budget_instruction(
            session.pubkey(),
            budget_address,
            seller,
            reference,
            amount,
            "silent coffee",
        )],
        &[&session],
    )
    .await
    .expect("pay_from_budget failed");
    let spent = budget(&context, budget_address).await;
    assert_eq!(spent.spent, amount);

    // The same month, a higher limit: the pot is topped up, the limit moves,
    // and what was already spent still counts.
    let second_limit = LAMPORTS_PER_SOL / 2;
    let pot_before = lamports(&context, budget_address).await;
    refresh_blockhash(&mut context).await;
    send(
        &mut context,
        &[open_budget_instruction(
            buyer,
            session.pubkey(),
            second_limit,
        )],
        &[],
    )
    .await
    .expect("the same-month refill failed");
    let refilled = budget(&context, budget_address).await;
    assert_eq!(refilled.spent, amount);
    assert_eq!(refilled.monthly_limit, second_limit);
    assert_eq!(refilled.month_index, spent.month_index);
    assert!(
        lamports(&context, budget_address).await >= pot_before,
        "a refill should reach the new limit, not take the pot backwards"
    );

    // A calendar month later, the same call starts the counter over.
    let mut clock: Clock = context
        .banks_client
        .get_sysvar()
        .await
        .expect("the clock sysvar should be readable");
    clock.unix_timestamp += 40 * 86_400;
    context.set_sysvar(&clock);
    refresh_blockhash(&mut context).await;
    send(
        &mut context,
        &[open_budget_instruction(
            buyer,
            session.pubkey(),
            second_limit,
        )],
        &[],
    )
    .await
    .expect("the next month's refill failed");
    let next_month = budget(&context, budget_address).await;
    assert_eq!(next_month.spent, 0);
    assert_ne!(next_month.month_index, refilled.month_index);
}

/// Setting a new limit has to leave the buyer the whole of it, which a top-up
/// alone cannot do: `open_budget` keeps `spent` on purpose. Closing the
/// allowance and opening it again in one transaction is what the app does
/// instead — one signature, the same session key, and the month starts over at
/// the limit they typed.
#[tokio::test]
async fn closing_and_reopening_budget_starts_the_month_over() {
    let session = Keypair::new();
    let (mut context, _admin) = started_with_policy(
        None,
        TEST_FEE_BPS,
        TEST_FIXED_FEE,
        FeeMode::SellerPays,
        TREASURY_LAMPORTS,
        &[(session.pubkey(), LAMPORTS_PER_SOL)],
    )
    .await;

    let buyer = context.payer.pubkey();
    let budget_address = budget_pda(&buyer);
    let first_limit = LAMPORTS_PER_SOL / 4;
    send(
        &mut context,
        &[open_budget_instruction(
            buyer,
            session.pubkey(),
            first_limit,
        )],
        &[],
    )
    .await
    .expect("open_budget failed");

    let amount = LAMPORTS_PER_SOL / 20;
    pay_from_budget(&mut context, budget_address, &session, amount, "first")
        .await
        .expect("pay_from_budget failed");
    assert_eq!(budget(&context, budget_address).await.spent, amount);

    // The same month, a bigger limit: close and open in the one transaction the
    // app sends, so the buyer never sits without an allowance in between.
    let second_limit = LAMPORTS_PER_SOL / 2;
    refresh_blockhash(&mut context).await;
    send(
        &mut context,
        &[
            close_budget_instruction(buyer),
            open_budget_instruction(buyer, session.pubkey(), second_limit),
        ],
        &[],
    )
    .await
    .expect("close and reopen failed");

    let reopened = budget(&context, budget_address).await;
    assert_eq!(reopened.spent, 0, "a new limit starts the month over");
    assert_eq!(reopened.monthly_limit, second_limit);
    assert_eq!(reopened.session_key, session.pubkey());
    assert_eq!(
        lamports(&context, budget_address).await - budget_rent(),
        second_limit
    );

    // The whole new limit is spendable, not just the part the old month left.
    pay_from_budget(
        &mut context,
        budget_address,
        &session,
        second_limit,
        "the whole new month",
    )
    .await
    .expect("the new limit should be spendable in full");
}

/// The same pair on the token path: the vault drains back into the buyer's own
/// account and is funded again to the new limit, in one transaction, with the
/// counter at zero.
#[tokio::test]
async fn closing_and_reopening_a_token_budget_starts_the_month_over() {
    let (mut context, world) = TokenWorld::start().await;
    open_token_budget(&mut context, &world, 500_000).await;
    pay_from_token_budget(&mut context, &world, Keypair::new().pubkey(), 200_000).await;
    assert_eq!(token_budget(&context, world.budget).await.spent, 200_000);

    refresh_blockhash(&mut context).await;
    send(
        &mut context,
        &[
            close_token_budget_instruction(
                world.buyer.pubkey(),
                world.budget,
                world.mint,
                world.vault,
                world.buyer_token_account,
            ),
            open_token_budget_instruction(
                world.buyer.pubkey(),
                world.budget,
                world.mint,
                world.buyer_token_account,
                world.vault,
                world.session.pubkey(),
                600_000,
            ),
        ],
        &[&world.buyer],
    )
    .await
    .expect("close and reopen failed");

    let reopened = token_budget(&context, world.budget).await;
    assert_eq!(reopened.spent, 0, "a new limit starts the month over");
    assert_eq!(reopened.monthly_limit, 600_000);
    assert_eq!(token_balance(&context, world.vault).await, 600_000);

    // More than the old month had left (500_000 - 200_000), so what is spendable
    // is the new limit and not the remainder of the old one.
    pay_from_token_budget(&mut context, &world, Keypair::new().pubkey(), 400_000).await;
}

/// The token budget runs the same rule, so it gets the same proof. The counter
/// is written into the account between opens — the exact state a payment leaves
/// behind — so the test is about what the next `open_token_budget` keeps or
/// clears, not about how the counter got there.
#[tokio::test]
async fn open_token_budget_keeps_spent_until_the_month_changes() {
    let mut test = program_test();
    let admin = Keypair::new();
    let buyer = Keypair::new();
    let session = Keypair::new();
    let mint = Keypair::new().pubkey();
    let buyer_token_account = Keypair::new().pubkey();
    let (budget_address, bump) = token_budget_pda(&buyer.pubkey(), &mint);
    let vault = associated_token_address(budget_address, mint);

    test.add_account(admin.pubkey(), system_account(LAMPORTS_PER_SOL / 10));
    test.add_account(buyer.pubkey(), system_account(LAMPORTS_PER_SOL));
    test.add_account(session.pubkey(), system_account(TEST_SESSION_FLOAT));
    test.add_account(treasury_pda(), system_account(TREASURY_LAMPORTS));
    test.add_account(mint, spl_mint_account(buyer.pubkey(), 6));
    test.add_account(
        buyer_token_account,
        spl_token_account(mint, buyer.pubkey(), 1_000),
    );
    // The vault already holds more than every limit below, so each open skips
    // its token transfer and the test stays inside the month rule.
    test.add_account(vault, spl_token_account(mint, budget_address, 1_000));

    let mut context = test.start_with_context().await;
    context.set_account(
        &program_data_address(),
        &program_data_account(Some(admin.pubkey())).into(),
    );
    send(
        &mut context,
        &[initialize_config_instruction(
            admin.pubkey(),
            TEST_FEE_BPS,
            TEST_FIXED_FEE,
            FeeMode::SellerPays,
            TEST_SESSION_FLOAT,
        )],
        &[&admin],
    )
    .await
    .expect("initialize_config failed");

    let open = |limit: u64| {
        open_token_budget_instruction(
            buyer.pubkey(),
            budget_address,
            mint,
            buyer_token_account,
            vault,
            session.pubkey(),
            limit,
        )
    };

    send(&mut context, &[open(50)], &[&buyer])
        .await
        .expect("open_token_budget failed");
    let opened = token_budget(&context, budget_address).await;
    assert_eq!(opened.spent, 0);
    assert_eq!(opened.monthly_limit, 50);
    assert_eq!(opened.bump, bump);

    // Mid-month, the same account has spent part of the limit.
    let spent = written_token_budget(
        buyer.pubkey(),
        session.pubkey(),
        mint,
        50,
        20,
        opened.month_index,
        bump,
    );
    context.set_account(&budget_address, &spent.into());
    refresh_blockhash(&mut context).await;
    send(&mut context, &[open(80)], &[&buyer])
        .await
        .expect("the same-month token refill failed");
    let refilled = token_budget(&context, budget_address).await;
    assert_eq!(refilled.spent, 20);
    assert_eq!(refilled.monthly_limit, 80);
    assert_eq!(refilled.month_index, opened.month_index);

    // A calendar month later the same open starts the counter over.
    let stale = written_token_budget(
        buyer.pubkey(),
        session.pubkey(),
        mint,
        80,
        20,
        opened.month_index - 1,
        bump,
    );
    context.set_account(&budget_address, &stale.into());
    refresh_blockhash(&mut context).await;
    send(&mut context, &[open(80)], &[&buyer])
        .await
        .expect("the next month's token refill failed");
    let next_month = token_budget(&context, budget_address).await;
    assert_eq!(next_month.spent, 0);
    assert_eq!(next_month.month_index, opened.month_index);
}

// ---------------------------------------------------------------------------
// Config, treasury, and the fee policy
// ---------------------------------------------------------------------------

#[tokio::test]
async fn initialize_config_records_the_upgrade_authority() {
    let mut test = program_test();
    let admin = Keypair::new();
    test.add_account(admin.pubkey(), system_account(LAMPORTS_PER_SOL / 10));
    test.add_account(treasury_pda(), system_account(TREASURY_LAMPORTS));
    let mut context = test.start_with_context().await;
    context.set_account(
        &program_data_address(),
        &program_data_account(Some(admin.pubkey())).into(),
    );

    let initialize = initialize_config_instruction(
        admin.pubkey(),
        TEST_FEE_BPS,
        TEST_FIXED_FEE,
        FeeMode::SellerPays,
        TEST_SESSION_FLOAT,
    );
    send(&mut context, std::slice::from_ref(&initialize), &[&admin])
        .await
        .expect("initialize_config failed");

    let written = config(&context, config_pda()).await;
    assert_eq!(written.admin, admin.pubkey());
    assert_eq!(written.policy.fee_bps_below, TEST_FEE_BPS);
    assert_eq!(written.policy.fee_bps_above, TEST_FEE_BPS);
    assert_eq!(written.policy.flat_usdc_base_units, TEST_FIXED_FEE);
    assert_eq!(written.fee_mode, FeeMode::SellerPays);
    assert_eq!(written.session_fee_float, TEST_SESSION_FLOAT);
    assert_eq!(written.version, CONFIG_VERSION);
    assert_eq!(
        account(&context, config_pda())
            .await
            .expect("config")
            .data
            .len(),
        CONFIG_SPACE
    );

    // One config per deployment: the PDA already exists.
    refresh_blockhash(&mut context).await;
    let second = send(&mut context, &[initialize], &[&admin]).await;
    assert!(second.is_err(), "a second initialize_config must fail");
}

#[tokio::test]
async fn initialize_config_rejects_a_signer_that_is_not_the_upgrade_authority() {
    let mut test = program_test();
    let stranger = Keypair::new();
    test.add_account(stranger.pubkey(), system_account(LAMPORTS_PER_SOL / 10));
    test.add_account(treasury_pda(), system_account(TREASURY_LAMPORTS));
    let mut context = test.start_with_context().await;
    // The program's authority is somebody else entirely.
    let authority = Keypair::new().pubkey();
    context.set_account(
        &program_data_address(),
        &program_data_account(Some(authority)).into(),
    );

    let initialize = initialize_config_instruction(
        stranger.pubkey(),
        TEST_FEE_BPS,
        TEST_FIXED_FEE,
        FeeMode::SellerPays,
        TEST_SESSION_FLOAT,
    );
    let attempt = send(&mut context, &[initialize], &[&stranger]).await;
    let error = attempt.expect_err("a non-authority signer must not install a config");
    assert!(
        error.contains(INVALID_CONFIG_ADMIN),
        "expected {INVALID_CONFIG_ADMIN}, got {error}"
    );
    assert!(
        account(&context, config_pda()).await.is_none(),
        "a rejected bootstrap must not leave a config behind"
    );
}

/// The SOL path asks the treasury for nothing since the seller started carrying
/// the marker's rent: a treasury holding one lamport settles as well as a funded
/// one, and the only lamports it sees are the fee arriving. The price is half a
/// SOL rather than a tenth because the fee is the rate *less* the marker's rent
/// now, and a fee that lands on a bare system account has to leave it rent
/// exempt — a tenth of a SOL's fee no longer does.
#[tokio::test]
async fn settle_payment_needs_nothing_of_the_treasury() {
    let mut context = started_with_policy(
        None,
        TEST_FEE_BPS,
        TEST_FIXED_FEE,
        FeeMode::SellerPays,
        1,
        &[],
    )
    .await
    .0;
    let payer = context.payer.pubkey();
    let seller = Keypair::new().pubkey();
    let reference = Keypair::new().pubkey();
    let amount = LAMPORTS_PER_SOL / 2;

    send(
        &mut context,
        &[settle_instruction(
            payer, seller, reference, amount, "coffee",
        )],
        &[],
    )
    .await
    .expect("an unfunded treasury must not block a SOL settlement");

    assert_eq!(
        lamports(&context, seller).await,
        amount - fee(amount) - receipt_rent()
    );
    assert_eq!(
        lamports(&context, treasury_pda()).await,
        1 + fee(amount),
        "the treasury only receives the fee"
    );
}

#[tokio::test]
async fn settle_payment_rejects_a_substituted_treasury() {
    let mut context = started().await;
    let payer = context.payer.pubkey();
    let seller = Keypair::new().pubkey();
    let reference = Keypair::new().pubkey();
    let amount = LAMPORTS_PER_SOL / 10;
    let decoy = Keypair::new().pubkey();

    let attempt = send(
        &mut context,
        &[settle_instruction_with_treasury(
            payer, seller, reference, amount, "coffee", decoy,
        )],
        &[],
    )
    .await;
    let error = attempt.expect_err("the treasury is addressed by seed, not by the caller");
    // Custom(2006) is Anchor's `ConstraintSeeds`.
    assert!(
        error.contains("Custom(2006)"),
        "expected a seeds constraint failure, got {error}"
    );
}

#[tokio::test]
async fn buyer_pays_policy_is_rejected_before_it_is_implemented() {
    let mut context = started_with_policy(
        None,
        TEST_FEE_BPS,
        TEST_FIXED_FEE,
        FeeMode::BuyerPays,
        TREASURY_LAMPORTS,
        &[],
    )
    .await
    .0;
    let payer = context.payer.pubkey();
    let seller = Keypair::new().pubkey();
    let reference = Keypair::new().pubkey();
    let amount = LAMPORTS_PER_SOL / 10;

    let attempt = send(
        &mut context,
        &[settle_instruction(
            payer, seller, reference, amount, "coffee",
        )],
        &[],
    )
    .await;
    let error = attempt.expect_err("only SellerPays settles in this release");
    assert!(
        error.contains(FEE_MODE_UNSUPPORTED),
        "expected {FEE_MODE_UNSUPPORTED}, got {error}"
    );
}

/// The percentage has a ceiling in every currency: a large invoice pays at most
/// 0.1 SOL, 10 USDC or 10 EURC. Without it the rate would keep scaling with the
/// price, so a 10 SOL sale would cost 0.2 SOL at 2% however routine the work is.
/// The ceiling is the program's: no rate in a policy can raise it.
#[test]
fn the_fee_has_a_ceiling_in_every_currency() {
    let rent = receipt_rent();
    let policy = test_policy(TEST_FEE_BPS, 89_400);

    // 10 SOL at 2% is 0.2 SOL; the ceiling takes it to 0.1 SOL exactly.
    let sol_amount = 10 * LAMPORTS_PER_SOL;
    assert_eq!(
        fee_for(&policy, sol_amount, None, rent).expect("fits"),
        MAX_SOL_FEE_LAMPORTS
    );
    // Below the ceiling the rate is untouched: 1 SOL still costs 2% less the
    // record rent its seller already paid.
    assert_eq!(
        fee_for(&policy, LAMPORTS_PER_SOL, None, rent).expect("fits"),
        20_000_000 - rent
    );

    // 1,000 USDC or EURC at 2% is 20 units; ten is the most either pays.
    let token_amount = 1_000_000_000u64;
    assert_eq!(
        fee_for(&policy, token_amount, Some(&usdc_mint()), rent,).expect("fits"),
        MAX_TOKEN_FEE_BASE_UNITS
    );
    assert_eq!(
        fee_for(&policy, token_amount, Some(&eurc_mint()), rent,).expect("fits"),
        MAX_TOKEN_FEE_BASE_UNITS
    );

    // 40 SOL at any rate above 0.25% lands on the ceiling: 0.4 SOL of fee is
    // over it whatever the rate did on the way there.
    assert_eq!(
        fee_for(&policy, 40 * LAMPORTS_PER_SOL, None, rent).expect("fits"),
        MAX_SOL_FEE_LAMPORTS
    );

    // A whole-invoice rate on 1 SOL reaches it too, so the ceiling is the cap in
    // a live policy's whole range, not only above the two rates' usual sizes.
    assert_eq!(
        fee_for(&test_policy(10_000, 89_400), LAMPORTS_PER_SOL, None, rent,).expect("fits"),
        MAX_SOL_FEE_LAMPORTS
    );
}

/// The record's rent is the floor of what a seller pays, in both currencies.
///
/// 2% of a price below the rent is less than the rent, so the token path charges
/// the flat — the record and nothing more — and the SOL path charges no fee,
/// because its seller is already handing over that rent. Above the crossover the
/// percentage takes over, and a mint the policy does not name has no record cost
/// to cover.
#[test]
fn the_record_fee_is_the_floor_of_the_fee() {
    let rent = receipt_rent();
    let flat = 89_400u64;
    let policy = test_policy(TEST_FEE_BPS, flat);

    // 1 token: 2% is 20,000 base units, under the rent. 100,000,000 lamports:
    // 2% is 2,000,000, still under it at 1,224,960.
    assert_eq!(
        fee_for(&policy, 1_000_000, Some(&usdc_mint()), rent,).expect("fits"),
        flat
    );
    assert_eq!(fee_for(&policy, 50_000_000, None, rent).expect("fits"), 0);

    // Above the crossover the rate is the larger share, and the SOL side takes
    // the rent off it because the seller pays that rent separately.
    assert_eq!(
        fee_for(&policy, 10_000_000, Some(&usdc_mint()), rent,).expect("fits"),
        200_000
    );
    assert_eq!(
        fee_for(&policy, 10_000_000, Some(&eurc_mint()), rent,).expect("fits"),
        200_000
    );
    assert_eq!(
        fee_for(&policy, 1_000_000_000, None, rent).expect("fits"),
        20_000_000 - rent
    );

    // A mint that is neither currency has no flat to pay, so it keeps the rate.
    let other = Pubkey::new_from_array([3u8; 32]);
    assert_eq!(
        fee_for(&policy, 1_000_000, Some(&other), rent).expect("fits"),
        20_000
    );

    // The cap still guards the small end: a 100% policy cannot take the price.
    let total = test_policy(10_000, 0);
    assert_eq!(
        fee_for(&total, 1_000_000, Some(&usdc_mint()), rent,).expect("fits"),
        999_999
    );
}

/// The 0.1 SOL / 10-token boundary is a change of slope, not a cliff: the part
/// at or below it pays the lower rate and the part above pays the higher one, so
/// one lamport past the boundary costs one lamport's worth of rate more.
#[test]
fn the_tier_boundary_is_marginal() {
    let policy = config_with_policy(tiered_policy());

    // 1% up to and including the boundary.
    assert_eq!(
        fee_for(&policy, 99_999_999, None, 0).expect("fits"),
        999_999
    );
    assert_eq!(
        fee_for(&policy, 100_000_000, None, 0).expect("fits"),
        1_000_000
    );
    // One lamport past it owes 1% of 0.1 SOL plus 0.5% of that lamport, floored.
    assert_eq!(
        fee_for(&policy, 100_000_001, None, 0).expect("fits"),
        1_000_000
    );
    // 0.2 SOL: 1,000,000 plus 0.5% of the second 0.1 SOL.
    assert_eq!(
        fee_for(&policy, 200_000_000, None, 0).expect("fits"),
        1_500_000
    );

    // The token boundary is its own number and follows the same rule. The flat
    // is not added on top: it is a floor, and at these sizes the rate is above
    // it.
    assert_eq!(
        fee_for(&policy, 10_000_000, Some(&usdc_mint()), 0,).expect("fits"),
        100_000
    );
    assert_eq!(
        fee_for(&policy, 20_000_000, Some(&usdc_mint()), 0,).expect("fits"),
        150_000
    );
}

/// The token floor is the record's rent **plus the network fee**: a payment too
/// small for the percentage to cover both is charged exactly those costs, one
/// large enough for the percentage to cover them is charged the percentage, and
/// the SOL paths are untouched until a relayer pays their fee.
#[test]
fn the_token_floor_is_the_record_rent_plus_the_network_fee() {
    let mut policy = test_policy_of(200, 89_400);
    policy.network_fee_lamports = 5_000;
    policy.network_fee_usdc_base_units = 2_000;
    policy.network_fee_eurc_base_units = 1_800;
    let config = config_with_policy(policy);

    // 2% of 1,000,000 is 20,000, below the 89,400 + 2,000 the record and the fee
    // cost, so the seller pays the costs and the protocol takes nothing.
    assert_eq!(
        fee_for(&config, 1_000_000, Some(&usdc_mint()), 0).expect("fits"),
        91_400
    );
    // The EURC pair is its own: the same rent, its own fee.
    assert_eq!(
        fee_for(&config, 1_000_000, Some(&eurc_mint()), 0).expect("fits"),
        91_200
    );
    // Ten units of the stablecoin: 2% is 200,000, above the floor, so the seller
    // pays the percentage and the costs come out of it.
    assert_eq!(
        fee_for(&config, 10_000_000, Some(&usdc_mint()), 0,).expect("fits"),
        200_000
    );
    // The SOL path's two costs are both lamports: the marker's rent comes off the
    // percentage and the network fee floors what is left.
    assert_eq!(
        fee_for(&config, 10_000_000, None, 0).expect("fits"),
        200_000
    );
    // With a rent the percentage cannot cover, the SOL fee is the network fee
    // alone: the seller pays that plus the rent, and no percentage.
    let mut sol_policy = test_policy_of(200, 89_400);
    sol_policy.network_fee_lamports = 5_000;
    let sol_config = config_with_policy(sol_policy);
    assert_eq!(
        fee_for(&sol_config, 1_000_000, None, receipt_rent(),).expect("fits"),
        5_000,
        "below the costs, the seller pays the network fee and no percentage"
    );
    // With a percentage that covers them, the fee is the percentage less the
    // marker's rent, and the platform keeps what the network fee leaves.
    assert_eq!(
        fee_for(&sol_config, LAMPORTS_PER_SOL / 10, None, receipt_rent(),).expect("fits"),
        2_000_000 - receipt_rent()
    );
    // The policy names one signature, and a sponsored settle costs two: the
    // floor a relayer settlement charges (and refunds) is doubled, not guessed.
    assert_eq!(
        fee_for_signatures(&sol_config, 1_000_000, None, receipt_rent(), 2).expect("fits"),
        2 * 5_000
    );
}

/// `migrate_config` upgrades the live version 1 account in place: the layout
/// grows from 62 to 160 bytes, the fields that outlive it are carried over, and
/// the admin pays only the rent the taller account now needs.
#[tokio::test]
async fn migrate_config_upgrades_the_v1_layout_in_place() {
    let mut test = program_test();
    let admin = Keypair::new();
    test.add_account(admin.pubkey(), system_account(LAMPORTS_PER_SOL / 10));
    test.add_account(treasury_pda(), system_account(TREASURY_LAMPORTS));
    let mut context = test.start_with_context().await;

    let (address, bump) = Pubkey::find_program_address(&[CONFIG_SEED], &payvand::id());
    context.set_account(
        &address,
        &v1_config_account(admin.pubkey(), bump, 254, 30_000_000).into(),
    );
    refresh_blockhash(&mut context).await;
    let before = account(&context, address)
        .await
        .expect("config account missing")
        .lamports;

    // A funded stranger that is not the recorded admin cannot migrate it, and
    // the refused attempt leaves the old layout untouched.
    let stranger = Keypair::new();
    context.set_account(
        &stranger.pubkey(),
        &system_account(LAMPORTS_PER_SOL / 10).into(),
    );
    refresh_blockhash(&mut context).await;
    let policy = tiered_policy();
    let attempt = send(
        &mut context,
        &[migrate_config_instruction(stranger.pubkey(), policy)],
        &[&stranger],
    )
    .await;
    let error = attempt.expect_err("only the recorded admin may migrate");
    assert!(
        error.contains(INVALID_CONFIG_ADMIN),
        "expected {INVALID_CONFIG_ADMIN}, got {error}"
    );
    assert_eq!(
        account(&context, address)
            .await
            .expect("config account missing")
            .data
            .len(),
        CONFIG_SPACE_V1,
        "a refused migration leaves the account alone"
    );

    send(
        &mut context,
        &[migrate_config_instruction(admin.pubkey(), policy)],
        &[&admin],
    )
    .await
    .expect("the admin migration should succeed");

    let migrated = account(&context, address)
        .await
        .expect("config account missing");
    assert_eq!(migrated.data.len(), CONFIG_SPACE);
    assert!(
        migrated.lamports >= Rent::default().minimum_balance(CONFIG_SPACE),
        "the taller account is left rent exempt"
    );
    assert!(migrated.lamports > before, "the admin paid the difference");

    let migrated = config(&context, address).await;
    assert_eq!(migrated.admin, admin.pubkey());
    // Everything the policy carries is installed except the two token flats: a
    // migration zeroes them, so the first refresh has no stale price to be
    // bounded against (see the refresh below).
    assert_eq!(migrated.policy.fee_bps_below, policy.fee_bps_below);
    assert_eq!(migrated.policy.fee_bps_above, policy.fee_bps_above);
    assert_eq!(migrated.policy.sol_tier_lamports, policy.sol_tier_lamports);
    assert_eq!(
        migrated.policy.token_tier_base_units,
        policy.token_tier_base_units
    );
    assert_eq!(migrated.policy.flat_usdc_base_units, 0);
    assert_eq!(migrated.policy.flat_eurc_base_units, 0);
    assert_eq!(
        migrated.policy.network_fee_usdc_base_units, 0,
        "the fee's token equivalents are priced, not migrated"
    );
    assert_eq!(
        migrated.policy.network_fee_lamports, policy.network_fee_lamports,
        "what a signature costs is policy, not a price, so it is carried over"
    );
    assert_eq!(migrated.fee_mode, FeeMode::SellerPays);
    assert_eq!(migrated.session_fee_float, 30_000_000);
    assert_eq!(migrated.flats_updated_at, 0, "no cooldown after migrating");
    assert_eq!(migrated.bump, bump);
    assert_eq!(migrated.treasury_bump, 254);
    assert_eq!(migrated.version, CONFIG_VERSION);

    // A migrated account carries no price anchor — that account postdates the
    // layouts this migration upgrades — so a permissionless refresh has no band
    // to be measured against until the admin writes one.
    let caller = Keypair::new();
    context.set_account(
        &caller.pubkey(),
        &system_account(LAMPORTS_PER_SOL / 10).into(),
    );
    refresh_blockhash(&mut context).await;
    let unanchored = send(
        &mut context,
        &[refresh_fee_price_instruction(
            caller.pubkey(),
            67_000_000,
            62_000_000,
        )],
        &[&caller],
    )
    .await;
    assert!(
        unanchored.is_err(),
        "a refresh before the admin anchors a price has no band"
    );

    // The admin's anchor prices both zeroed flats from the cluster's own rent,
    // so the migration is one admin call away from being priced.
    let rent = receipt_rent();
    refresh_blockhash(&mut context).await;
    send(
        &mut context,
        &[set_fee_price_anchor_instruction(
            admin.pubkey(),
            67_000_000,
            62_000_000,
        )],
        &[&admin],
    )
    .await
    .expect("the admin anchors the price after a migration");
    let pegged = config(&context, address).await;
    assert_eq!(
        pegged.policy.flat_usdc_base_units,
        flat_at(rent, 67_000_000)
    );
    assert_eq!(
        pegged.policy.flat_eurc_base_units,
        flat_at(rent, 62_000_000)
    );
    assert!(pegged.flats_updated_at > 0);

    // A version 3 account is neither of the layouts the migration reads: it is
    // not rerun over what it just wrote. A fresh blockhash keeps this from
    // deduping against the successful migration above.
    refresh_blockhash(&mut context).await;
    let again = send(
        &mut context,
        &[migrate_config_instruction(admin.pubkey(), policy)],
        &[&admin],
    )
    .await;
    let error = again.expect_err("a migrated account is not migrated twice");
    assert!(
        error.contains(INVALID_CONFIG_VERSION),
        "expected {INVALID_CONFIG_VERSION}, got {error}"
    );
}

/// A live version 3 config migrates to version 4: the layout grows by the
/// network fee, the rate the admin passes is installed, and the four
/// price-derived fields wait for the anchor that prices them.
#[tokio::test]
async fn migrate_config_upgrades_the_v3_layout_and_keeps_its_network_fee() {
    let mut test = program_test();
    let admin = Keypair::new();
    test.add_account(admin.pubkey(), system_account(LAMPORTS_PER_SOL / 10));
    test.add_account(treasury_pda(), system_account(TREASURY_LAMPORTS));
    let mut context = test.start_with_context().await;

    let (address, bump) = Pubkey::find_program_address(&[CONFIG_SEED], &payvand::id());
    let old = tiered_policy();
    context.set_account(
        &address,
        &v3_config_account(admin.pubkey(), bump, 254, 30_000_000, &old).into(),
    );

    // The admin names the network fee the new layout carries; the price-derived
    // fields it passes are ignored.
    let mut policy = tiered_policy();
    policy.network_fee_lamports = 5_000;
    policy.network_fee_usdc_base_units = 999_999;
    policy.network_fee_eurc_base_units = 999_999;

    send(
        &mut context,
        &[migrate_config_instruction(admin.pubkey(), policy)],
        &[&admin],
    )
    .await
    .expect("a version 3 config should migrate");

    let migrated = account(&context, address)
        .await
        .expect("config account missing");
    assert_eq!(migrated.data.len(), CONFIG_SPACE);
    let migrated = config(&context, address).await;
    assert_eq!(migrated.version, CONFIG_VERSION);
    assert_eq!(migrated.policy.fee_bps_below, policy.fee_bps_below);
    assert_eq!(migrated.policy.network_fee_lamports, 5_000);
    assert_eq!(migrated.policy.network_fee_usdc_base_units, 0);
    assert_eq!(migrated.policy.network_fee_eurc_base_units, 0);
    assert_eq!(migrated.session_fee_float, 30_000_000);
    assert_eq!(migrated.bump, bump);

    // The anchor prices the network fee's token equivalents beside the flats,
    // from the same SOL price.
    let rent = receipt_rent();
    send(
        &mut context,
        &[set_fee_price_anchor_instruction(
            admin.pubkey(),
            67_000_000,
            62_000_000,
        )],
        &[&admin],
    )
    .await
    .expect("the admin anchors the price");
    let pegged = config(&context, address).await;
    assert_eq!(
        pegged.policy.flat_usdc_base_units,
        flat_at(rent, 67_000_000)
    );
    assert_eq!(
        pegged.policy.network_fee_usdc_base_units,
        flat_at(5_000, 67_000_000)
    );
    assert_eq!(
        pegged.policy.network_fee_eurc_base_units,
        flat_at(5_000, 62_000_000)
    );
}

/// The token flats track the cluster's rent and a live SOL price, and the call
/// that rewrites them is bounded by a band measured from the admin's anchor and
/// a cooldown.
///
/// The config is initialized with the production flat (89,400 base units per
/// six-decimal token), which implies a SOL price of about €73. The admin anchors
/// $67 / €62: a caller's price outside `PRICE_ANCHOR_BAND_BPS` of that is refused
/// by name, an honest one writes the rent at its price, and the same caller
/// cannot do it twice in a minute.
#[tokio::test]
async fn refresh_fee_price_repegs_the_flats_within_its_bounds() {
    let (mut context, admin) = started_with_policy(
        None,
        100,
        89_400,
        FeeMode::SellerPays,
        TREASURY_LAMPORTS,
        &[],
    )
    .await;
    let rent = receipt_rent();
    let usd = 67_000_000u64;
    let eur = 62_000_000u64;

    // The band is measured from a price only the admin writes.
    send(
        &mut context,
        &[set_fee_price_anchor_instruction(admin.pubkey(), usd, eur)],
        &[&admin],
    )
    .await
    .expect("the admin anchors the price");

    // Anyone may call the refresh: this key is a stranger to the config.
    let caller = Keypair::new();
    context.set_account(
        &caller.pubkey(),
        &system_account(LAMPORTS_PER_SOL / 10).into(),
    );
    // The anchor's own write stamped a refresh, so walk past that cooldown.
    advance_clock(&mut context, FLATS_REFRESH_SECS).await;

    // A price nowhere near the anchor ($1 a SOL) is refused by name.
    let wild = send(
        &mut context,
        &[refresh_fee_price_instruction(
            caller.pubkey(),
            1_000_000,
            1_000_000,
        )],
        &[&caller],
    )
    .await;
    let error = wild.expect_err("a price outside the band must be refused");
    assert!(
        error.contains(INVALID_FEE_PRICE),
        "expected {INVALID_FEE_PRICE}, got {error}"
    );

    // The honest one writes the rent at that price, in each currency's units.
    send(
        &mut context,
        &[refresh_fee_price_instruction(caller.pubkey(), usd, eur)],
        &[&caller],
    )
    .await
    .expect("an honest refresh should be accepted");

    let refreshed = config(&context, config_pda()).await;
    assert_eq!(refreshed.policy.flat_usdc_base_units, flat_at(rent, usd));
    assert_eq!(refreshed.policy.flat_eurc_base_units, flat_at(rent, eur));
    // A zero network fee is priced at zero in both currencies, which is what the
    // default suite settles at.
    assert_eq!(refreshed.policy.network_fee_usdc_base_units, 0);
    assert_eq!(refreshed.policy.network_fee_eurc_base_units, 0);
    assert_eq!(
        refreshed.policy.flat_eurc_base_units, 75_947,
        "the rent at EUR 62 in six-decimal units"
    );
    assert!(refreshed.flats_updated_at > 0);
    assert_eq!(refreshed.version, CONFIG_VERSION);

    // And a second call inside the cooldown is refused even at the same price.
    refresh_blockhash(&mut context).await;
    let again = send(
        &mut context,
        &[refresh_fee_price_instruction(caller.pubkey(), usd, eur)],
        &[&caller],
    )
    .await;
    let error = again.expect_err("a second refresh within a minute must be refused");
    assert!(
        error.contains(FEE_PRICE_TOO_SOON),
        "expected {FEE_PRICE_TOO_SOON}, got {error}"
    );
    assert!(FLATS_REFRESH_SECS >= 60, "the cooldown is a real one");
    assert!(
        PRICE_ANCHOR_BAND_BPS < 5_000,
        "the band is a bound, not a formality"
    );
}

/// The band is measured from the admin's anchor, not from the flat the previous
/// refresh wrote, so repeated permissionless calls cannot walk the flats away.
///
/// One accepted refresh takes the price to 9% above the anchor. Every later call
/// then prices another 2% higher: each is within `PRICE_ANCHOR_BAND_BPS` of the
/// flat that call would start from — a relative band would have accepted all
/// four and drifted the record fee about 17% — and each is refused because it is
/// outside the band around the anchor. The flat and the refresh stamp survive
/// every refusal unchanged.
#[tokio::test]
async fn refresh_fee_price_cannot_walk_the_flats_away_from_the_anchor() {
    let (mut context, admin) = started_with_policy(
        None,
        100,
        89_400,
        FeeMode::SellerPays,
        TREASURY_LAMPORTS,
        &[],
    )
    .await;
    let rent = receipt_rent();
    let usd = 67_000_000u64;
    let eur = 62_000_000u64;

    // Only the admin chooses the anchor a refresh is measured against.
    let stranger = Keypair::new();
    context.set_account(
        &stranger.pubkey(),
        &system_account(LAMPORTS_PER_SOL / 10).into(),
    );
    refresh_blockhash(&mut context).await;
    let refused = send(
        &mut context,
        &[set_fee_price_anchor_instruction(
            stranger.pubkey(),
            usd,
            eur,
        )],
        &[&stranger],
    )
    .await;
    let error = refused.expect_err("a stranger cannot choose the anchor");
    assert!(
        error.contains(HAS_ONE_FAILED),
        "expected {HAS_ONE_FAILED}, got {error}"
    );

    send(
        &mut context,
        &[set_fee_price_anchor_instruction(admin.pubkey(), usd, eur)],
        &[&admin],
    )
    .await
    .expect("the admin anchors the price");
    assert_eq!(
        config(&context, config_pda())
            .await
            .policy
            .flat_usdc_base_units,
        flat_at(rent, usd),
        "anchoring prices the flats too"
    );

    let caller = Keypair::new();
    context.set_account(
        &caller.pubkey(),
        &system_account(LAMPORTS_PER_SOL / 10).into(),
    );

    // One honest refresh, 9% above the anchor, is inside the band and lands.
    let accepted = price_above(usd, 9);
    advance_clock(&mut context, FLATS_REFRESH_SECS).await;
    send(
        &mut context,
        &[refresh_fee_price_instruction(
            caller.pubkey(),
            accepted,
            price_above(eur, 9),
        )],
        &[&caller],
    )
    .await
    .expect("a price inside the anchor band is accepted");
    let walked = config(&context, config_pda()).await;
    assert_eq!(walked.policy.flat_usdc_base_units, flat_at(rent, accepted));
    let stamped = walked.flats_updated_at;

    for step in [11u64, 13, 15, 17] {
        let price = price_above(usd, step);
        // What the flat on chain implies, the price the old relative band would
        // have measured from: every step below is well within 10% of it.
        let implied = walked.policy.flat_usdc_base_units as u128 * 1_000_000_000 / rent as u128;
        assert!(
            price as u128 * 10_000 <= implied * 11_000,
            "step {step}% is inside the old band around the current flat"
        );

        advance_clock(&mut context, FLATS_REFRESH_SECS).await;
        let attempt = send(
            &mut context,
            &[refresh_fee_price_instruction(
                caller.pubkey(),
                price,
                price_above(eur, step),
            )],
            &[&caller],
        )
        .await;
        let error = attempt.expect_err("a step outside the anchor band is refused");
        assert!(
            error.contains(INVALID_FEE_PRICE),
            "expected {INVALID_FEE_PRICE} at step {step}%, got {error}"
        );

        let after = config(&context, config_pda()).await;
        assert_eq!(
            after.policy.flat_usdc_base_units,
            flat_at(rent, accepted),
            "a refused step must not move the flat"
        );
        assert_eq!(after.flats_updated_at, stamped);
    }
}

#[tokio::test]
async fn update_config_requires_the_admin_and_a_valid_policy() {
    let (mut context, _admin) = {
        let mut test = program_test();
        let admin = Keypair::new();
        test.add_account(admin.pubkey(), system_account(LAMPORTS_PER_SOL / 10));
        test.add_account(treasury_pda(), system_account(TREASURY_LAMPORTS));
        let mut context = test.start_with_context().await;
        context.set_account(
            &program_data_address(),
            &program_data_account(Some(admin.pubkey())).into(),
        );
        send(
            &mut context,
            &[initialize_config_instruction(
                admin.pubkey(),
                TEST_FEE_BPS,
                TEST_FIXED_FEE,
                FeeMode::SellerPays,
                TEST_SESSION_FLOAT,
            )],
            &[&admin],
        )
        .await
        .expect("initialize_config failed");
        (context, admin)
    };
    let admin_keypair = _admin;
    let admin = admin_keypair.pubkey();

    // A funded stranger cannot rewrite the policy.
    let stranger = Keypair::new();
    context.set_account(
        &stranger.pubkey(),
        &system_account(LAMPORTS_PER_SOL / 10).into(),
    );
    let attempt = send(
        &mut context,
        &[update_config_instruction(
            stranger.pubkey(),
            TEST_FEE_BPS,
            0,
            FeeMode::SellerPays,
            TEST_SESSION_FLOAT,
        )],
        &[&stranger],
    )
    .await;
    let error = attempt.expect_err("only the admin may update the policy");
    // Custom(2001) is Anchor's `ConstraintHasOne`.
    assert!(
        error.contains("Custom(2001)"),
        "expected an admin constraint failure, got {error}"
    );

    // The admin can, and the change is what settlements read afterwards.
    send(
        &mut context,
        &[update_config_instruction(
            admin,
            100,
            0,
            FeeMode::SellerPays,
            TEST_SESSION_FLOAT,
        )],
        &[&admin_keypair],
    )
    .await
    .expect("the admin update should succeed");
    let updated = config(&context, config_pda()).await;
    assert_eq!(updated.policy.fee_bps_below, 100);
    assert_eq!(
        updated.admin, admin,
        "update_config never rewrites the admin"
    );

    // Invalid policy values are named errors.
    let too_much = send(
        &mut context,
        &[update_config_instruction(
            admin,
            10_001,
            0,
            FeeMode::SellerPays,
            TEST_SESSION_FLOAT,
        )],
        &[&admin_keypair],
    )
    .await;
    let error = too_much.expect_err("a fee above 100% must be refused");
    assert!(
        error.contains(INVALID_FEE_BPS),
        "expected {INVALID_FEE_BPS}, got {error}"
    );

    let float_too_small = send(
        &mut context,
        &[update_config_instruction(
            admin,
            100,
            0,
            FeeMode::SellerPays,
            1,
        )],
        &[&admin_keypair],
    )
    .await;
    let error = float_too_small.expect_err("a float that cannot cover one payment must be refused");
    assert!(
        error.contains("Custom(6013)"),
        "expected Custom(6013), got {error}"
    );

    // A byte that is not a `FeeMode` variant never reaches the handler: Borsh
    // rejects the instruction data, which is the named error for a bad mode.
    let mut raw = 100u16.to_le_bytes().to_vec();
    raw.extend_from_slice(&0u64.to_le_bytes());
    raw.push(9);
    raw.extend_from_slice(&TEST_SESSION_FLOAT.to_le_bytes());
    let bad_mode = Instruction {
        program_id: payvand::id(),
        accounts: vec![
            AccountMeta::new_readonly(admin, true),
            AccountMeta::new(config_pda(), false),
        ],
        data: instruction_data("update_config", &raw),
    };
    let error = send(&mut context, &[bad_mode], &[&admin_keypair])
        .await
        .expect_err("an unknown fee mode must be refused");
    // Custom(102) is Anchor's `InstructionDidNotDeserialize`.
    assert!(
        error.contains("Custom(102)"),
        "expected an undecodable instruction, got {error}"
    );
}

/// The float floor is exactly `R_p + R_r`: a config that can front one payment
/// passes, one lamport less does not. Pinned as a boundary so a receipt or
/// invoice size change cannot quietly leave the floor too low.
#[tokio::test]
async fn session_float_floor_accepts_the_exact_rent_sum_and_rejects_one_lamport_less() {
    let (mut context, admin_keypair) = started_with_policy(
        None,
        TEST_FEE_BPS,
        TEST_FIXED_FEE,
        FeeMode::SellerPays,
        TREASURY_LAMPORTS,
        &[],
    )
    .await;
    let admin = admin_keypair.pubkey();

    // The floor itself is a valid policy.
    send(
        &mut context,
        &[update_config_instruction(
            admin,
            TEST_FEE_BPS,
            TEST_FIXED_FEE,
            FeeMode::SellerPays,
            MIN_SESSION_FEE_FLOAT,
        )],
        &[&admin_keypair],
    )
    .await
    .expect("the exact floor must be accepted");
    assert_eq!(
        config(&context, config_pda()).await.session_fee_float,
        MIN_SESSION_FEE_FLOAT
    );

    // The floor is also the invoice plus receipt rent, read from the program's
    // own account sizes rather than a copied constant.
    assert_eq!(MIN_SESSION_FEE_FLOAT, invoice_rent() + receipt_rent());

    // One lamport less cannot front the payment it exists for.
    let under = send(
        &mut context,
        &[update_config_instruction(
            admin,
            TEST_FEE_BPS,
            TEST_FIXED_FEE,
            FeeMode::SellerPays,
            MIN_SESSION_FEE_FLOAT - 1,
        )],
        &[&admin_keypair],
    )
    .await;
    let error = under.expect_err("one lamport below the floor must be refused");
    assert!(
        error.contains("Custom(6013)"),
        "expected InvalidSessionFloat, got {error}"
    );
    // The refused update changed nothing.
    assert_eq!(
        config(&context, config_pda()).await.session_fee_float,
        MIN_SESSION_FEE_FLOAT
    );
}

#[tokio::test]
async fn zero_fee_policy_charges_no_fee() {
    let mut context = started_with_policy(None, 0, 0, FeeMode::SellerPays, TREASURY_LAMPORTS, &[])
        .await
        .0;
    let payer = context.payer.pubkey();
    let seller = Keypair::new().pubkey();
    let reference = Keypair::new().pubkey();
    let amount = LAMPORTS_PER_SOL / 10;
    let treasury_before = lamports(&context, treasury_pda()).await;

    send(
        &mut context,
        &[settle_instruction(
            payer, seller, reference, amount, "pilot",
        )],
        &[],
    )
    .await
    .expect("a fee-less pilot settle failed");

    assert_eq!(lamports(&context, seller).await, amount - receipt_rent());
    // A zero fee leaves the treasury alone: the seller funds the marker.
    assert_eq!(lamports(&context, treasury_pda()).await, treasury_before);
}

/// Task 1.6's rounding floor. 2% of 99 lamports is 1.98, and the program floors
/// the proportional fee to whole lamports rather than rounding it up: the seller
/// keeps 98 and the protocol takes 1. A price this small can never settle — the
/// invoice rent alone is millions of lamports — so the arithmetic is pinned where
/// it lives (`fee_for`) instead of through a transaction that would be refused
/// for an unrelated reason.
#[test]
fn fee_floors_rather_than_rounds_at_ninety_nine_lamports() {
    let policy = test_policy(TEST_FEE_BPS, 0);
    let fee = fee_for(&policy, 99, None, 0).expect("the fee fits in a u64");

    assert_eq!(fee, 1, "2% of 99 floored is one lamport, not two");
    assert_eq!(99 - fee, 98);

    // A floor, not a rounding: 2% of 199 is 3.98 and still 3. The first amount
    // whose rate is a whole lamport is 100.
    assert_eq!(fee_for(&policy, 199, None, 0).expect("the fee fits"), 3);
    assert_eq!(fee_for(&policy, 100, None, 0).expect("the fee fits"), 2);
}

/// Task 1.6's cap on the SOL path, for an invoice the 0.1 SOL ceiling cannot
/// rescue. `fee_bps = 10_000` is accepted (`10_001` is refused, in
/// `update_config_requires_the_admin_and_a_valid_policy`), and `fee_for` then
/// takes the rate — the whole amount — and subtracts the marker's rent the seller
/// already paid, leaving a fee larger than this invoice's rent can cover: the
/// guard `amount > invoice_rent + fee` needs 102,491,680 lamports against this
/// 100,000,000. A larger invoice pays the ceiling and settles; this one fails
/// closed rather than letting the seller receive nothing.
#[tokio::test]
async fn a_ten_thousand_bps_policy_admits_no_tenth_sol_invoice() {
    let mut context =
        started_with_policy(None, 10_000, 0, FeeMode::SellerPays, TREASURY_LAMPORTS, &[])
            .await
            .0;
    let payer = context.payer.pubkey();
    let seller = Keypair::new().pubkey();
    let reference = Keypair::new().pubkey();
    let amount = LAMPORTS_PER_SOL / 10;
    let invoice = payment_pda(&seller, &reference, amount);
    let treasury_before = lamports(&context, treasury_pda()).await;

    // The policy is what the program stored, not what the test asked for.
    assert_eq!(
        config(&context, config_pda()).await.policy.fee_bps_below,
        10_000
    );

    let attempt = send(
        &mut context,
        &[settle_instruction(
            payer, seller, reference, amount, "all fee",
        )],
        &[],
    )
    .await;
    let error = attempt.expect_err("a 100% fee leaves the seller nothing to receive");
    assert!(
        error.contains(INVOICE_TOO_SMALL),
        "expected {INVOICE_TOO_SMALL}, got {error}"
    );

    // Refused before anything moved or was created.
    assert_eq!(lamports(&context, seller).await, 0);
    assert_eq!(lamports(&context, treasury_pda()).await, treasury_before);
    assert!(
        account(&context, invoice).await.is_none(),
        "a refused settle must not leave the invoice behind"
    );
    assert!(account(&context, receipt_pda(&invoice)).await.is_none());
}

#[tokio::test]
async fn withdraw_treasury_requires_the_admin_and_keeps_the_refund_reserve() {
    let (mut context, admin_keypair) = started_with_policy(
        None,
        TEST_FEE_BPS,
        TEST_FIXED_FEE,
        FeeMode::SellerPays,
        TREASURY_LAMPORTS,
        &[],
    )
    .await;
    let admin = admin_keypair.pubkey();
    let treasury = treasury_pda();
    // Withdrawable is everything above the reserve: one settlement's refund and
    // the account's own rent.
    let surplus = lamports(&context, treasury).await - treasury_reserve();
    assert_eq!(treasury_reserve(), token_refund() + treasury_rent());

    // A funded stranger cannot move treasury lamports.
    let stranger = Keypair::new();
    context.set_account(
        &stranger.pubkey(),
        &system_account(LAMPORTS_PER_SOL / 10).into(),
    );
    let attempt = send(
        &mut context,
        &[withdraw_treasury_instruction(stranger.pubkey(), 1)],
        &[&stranger],
    )
    .await;
    let error = attempt.expect_err("only the admin may withdraw");
    // Custom(2001) is Anchor's `ConstraintHasOne`.
    assert!(
        error.contains("Custom(2001)"),
        "expected an admin constraint failure, got {error}"
    );

    // More than the surplus is refused rather than draining the treasury.
    let overdraft = send(
        &mut context,
        &[withdraw_treasury_instruction(admin, surplus + 1)],
        &[&admin_keypair],
    )
    .await;
    let error = overdraft.expect_err("the treasury must keep its refund reserve");
    assert!(
        error.contains(TREASURY_UNDERFUNDED),
        "expected {TREASURY_UNDERFUNDED}, got {error}"
    );

    let payer_before = lamports(&context, admin).await;
    send(
        &mut context,
        &[withdraw_treasury_instruction(admin, surplus)],
        &[&admin_keypair],
    )
    .await
    .expect("the admin withdrawal should succeed");
    assert_eq!(
        lamports(&context, treasury).await,
        treasury_reserve(),
        "the reserve is what the next token settlement needs to refund"
    );
    // The test payer covers the network fee, not the admin.
    assert_eq!(lamports(&context, admin).await, payer_before + surplus);
}

/// Task 1.6's sequence, in the order an operator can cause it: the admin
/// withdraws every lamport the program allows, and the next settle goes through
/// anyway — the seller pays for the marker, so a drained treasury is not a
/// settlement the protocol cannot finish, and the reserve a withdrawal leaves
/// cannot be under one refund.
///
/// This used to be the underfunding guard's second case. It survives the move
/// as the opposite assertion: what a withdrawal must not do is take the
/// protocol's ability to settle with it.
#[tokio::test]
async fn withdrawing_the_treasury_to_its_reserve_does_not_block_the_next_settle() {
    let (mut context, admin_keypair) = started_with_policy(
        None,
        TEST_FEE_BPS,
        TEST_FIXED_FEE,
        FeeMode::SellerPays,
        TREASURY_LAMPORTS,
        &[],
    )
    .await;
    let admin = admin_keypair.pubkey();
    let treasury = treasury_pda();
    let surplus = lamports(&context, treasury).await - treasury_reserve();

    send(
        &mut context,
        &[withdraw_treasury_instruction(admin, surplus)],
        &[&admin_keypair],
    )
    .await
    .expect("withdrawing the surplus failed");
    assert_eq!(lamports(&context, treasury).await, treasury_reserve());
    assert!(
        treasury_reserve() > token_refund(),
        "the reserve has to cover a refund for this test to mean anything"
    );

    let payer = context.payer.pubkey();
    let seller = Keypair::new().pubkey();
    let reference = Keypair::new().pubkey();
    let amount = LAMPORTS_PER_SOL / 10;
    let invoice = payment_pda(&seller, &reference, amount);

    send(
        &mut context,
        &[settle_instruction(
            payer,
            seller,
            reference,
            amount,
            "after the withdrawal",
        )],
        &[],
    )
    .await
    .expect("a treasury at its rent must not block the settle");

    assert_eq!(
        lamports(&context, seller).await,
        amount - fee(amount) - receipt_rent()
    );
    assert!(
        account(&context, invoice).await.is_none(),
        "the settle must close the invoice"
    );
    assert!(account(&context, receipt_pda(&invoice)).await.is_some());
    assert_eq!(
        lamports(&context, treasury).await,
        treasury_reserve() + fee(amount),
        "the fee is the only lamport the treasury gains"
    );
}

#[tokio::test]
async fn initialize_config_rejects_a_decoy_program_data_account() {
    let mut test = program_test();
    let admin = Keypair::new();
    test.add_account(admin.pubkey(), system_account(LAMPORTS_PER_SOL / 10));
    test.add_account(treasury_pda(), system_account(TREASURY_LAMPORTS));
    let mut context = test.start_with_context().await;
    context.set_account(
        &program_data_address(),
        &program_data_account(Some(admin.pubkey())).into(),
    );

    // A random address is not this program's ProgramData PDA.
    let decoy = Keypair::new().pubkey();
    let initialize = initialize_config_instruction_to(
        admin.pubkey(),
        decoy,
        TEST_FEE_BPS,
        TEST_FIXED_FEE,
        FeeMode::SellerPays,
        TEST_SESSION_FLOAT,
    );
    let attempt = send(&mut context, &[initialize], &[&admin]).await;
    let error = attempt.expect_err("a decoy ProgramData account must be refused");
    // Custom(2006) is Anchor's `ConstraintSeeds`.
    assert!(
        error.contains("Custom(2006)"),
        "expected a seeds constraint failure, got {error}"
    );
}

#[tokio::test]
async fn initialize_config_rejects_a_finalized_program() {
    let mut test = program_test();
    let admin = Keypair::new();
    test.add_account(admin.pubkey(), system_account(LAMPORTS_PER_SOL / 10));
    test.add_account(treasury_pda(), system_account(TREASURY_LAMPORTS));
    let mut context = test.start_with_context().await;
    // `None` means the program can never be upgraded again, so no signature can
    // prove the right to install a fee policy.
    context.set_account(&program_data_address(), &program_data_account(None).into());

    let initialize = initialize_config_instruction(
        admin.pubkey(),
        TEST_FEE_BPS,
        TEST_FIXED_FEE,
        FeeMode::SellerPays,
        TEST_SESSION_FLOAT,
    );
    let attempt = send(&mut context, &[initialize], &[&admin]).await;
    let error = attempt.expect_err("a finalized program must not bootstrap a config");
    assert!(
        error.contains("Custom(6011)"),
        "expected ProgramNotUpgradeable, got {error}"
    );
}

#[tokio::test]
async fn buyer_pays_policy_is_rejected_on_the_budget_path_too() {
    let session = Keypair::new();
    let (mut context, _admin) = started_with_policy(
        None,
        TEST_FEE_BPS,
        TEST_FIXED_FEE,
        FeeMode::BuyerPays,
        TREASURY_LAMPORTS,
        &[(session.pubkey(), LAMPORTS_PER_SOL)],
    )
    .await;
    let buyer = context.payer.pubkey();
    let budget_address = budget_pda(&buyer);
    let monthly_limit = LAMPORTS_PER_SOL / 4;

    let open = Instruction {
        program_id: payvand::id(),
        accounts: vec![
            AccountMeta::new(buyer, true),
            AccountMeta::new(budget_address, false),
            AccountMeta::new(session.pubkey(), false),
            AccountMeta::new_readonly(config_pda(), false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: instruction_data("open_budget", &monthly_limit.to_le_bytes()),
    };
    send(&mut context, &[open], &[])
        .await
        .expect("open_budget is independent of the fee mode");

    let seller = Keypair::new().pubkey();
    let reference = Keypair::new().pubkey();
    let amount = LAMPORTS_PER_SOL / 20;
    let invoice = payment_pda(&seller, &reference, amount);
    let pay = Instruction {
        program_id: payvand::id(),
        accounts: vec![
            AccountMeta::new(session.pubkey(), true),
            AccountMeta::new(budget_address, false),
            AccountMeta::new(invoice, false),
            AccountMeta::new(seller, false),
            AccountMeta::new(receipt_pda(&invoice), false),
            AccountMeta::new_readonly(reference, false),
            AccountMeta::new_readonly(config_pda(), false),
            AccountMeta::new(treasury_pda(), false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: instruction_data("pay_from_budget", &amount_and_memo(amount, "silent coffee")),
    };
    let attempt = send(&mut context, &[pay], &[&session]).await;
    let error = attempt.expect_err("the budget path supports SellerPays only in this release");
    assert!(
        error.contains(FEE_MODE_UNSUPPORTED),
        "expected {FEE_MODE_UNSUPPORTED}, got {error}"
    );
}

// ---------------------------------------------------------------------------
// Layout and harness guards
// ---------------------------------------------------------------------------

// Proves the suite is exercising the built artifact: `ProgramTest` with no
// native processor loads the program as an executable BPF account, which is
// exactly what the cluster deploys.
#[tokio::test]
async fn tests_run_the_built_sbf_program() {
    let context = started().await;
    let program = account(&context, payvand::id())
        .await
        .expect("program account missing");
    assert!(program.executable, "program is not executable");
    assert_eq!(
        program.owner,
        solana_sdk_ids::bpf_loader::id(),
        "program is not owned by the BPF loader"
    );
}

/// Task 4.4's Token-2022 decision, enforced: a mint the SPL Token program does
/// not own is refused by name, before the program writes an invoice or CPIs into
/// a token program — the attempt used to die inside the first CPI instead.
#[tokio::test]
async fn a_token_2022_mint_is_rejected_by_name() {
    let (mut context, world) = TokenWorld::start().await;

    // A Token-2022 mint with the buyer's, seller's and treasury's Token-2022
    // accounts for it: what a Token-2022 client hands the program.
    let mint = Keypair::new().pubkey();
    context.set_account(
        &mint,
        &token_2022_mint_account(world.admin.pubkey(), 6).into(),
    );
    let seller = Keypair::new().pubkey();
    let ata =
        |wallet: Pubkey| associated_token_address_for_program(token_2022_program(), wallet, mint);
    context.set_account(
        &ata(seller),
        &token_2022_token_account(mint, seller, 0).into(),
    );
    context.set_account(
        &ata(world.buyer.pubkey()),
        &token_2022_token_account(mint, world.buyer.pubkey(), TEST_TOKEN_BALANCE).into(),
    );
    context.set_account(
        &ata(treasury_pda()),
        &token_2022_token_account(mint, treasury_pda(), 0).into(),
    );

    let reference = Keypair::new().pubkey();
    let amount = 100_000;
    let error = send(
        &mut context,
        &[settle_token_payment_instruction_with_token_program(
            token_2022_program(),
            world.buyer.pubkey(),
            seller,
            reference,
            mint,
            amount,
            "token-2022",
        )],
        &[&world.buyer],
    )
    .await
    .expect_err("a Token-2022 mint must be refused");
    assert!(
        error.contains(TOKEN_2022_UNSUPPORTED),
        "expected {TOKEN_2022_UNSUPPORTED}, got {error}"
    );

    // Refused before anything was written: no invoice, no receipt, and the
    // buyer still holds its balance.
    let invoice = token_payment_pda(&seller, &reference, &mint, amount);
    assert!(account(&context, invoice).await.is_none());
    assert!(account(&context, receipt_pda(&invoice)).await.is_none());
    assert_eq!(
        token_balance(&context, ata(world.buyer.pubkey())).await,
        TEST_TOKEN_BALANCE
    );
}

/// Borsh length of an account's fields, without Anchor's discriminator.
fn borsh_len<T: anchor_lang::AnchorSerialize>(value: &T) -> usize {
    let mut bytes = Vec::new();
    value.serialize(&mut bytes).expect("account serializes");
    bytes.len()
}

/// Serializing the real structs is the check that cannot drift from `state.rs`:
/// adding a field without growing its space constant fails here instead of on
/// chain as a realloc.
#[test]
fn serialized_accounts_fit_their_space_constants() {
    let key = [7u8; 32];
    // A memo at the maximum length: the space reserves all 96 bytes, so a short
    // memo would serialise smaller and prove nothing.
    let memo = "m".repeat(PAYMENT_MAX_MEMO);
    let payment = Payment {
        seller: Pubkey::from(key),
        reference: Pubkey::from(key),
        amount: 1,
        memo: memo.clone(),
        state: PaymentState::Pending,
        payer: Pubkey::from(key),
        created_at: 1,
        settled_at: 0,
        bump: 255,
    };
    assert_eq!(borsh_len(&payment) + 8, PAYMENT_SPACE);

    let token_payment = TokenPayment {
        seller: Pubkey::from(key),
        reference: Pubkey::from(key),
        mint: Pubkey::from(key),
        amount: 1,
        memo: memo.clone(),
        state: PaymentState::Pending,
        payer: Pubkey::from(key),
        created_at: 1,
        settled_at: 0,
        bump: 255,
    };
    assert_eq!(
        borsh_len(&token_payment) + 8,
        payvand::constants::TOKEN_PAYMENT_SPACE
    );

    let receipt = PaymentReceipt {
        invoice: Pubkey::from(key),
        settled_at: 2,
    };
    assert_eq!(borsh_len(&receipt) + 8, PAYMENT_RECEIPT_SPACE);

    let budget = Budget {
        buyer: Pubkey::from(key),
        session_key: Pubkey::from(key),
        monthly_limit: 1,
        spent: 0,
        month_index: 1,
        bump: 255,
    };
    assert_eq!(borsh_len(&budget) + 8, BUDGET_SPACE);

    let token_budget = TokenBudget {
        buyer: Pubkey::from(key),
        session_key: Pubkey::from(key),
        mint: Pubkey::from(key),
        monthly_limit: 1,
        spent: 0,
        month_index: 1,
        bump: 255,
    };
    assert_eq!(borsh_len(&token_budget) + 8, TOKEN_BUDGET_SPACE);

    let config = Config {
        admin: Pubkey::from(key),
        policy: test_policy_of(200, 1),
        fee_mode: FeeMode::SellerPays,
        session_fee_float: 30_000_000,
        flats_updated_at: 1_700_000_000,
        bump: 255,
        treasury_bump: 254,
        version: CONFIG_VERSION,
    };
    assert_eq!(borsh_len(&config) + 8, CONFIG_SPACE);
}

// The spaces are hand-written byte sums; a pin on the arithmetic catches a
// change to the constant itself, and the test above catches a change to a
// struct that the constant was not updated for.
#[test]
fn account_spaces_match_the_sizes_used_by_the_program() {
    assert_eq!(PAYMENT_SPACE, 230);
    // buyer, session key, limit, spent, month index, bump.
    assert_eq!(BUDGET_SPACE, 8 + 32 + 32 + 8 + 8 + 8 + 1);
    assert_eq!(BUDGET_SPACE, 97);

    // seller, reference, mint, amount, memo, state, payer, two timestamps, bump.
    assert_eq!(TOKEN_PAYMENT_SPACE, 262);
    // buyer, session key, mint, limit, spent, month index, bump.
    assert_eq!(TOKEN_BUDGET_SPACE, 8 + 32 + 32 + 32 + 8 + 8 + 8 + 1);
    assert_eq!(TOKEN_BUDGET_SPACE, 129);

    // One marker for both currencies: the invoice it points at and the
    // `settled_at` replay guard. The payment's own fields live in the
    // `PaymentSettled` event, which is what keeps this at 48 bytes instead of
    // 301.
    let receipt = 8 + 32 + 8;
    assert_eq!(PAYMENT_RECEIPT_SPACE, receipt);
    assert_eq!(PAYMENT_RECEIPT_SPACE, 48);

    // admin, the policy (two rates, their two boundaries, the two per-currency
    // flats, the two mints they apply to and the network fee in lamports and in
    // each token), fee_mode, session_fee_float, flats_updated_at, bump,
    // treasury_bump, version.
    assert_eq!(
        CONFIG_SPACE,
        8 + 32 + (2 + 2 + 8 + 8 + 8 + 8 + 32 + 32 + 8 + 8 + 8) + 1 + 8 + 8 + 1 + 1 + 1
    );
    assert_eq!(CONFIG_SPACE, 184);
    // The version 3 layout `migrate_config` upgrades from: the same account
    // without the network fee.
    assert_eq!(CONFIG_SPACE_V3, 160);
    // The version 2 layout `migrate_config` upgrades from: the same account
    // without the refresh cooldown.
    assert_eq!(CONFIG_SPACE_V2, 152);
    // The version 1 layout `migrate_config` upgrades from.
    assert_eq!(CONFIG_SPACE_V1, 8 + 32 + 2 + 8 + 1 + 8 + 1 + 1 + 1);
    assert_eq!(CONFIG_SPACE_V1, 62);
}

/// The config bootstrap reads the upgradeable loader's `ProgramData` metadata
/// by hand. This pins those offsets to what bincode writes, so a loader change
/// fails here instead of silently reading the wrong bytes on chain.
#[test]
fn program_data_metadata_layout_matches_bincode() {
    use anchor_lang::solana_program::bpf_loader_upgradeable::UpgradeableLoaderState;

    let authority = Pubkey::new_from_array([9u8; 32]);
    let state = UpgradeableLoaderState::ProgramData {
        slot: 7,
        upgrade_authority_address: Some(authority),
    };
    let bytes = bincode::serialize(&state).expect("ProgramData serializes");
    assert_eq!(
        bytes.len(),
        UpgradeableLoaderState::size_of_programdata_metadata()
    );
    // Uninitialized = 0, Buffer = 1, Program = 2, ProgramData = 3.
    assert_eq!(u32::from_le_bytes(bytes[0..4].try_into().unwrap()), 3);
    assert_eq!(bytes[12], 1);
    assert_eq!(&bytes[13..45], authority.as_ref());

    let finalized = UpgradeableLoaderState::ProgramData {
        slot: 7,
        upgrade_authority_address: None,
    };
    let bytes = bincode::serialize(&finalized).expect("ProgramData serializes");
    assert_eq!(bytes[12], 0);
}

// ---------------------------------------------------------------------------
// Token flows: the SPL programs execute for real
// ---------------------------------------------------------------------------

/// What `init` puts in a token invoice, and what its close hands back.
fn token_invoice_rent() -> u64 {
    Rent::default().minimum_balance(TOKEN_PAYMENT_SPACE)
}

/// A token account's rent: 165 bytes of the SPL layout.
fn token_account_rent() -> u64 {
    Rent::default().minimum_balance(165)
}

/// A plain system transfer, the bytes the client builds by hand: instruction 2
/// and the lamports, little-endian. The program cannot make this one — the
/// floats are lamports the buyer owns.
fn transfer_lamports_instruction(from: Pubkey, to: Pubkey, lamports: u64) -> Instruction {
    let mut data = Vec::with_capacity(12);
    data.extend_from_slice(&2u32.to_le_bytes());
    data.extend_from_slice(&lamports.to_le_bytes());
    Instruction {
        program_id: SYSTEM_PROGRAM_ID,
        accounts: vec![AccountMeta::new(from, true), AccountMeta::new(to, false)],
        data,
    }
}

/// What the buyer holds before any test starts: enough for every budget and
/// payment below with room to spare.
const TEST_TOKEN_BALANCE: u64 = 1_000_000;

/// A bank with a funded buyer who already holds a token account, a session key
/// that starts with no lamports, and the derived budget and vault addresses.
struct TokenWorld {
    admin: Keypair,
    buyer: Keypair,
    session: Keypair,
    mint: Pubkey,
    buyer_token_account: Pubkey,
    budget: Pubkey,
    vault: Pubkey,
}

impl TokenWorld {
    /// Builds the bank, starts it, and bootstraps the config under the default
    /// test policy, so a test begins with nothing left to set up.
    async fn start() -> (ProgramTestContext, TokenWorld) {
        let admin = Keypair::new();
        let buyer = Keypair::new();
        let session = Keypair::new();
        let mint = Keypair::new().pubkey();
        let buyer_token_account = associated_token_address(buyer.pubkey(), mint);
        let (budget, _bump) = token_budget_pda(&buyer.pubkey(), &mint);
        let vault = associated_token_address(budget, mint);

        let mut test = program_test();
        test.add_account(admin.pubkey(), system_account(LAMPORTS_PER_SOL / 10));
        test.add_account(buyer.pubkey(), system_account(LAMPORTS_PER_SOL));
        test.add_account(treasury_pda(), system_account(TREASURY_LAMPORTS));
        test.add_account(mint, spl_mint_account(admin.pubkey(), 6));
        test.add_account(
            buyer_token_account,
            spl_token_account(mint, buyer.pubkey(), TEST_TOKEN_BALANCE),
        );

        let mut context = test.start_with_context().await;
        context.set_account(
            &program_data_address(),
            &program_data_account(Some(admin.pubkey())).into(),
        );
        send(
            &mut context,
            &[initialize_config_instruction(
                admin.pubkey(),
                TEST_FEE_BPS,
                TEST_FIXED_FEE,
                FeeMode::SellerPays,
                TEST_SESSION_FLOAT,
            )],
            &[&admin],
        )
        .await
        .expect("initialize_config failed");

        // The treasury's own account for the mint: the fee's destination on both
        // token paths, opened once by the admin.
        send(
            &mut context,
            &[open_treasury_token_account_instruction(
                admin.pubkey(),
                mint,
            )],
            &[&admin],
        )
        .await
        .expect("open_treasury_token_account failed");

        (
            context,
            TokenWorld {
                admin,
                buyer,
                session,
                mint,
                buyer_token_account,
                budget,
                vault,
            },
        )
    }
}

/// `open_token_budget` for the world's buyer and session key.
async fn open_token_budget(context: &mut ProgramTestContext, world: &TokenWorld, limit: u64) {
    send(
        context,
        &[open_token_budget_instruction(
            world.buyer.pubkey(),
            world.budget,
            world.mint,
            world.buyer_token_account,
            world.vault,
            world.session.pubkey(),
            limit,
        )],
        &[&world.buyer],
    )
    .await
    .expect("open_token_budget failed");
}

/// `seller`'s token account, opened before the payment the way the seller would
/// open it: its rent is the seller's, never the buyer's session float.
fn seed_seller_token_account(context: &mut ProgramTestContext, mint: Pubkey, seller: Pubkey) {
    context.set_account(
        &associated_token_address(seller, mint),
        &spl_token_account(mint, seller, 0).into(),
    );
}

/// One `pay_from_token_budget` from the session key to `seller`, who holds no SOL
/// but has opened their token account.
async fn pay_from_token_budget(
    context: &mut ProgramTestContext,
    world: &TokenWorld,
    seller: Pubkey,
    amount: u64,
) {
    let reference = Keypair::new().pubkey();
    seed_seller_token_account(context, world.mint, seller);
    // The session key pays, as it does in the client.
    send_with_fee_payer(
        context,
        &[pay_from_token_budget_instruction(
            world,
            world.mint,
            world.vault,
            seller,
            reference,
            amount,
            "spl",
        )],
        &world.session,
    )
    .await
    .expect("pay_from_token_budget failed");
}

/// A second mint with `budget`'s own canonical vault for it, funded with
/// `tokens`: everything a token path can be handed where only the budget's mint
/// check should reject it.
fn seed_other_mint(
    context: &mut ProgramTestContext,
    world: &TokenWorld,
    tokens: u64,
) -> (Pubkey, Pubkey) {
    let mint = Keypair::new().pubkey();
    let vault = associated_token_address(world.budget, mint);
    context.set_account(&mint, &spl_mint_account(world.admin.pubkey(), 6).into());
    context.set_account(
        &vault,
        &spl_token_account(mint, world.budget, tokens).into(),
    );
    (mint, vault)
}

/// Task 3.1's split on the payer path: the seller receives `amount - fee_t`, the
/// treasury's account for the mint receives `fee_t`, and the buyer loses exactly
/// the price.
#[tokio::test]
async fn settle_token_payment_splits_the_price_between_seller_and_treasury() {
    let (mut context, world) = TokenWorld::start().await;

    // The treasury's account for this mint is provisioned once by the admin: the
    // treasury is a PDA, so nobody can open it the way a wallet owner would.
    let treasury_token_account = associated_token_address(treasury_pda(), world.mint);
    send(
        &mut context,
        &[open_treasury_token_account_instruction(
            world.admin.pubkey(),
            world.mint,
        )],
        &[&world.admin],
    )
    .await
    .expect("open_treasury_token_account failed");
    assert_eq!(token_balance(&context, treasury_token_account).await, 0);

    // The seller has an account but no SOL; opening it is their own step.
    let seller = Keypair::new().pubkey();
    let seller_token_account = associated_token_address(seller, world.mint);
    context.set_account(
        &seller_token_account,
        &spl_token_account(world.mint, seller, 0).into(),
    );

    let reference = Keypair::new().pubkey();
    let amount = 700_000;
    let fee_t = token_fee(amount);
    let invoice = token_payment_pda(&seller, &reference, &world.mint, amount);
    let treasury_before = lamports(&context, treasury_pda()).await;

    send(
        &mut context,
        &[settle_token_payment_instruction(
            world.buyer.pubkey(),
            seller,
            reference,
            world.mint,
            amount,
            "spl",
        )],
        &[&world.buyer],
    )
    .await
    .expect("settle_token_payment failed");

    // The split, and nothing else leaving the buyer.
    assert_eq!(
        token_balance(&context, seller_token_account).await,
        amount - fee_t
    );
    assert_eq!(token_balance(&context, treasury_token_account).await, fee_t);
    assert_eq!(
        token_balance(&context, world.buyer_token_account).await,
        TEST_TOKEN_BALANCE - amount
    );
    // The seller carried no SOL before or after the payment.
    assert_eq!(lamports(&context, seller).await, 0);

    // The invoice closes to the pool, which keeps its rent and pays the
    // receipt's; the receipt is the record that stays.
    assert_eq!(lamports(&context, invoice).await, 0);
    let kept = receipt(&context, receipt_pda(&invoice)).await;
    assert_eq!(kept.invoice, invoice);
    assert!(kept.settled_at > 0);
    // The pool advanced both rents back to the payer and took the invoice's
    // back when it closed, so it is one receipt rent down for the record.
    assert_eq!(
        lamports(&context, treasury_pda()).await,
        treasury_before - receipt_rent()
    );
}

/// The token seller's floor is the record's rent **and** the network fee, and the
/// sponsor is refunded the fee it paid as well as both rents it advanced — so the
/// seller ends up paying what the settlement cost us, and the payer nothing.
#[tokio::test]
async fn a_token_settle_charges_the_network_fee_to_the_seller() {
    let (mut context, world) = TokenWorld::start().await;

    // A policy that names both costs: 89,400 base units of record rent and 2,000
    // of network fee in USDC, against 5,000 lamports actually spent.
    let mut policy = test_policy_of(TEST_FEE_BPS, 89_400);
    // The world's mint is the one this policy prices in USDC, so its flat and the
    // fee's USDC equivalent apply to the invoice below.
    policy.usdc_mint = world.mint;
    policy.network_fee_lamports = 5_000;
    policy.network_fee_usdc_base_units = 2_000;
    send(
        &mut context,
        &[update_config_with_policy_instruction(
            world.admin.pubkey(),
            policy,
        )],
        &[&world.admin],
    )
    .await
    .expect("update_config failed");

    let seller = Keypair::new().pubkey();
    let seller_token_account = associated_token_address(seller, world.mint);
    context.set_account(
        &seller_token_account,
        &spl_token_account(world.mint, seller, 0).into(),
    );

    let reference = Keypair::new().pubkey();
    let amount = 700_000;
    // 2% of 700,000 is 14,000, below the 91,400 the record and the fee cost.
    let fee_t = 91_400;
    let treasury_before = lamports(&context, treasury_pda()).await;
    let buyer_before = lamports(&context, world.buyer.pubkey()).await;

    // The buyer pays the transaction's own fee here, as the sponsor does on the
    // wallettess path; the pool's refund makes it whole.
    send_with_fee_payer(
        &mut context,
        &[settle_token_payment_instruction(
            world.buyer.pubkey(),
            seller,
            reference,
            world.mint,
            amount,
            "spl",
        )],
        &world.buyer,
    )
    .await
    .expect("settle_token_payment failed");

    assert_eq!(
        token_balance(&context, seller_token_account).await,
        amount - fee_t
    );
    assert_eq!(
        token_balance(
            &context,
            associated_token_address(treasury_pda(), world.mint)
        )
        .await,
        fee_t
    );
    // The seller bore the record and the fee; the payer's 5,000-lamport
    // transaction fee came back with the refund, so it paid the price only.
    assert_eq!(lamports(&context, world.buyer.pubkey()).await, buyer_before);
    assert_eq!(
        lamports(&context, treasury_pda()).await,
        treasury_before - receipt_rent() - 5_000,
        "the pool keeps the record's rent and pays out the network fee"
    );
}

/// A policy whose network fee is beyond what the program will reimburse is
/// refused when it is written, so a settlement never has to find out.
#[tokio::test]
async fn update_config_refuses_an_implausible_network_fee() {
    let (mut context, admin) = started_with_policy(
        None,
        TEST_FEE_BPS,
        TEST_FIXED_FEE,
        FeeMode::SellerPays,
        TREASURY_LAMPORTS,
        &[],
    )
    .await;

    let mut policy = test_policy_of(TEST_FEE_BPS, 0);
    policy.network_fee_lamports = MAX_NETWORK_FEE_LAMPORTS + 1;
    let attempt = send(
        &mut context,
        &[update_config_with_policy_instruction(
            admin.pubkey(),
            policy.clone(),
        )],
        &[&admin],
    )
    .await;
    let error = attempt.expect_err("an absurd network fee must be refused");
    assert!(
        error.contains(INVALID_NETWORK_FEE),
        "expected {INVALID_NETWORK_FEE}, got {error}"
    );

    // The same policy with a real fee is accepted.
    let mut policy = policy;
    policy.network_fee_lamports = 5_000;
    send(
        &mut context,
        &[update_config_with_policy_instruction(
            admin.pubkey(),
            policy,
        )],
        &[&admin],
    )
    .await
    .expect("a plausible network fee should be accepted");
    let stored = config(&context, config_pda()).await;
    assert_eq!(stored.policy.network_fee_lamports, 5_000);
}

/// The gasless case: a wallet that holds only the token has never had a lamport,
/// so it cannot pay the network fee or advance the two record rents. The sponsor
/// signs as fee payer, advances both rents and receives the pool's refund of
/// them, and the buyer's lamport balance is still exactly zero after the
/// settlement — the price is the only thing that left it. A sponsor that funded
/// the buyer instead would hand it 3,939,360 lamports a settlement, which is the
/// faucet this split exists to close.
#[tokio::test]
async fn a_sponsored_settle_pays_with_a_wallet_that_holds_no_sol() {
    let (mut context, world) = TokenWorld::start().await;

    // A buyer that was never funded: the key exists only as the authority on its
    // token account, which is enough to sign for it.
    let buyer = Keypair::new();
    let buyer_token_account = associated_token_address(buyer.pubkey(), world.mint);
    context.set_account(
        &buyer_token_account,
        &spl_token_account(world.mint, buyer.pubkey(), TEST_TOKEN_BALANCE).into(),
    );

    // The seller's account is their own setup step, as on every token path.
    let seller = Keypair::new().pubkey();
    let seller_token_account = associated_token_address(seller, world.mint);
    context.set_account(
        &seller_token_account,
        &spl_token_account(world.mint, seller, 0).into(),
    );

    let reference = Keypair::new().pubkey();
    let amount = 700_000;
    let fee_t = token_fee(amount);
    let invoice = token_payment_pda(&seller, &reference, &world.mint, amount);

    // The relayer: the bank's own funded key, so it is the transaction's fee
    // payer as well as the settlement's sponsor.
    let sponsor = context.payer.pubkey();
    let sponsor_before = lamports(&context, sponsor).await;
    let treasury_before = lamports(&context, treasury_pda()).await;
    assert_eq!(lamports(&context, buyer.pubkey()).await, 0);

    send(
        &mut context,
        &[settle_token_payment_instruction_with_sponsor(
            token_program(),
            buyer.pubkey(),
            sponsor,
            seller,
            reference,
            world.mint,
            amount,
            "sponsored",
        )],
        &[&buyer],
    )
    .await
    .expect("a sponsored token settle should succeed");

    // The buyer paid the price and no lamports at all.
    assert_eq!(
        lamports(&context, buyer.pubkey()).await,
        0,
        "a sponsored settle must not leave the buyer holding lamports"
    );
    assert_eq!(
        token_balance(&context, buyer_token_account).await,
        TEST_TOKEN_BALANCE - amount
    );
    assert_eq!(
        token_balance(&context, seller_token_account).await,
        amount - fee_t
    );
    assert_eq!(
        token_balance(
            &context,
            associated_token_address(treasury_pda(), world.mint)
        )
        .await,
        fee_t
    );

    // The refund came back to the sponsor, so its whole cost is the network fee:
    // two signatures at the bank's 5,000 lamports each.
    assert_eq!(
        sponsor_before - lamports(&context, sponsor).await,
        10_000,
        "the sponsor should be refunded both rents and pay only the transaction fee"
    );

    // The pool's books are the unsponsored ones: it advanced both rents, took the
    // invoice's back when the account closed, and is one receipt rent down for
    // the record.
    assert_eq!(
        lamports(&context, treasury_pda()).await,
        treasury_before - receipt_rent()
    );
    let kept = receipt(&context, receipt_pda(&invoice)).await;
    assert_eq!(kept.invoice, invoice);
    assert!(kept.settled_at > 0);
}

/// A sponsored settlement is refused exactly where an unsponsored one is when the
/// pool cannot refund: before any token moves, with the named error. The buyer's
/// zero balance is what makes this the sponsored case rather than the wallet's.
#[tokio::test]
async fn a_sponsored_settle_is_refused_when_the_treasury_cannot_refund() {
    let (mut context, world) = TokenWorld::start().await;

    // Only the account's own rent: not enough for the marker's rent, let alone
    // the invoice's.
    context.set_account(&treasury_pda(), &system_account(treasury_rent()).into());

    let buyer = Keypair::new();
    let buyer_token_account = associated_token_address(buyer.pubkey(), world.mint);
    context.set_account(
        &buyer_token_account,
        &spl_token_account(world.mint, buyer.pubkey(), TEST_TOKEN_BALANCE).into(),
    );

    let seller = Keypair::new().pubkey();
    let seller_token_account = associated_token_address(seller, world.mint);
    context.set_account(
        &seller_token_account,
        &spl_token_account(world.mint, seller, 0).into(),
    );

    let reference = Keypair::new().pubkey();
    let amount = 700_000;
    let sponsor = context.payer.pubkey();
    let error = send(
        &mut context,
        &[settle_token_payment_instruction_with_sponsor(
            token_program(),
            buyer.pubkey(),
            sponsor,
            seller,
            reference,
            world.mint,
            amount,
            "sponsored",
        )],
        &[&buyer],
    )
    .await
    .expect_err("a pool that cannot refund must not start a settlement");
    assert!(
        error.contains(TREASURY_UNDERFUNDED),
        "expected {TREASURY_UNDERFUNDED}, got {error}"
    );

    // Nothing moved and no record was written.
    assert_eq!(token_balance(&context, seller_token_account).await, 0);
    assert_eq!(
        token_balance(&context, buyer_token_account).await,
        TEST_TOKEN_BALANCE
    );
    let invoice = token_payment_pda(&seller, &reference, &world.mint, amount);
    assert!(account(&context, receipt_pda(&invoice)).await.is_none());
}

/// Task 3.1's cap on the token path, where there is no invoice rent to subtract:
/// `settle_token_payment` guards `amount > fee_t` alone, so the `amount - 1` cap
/// in `fee_for` is what keeps the seller's side non-zero. At `fee_bps = 10_000`
/// a 700,000-unit invoice therefore pays the seller one base unit and the
/// treasury's account the other 699,999 — rather than a seller receiving zero.
#[tokio::test]
async fn a_ten_thousand_bps_policy_leaves_the_token_seller_one_base_unit() {
    let (mut context, world) = TokenWorld::start().await;

    // Set the way an admin changes it, so this is a live `Config` and not a
    // hand-built one.
    send(
        &mut context,
        &[update_config_instruction(
            world.admin.pubkey(),
            10_000,
            0,
            FeeMode::SellerPays,
            TEST_SESSION_FLOAT,
        )],
        &[&world.admin],
    )
    .await
    .expect("update_config failed");
    assert_eq!(
        config(&context, config_pda()).await.policy.fee_bps_below,
        10_000
    );

    let treasury_token_account = associated_token_address(treasury_pda(), world.mint);
    send(
        &mut context,
        &[open_treasury_token_account_instruction(
            world.admin.pubkey(),
            world.mint,
        )],
        &[&world.admin],
    )
    .await
    .expect("open_treasury_token_account failed");

    let seller = Keypair::new().pubkey();
    let seller_token_account = associated_token_address(seller, world.mint);
    context.set_account(
        &seller_token_account,
        &spl_token_account(world.mint, seller, 0).into(),
    );

    let reference = Keypair::new().pubkey();
    let amount = 700_000;

    send(
        &mut context,
        &[settle_token_payment_instruction(
            world.buyer.pubkey(),
            seller,
            reference,
            world.mint,
            amount,
            "all fee",
        )],
        &[&world.buyer],
    )
    .await
    .expect("a capped 100% token fee must still settle");

    assert_eq!(token_balance(&context, seller_token_account).await, 1);
    assert_eq!(
        token_balance(&context, treasury_token_account).await,
        amount - 1
    );
    assert_eq!(
        token_balance(&context, world.buyer_token_account).await,
        TEST_TOKEN_BALANCE - amount
    );
}

/// Task 3.4's replay criterion on the token path: one settlement per invoice. The
/// first settle closes the invoice account and leaves the receipt behind, so a
/// replay gets a fresh invoice account and it is the receipt's `settled_at` that
/// refuses it — before any token moves.
#[tokio::test]
async fn settle_token_payment_replay_is_rejected_as_already_settled() {
    let (mut context, world) = TokenWorld::start().await;

    // Both accounts are setup, not part of the payment: the treasury's for this
    // mint, and the seller's own.
    let treasury_token_account = associated_token_address(treasury_pda(), world.mint);
    send(
        &mut context,
        &[open_treasury_token_account_instruction(
            world.admin.pubkey(),
            world.mint,
        )],
        &[&world.admin],
    )
    .await
    .expect("open_treasury_token_account failed");

    let seller = Keypair::new().pubkey();
    let seller_token_account = associated_token_address(seller, world.mint);
    context.set_account(
        &seller_token_account,
        &spl_token_account(world.mint, seller, 0).into(),
    );

    let reference = Keypair::new().pubkey();
    let amount = 700_000;
    let invoice = token_payment_pda(&seller, &reference, &world.mint, amount);
    let settle = || {
        settle_token_payment_instruction(
            world.buyer.pubkey(),
            seller,
            reference,
            world.mint,
            amount,
            "spl",
        )
    };

    send(&mut context, &[settle()], &[&world.buyer])
        .await
        .expect("settle_token_payment failed");
    let seller_after_first = token_balance(&context, seller_token_account).await;
    let treasury_after_first = token_balance(&context, treasury_token_account).await;
    let buyer_after_first = token_balance(&context, world.buyer_token_account).await;
    let receipt_after_first = receipt(&context, receipt_pda(&invoice)).await;
    assert!(receipt_after_first.settled_at > 0);

    // Same invoice, same accounts, a second signature from the same wallet. The
    // new blockhash keeps this a distinct transaction rather than a duplicate.
    refresh_blockhash(&mut context).await;
    let error = send(&mut context, &[settle()], &[&world.buyer])
        .await
        .expect_err("a settled token invoice must not settle twice");
    assert!(
        error.contains(ALREADY_SETTLED),
        "expected {ALREADY_SETTLED}, got {error}"
    );

    // Nothing moved, and the record still says what the first settle wrote.
    assert_eq!(
        token_balance(&context, seller_token_account).await,
        seller_after_first,
        "a rejected replay must not pay the seller again"
    );
    assert_eq!(
        token_balance(&context, treasury_token_account).await,
        treasury_after_first,
        "a rejected replay must not pay the fee again"
    );
    assert_eq!(
        token_balance(&context, world.buyer_token_account).await,
        buyer_after_first,
        "a rejected replay must not charge the buyer again"
    );
    assert_eq!(
        lamports(&context, invoice).await,
        0,
        "a rejected replay must not open a second invoice account"
    );
    let receipt_after_replay = receipt(&context, receipt_pda(&invoice)).await;
    assert_eq!(
        receipt_after_replay.settled_at,
        receipt_after_first.settled_at
    );
    assert_eq!(receipt_after_replay.invoice, receipt_after_first.invoice);
}

/// The allowance path takes the wallet path's rule for the seller's account: it
/// must already exist. Creating it from the session float would spend the
/// buyer's money on rent nobody refunds, so opening the account is the seller's
/// setup step, never a line on the buyer's bill.
///
/// The records stay the pool's: the session key advances both and the treasury
/// refunds both in the same transaction, so a settled payment costs the key its
/// own signature. The budget's lamports never move at all; its token account
/// collects the fees.
#[tokio::test]
async fn a_budget_payment_never_pays_a_sellers_account_rent() {
    let (mut context, world) = TokenWorld::start().await;

    // The session key holds nothing until `open_token_budget` funds it.
    assert_eq!(lamports(&context, world.session.pubkey()).await, 0);
    let limit = TEST_TOKEN_BALANCE;
    let price = 200_000;
    let fee_t = token_fee(price);
    open_token_budget(&mut context, &world, limit).await;
    assert_eq!(token_balance(&context, world.vault).await, limit);
    // `open_token_budget` puts the key's float on the session key, and nothing
    // on the budget beyond its own rent.
    let session_before = lamports(&context, world.session.pubkey()).await;
    let budget_before = lamports(&context, world.budget).await;
    let treasury_before = lamports(&context, treasury_pda()).await;
    assert_eq!(
        session_before, SESSION_FEE_FLOAT,
        "the session key opens with its float"
    );
    assert_eq!(
        budget_before,
        Rent::default().minimum_balance(TOKEN_BUDGET_SPACE),
        "the budget holds its rent, and the pool funds the records"
    );

    // A seller who holds neither SOL nor a token account: refused, and the float
    // is not spent opening one for them.
    let fresh = Keypair::new().pubkey();
    let error = send_with_fee_payer(
        &mut context,
        &[pay_from_token_budget_instruction(
            &world,
            world.mint,
            world.vault,
            fresh,
            Keypair::new().pubkey(),
            price,
            "fresh seller",
        )],
        &world.session,
    )
    .await
    .expect_err("a seller with no token account must be refused");
    assert!(
        error.contains(MISSING_TOKEN_ACCOUNT),
        "expected {MISSING_TOKEN_ACCOUNT}, got {error}"
    );
    assert!(
        account(&context, associated_token_address(fresh, world.mint))
            .await
            .is_none(),
        "the buyer's float must not open the seller's account"
    );
    assert_eq!(lamports(&context, fresh).await, 0);
    // A failed transaction is rolled back, so the attempt costs the float its
    // signature and no rent at all.
    let session_after_refusal = lamports(&context, world.session.pubkey()).await;
    assert_eq!(session_after_refusal, session_before - TX_FEE);
    assert_eq!(token_budget(&context, world.budget).await.spent, 0);

    // A seller who opened their own account is paid, and the session key pays
    // its signature alone.
    let seller = Keypair::new().pubkey();
    seed_seller_token_account(&mut context, world.mint, seller);
    let account_rent = lamports(&context, associated_token_address(seller, world.mint)).await;
    pay_from_token_budget(&mut context, &world, seller, price).await;
    assert_eq!(
        token_balance(&context, associated_token_address(seller, world.mint)).await,
        price - fee_t
    );
    assert_eq!(
        lamports(&context, seller).await,
        0,
        "the seller receives tokens, never lamports"
    );
    assert_eq!(
        lamports(&context, associated_token_address(seller, world.mint)).await,
        account_rent,
        "the account's rent is exactly what the seller put there"
    );
    assert_eq!(
        lamports(&context, world.session.pubkey()).await,
        session_after_refusal - TX_FEE,
        "a payment costs the float its own signature"
    );

    // Two more sellers, each with their own account: the cost per payment stays
    // the signature, whatever the seller's account rent is.
    for _ in 0..2 {
        pay_from_token_budget(&mut context, &world, Keypair::new().pubkey(), price).await;
    }
    assert_eq!(
        lamports(&context, world.session.pubkey()).await,
        session_after_refusal - 3 * TX_FEE,
        "three payments cost the float three signatures and nothing else"
    );
    assert_eq!(
        lamports(&context, world.budget).await,
        budget_before,
        "the budget's lamports do not move: it pays in tokens"
    );
    assert_eq!(
        lamports(&context, treasury_pda()).await,
        treasury_before - 3 * receipt_rent(),
        "the pool funds one permanent receipt a payment"
    );
    assert_eq!(
        token_balance(
            &context,
            associated_token_address(treasury_pda(), world.mint)
        )
        .await,
        3 * fee_t,
        "every fee lands in the treasury's own account for the mint"
    );
    assert_eq!(token_budget(&context, world.budget).await.spent, 3 * price);
    assert_eq!(
        token_balance(&context, world.buyer_token_account).await,
        TEST_TOKEN_BALANCE - limit
    );
}

/// The client's refill rule (`tokenFloatNeed` + `tokenFloatTopUp` in
/// `apps/web/app/composables/useBudget.ts`), in one place: the session key fronts
/// the two records and its own fee, the pool refunds the records, and the budget
/// is never refilled. A seller's token account is not in the sum: it must exist
/// before the payment, so no float pays for it.
///
/// The two copies cannot be merged — a Rust harness cannot call the app — so the
/// vectors in `tests/fixtures/token-float-top-up.txt` are the shared truth both
/// this and `useBudget.test.ts` assert against.
fn token_float_top_up(
    invoice_rent: u64,
    receipt_rent: u64,
    fee: u64,
    _budget_held: u64,
    session_held: u64,
) -> (u64, u64) {
    let records = invoice_rent + receipt_rent;
    // The pool refunds both records, so the budget holds its own rent and is never
    // refilled: `to_budget` is zero whatever it holds.
    (0, (records + fee).saturating_sub(session_held))
}

/// Refills the session float by exactly what one more payment needs, by the rule
/// above and the accounts as they are now.
async fn top_up_token_floats(context: &mut ProgramTestContext, world: &TokenWorld) {
    let budget_held = lamports(context, world.budget)
        .await
        .saturating_sub(Rent::default().minimum_balance(TOKEN_BUDGET_SPACE));
    let session_held = lamports(context, world.session.pubkey()).await;
    let (to_budget, to_session) = token_float_top_up(
        token_invoice_rent(),
        receipt_rent(),
        TX_FEE,
        budget_held,
        session_held,
    );

    let mut instructions = Vec::new();
    if to_budget > 0 {
        instructions.push(transfer_lamports_instruction(
            world.buyer.pubkey(),
            world.budget,
            to_budget,
        ));
    }
    if to_session > 0 {
        instructions.push(transfer_lamports_instruction(
            world.buyer.pubkey(),
            world.session.pubkey(),
            to_session,
        ));
    }
    if instructions.is_empty() {
        return;
    }
    send(context, &instructions, &[&world.buyer])
        .await
        .expect("token float top-up failed");
}

/// The shared vectors: the rule here and the client's rule read the same table, so
/// one moving without the other is a red test rather than a stale comment.
#[test]
fn the_float_top_up_rule_matches_the_shared_vectors() {
    let fixture = include_str!("fixtures/token-float-top-up.txt");
    let mut rows = 0;
    for line in fixture.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let numbers = line
            .split_whitespace()
            .map(|number| {
                number
                    .parse::<u64>()
                    .expect("a number in the float fixture")
            })
            .collect::<Vec<_>>();
        assert_eq!(numbers.len(), 7, "a row needs seven numbers: {line}");
        let top_up = token_float_top_up(numbers[0], numbers[1], numbers[2], numbers[3], numbers[4]);
        assert_eq!(
            (top_up.0, top_up.1),
            (numbers[5], numbers[6]),
            "row: {line}"
        );
        rows += 1;
    }
    assert!(rows > 0, "the float fixture should hold vectors");
}

/// Task 3.2's refill path: the floats are lamports the program reads live, so the
/// buyer's plain transfers are the whole top-up — no limit change, no token
/// movement, and whatever is left comes back when the budget closes.
///
/// The key has to hold one payment's advance before the refund lands, so a key a
/// lamport short is refused; the refill is exactly the gap.
#[tokio::test]
async fn a_top_up_restores_a_float_short_of_one_payment() {
    let (mut context, world) = TokenWorld::start().await;
    let limit = TEST_TOKEN_BALANCE;
    let price = 200_000;
    open_token_budget(&mut context, &world, limit).await;

    // One lamport short of the advance a payment fronts: both records plus the
    // signature.
    let records = token_invoice_rent() + receipt_rent();
    context.set_account(
        &world.session.pubkey(),
        &system_account(records + TX_FEE - 1).into(),
    );
    let seller = Keypair::new().pubkey();
    seed_seller_token_account(&mut context, world.mint, seller);

    let error = send_with_fee_payer(
        &mut context,
        &[pay_from_token_budget_instruction(
            &world,
            world.mint,
            world.vault,
            seller,
            Keypair::new().pubkey(),
            price,
            "short",
        )],
        &world.session,
    )
    .await
    .expect_err("a float that cannot front the payment must refuse it");
    assert!(
        error.contains("Custom(1)"),
        "expected the session key's transfer to fail for want of lamports, got {error}"
    );
    assert_eq!(token_budget(&context, world.budget).await.spent, 0);

    // One refill, and the payment it could not fund goes through.
    top_up_token_floats(&mut context, &world).await;
    assert_eq!(
        lamports(&context, world.session.pubkey()).await,
        records + TX_FEE,
        "the refill brings the key to exactly one payment's advance"
    );
    pay_from_token_budget(&mut context, &world, seller, price).await;
    assert_eq!(
        token_balance(&context, associated_token_address(seller, world.mint)).await,
        price - token_fee(price)
    );
    assert_eq!(token_budget(&context, world.budget).await.spent, price);
}

/// Task 3.2's last criterion: the float can be topped up on its own. A refill is
/// the buyer's plain transfers, so the limit, the counter, the vault and the
/// buyer's token balance must be exactly what they were — only the two floats
/// move — and the payment they could not fund has to go through afterwards.
#[tokio::test]
async fn a_float_top_up_needs_no_limit_change() {
    let (mut context, world) = TokenWorld::start().await;

    let limit = TEST_TOKEN_BALANCE;
    let price = 200_000;
    open_token_budget(&mut context, &world, limit).await;

    // A key one lamport short of a payment's advance, and a seller whose account
    // exists: what refuses the payment is the float, which is what makes the
    // refill below the thing this test measures.
    let records = token_invoice_rent() + receipt_rent();
    context.set_account(
        &world.session.pubkey(),
        &system_account(records + TX_FEE - 1).into(),
    );
    let seller = Keypair::new().pubkey();
    seed_seller_token_account(&mut context, world.mint, seller);
    let error = send_with_fee_payer(
        &mut context,
        &[pay_from_token_budget_instruction(
            &world,
            world.mint,
            world.vault,
            seller,
            Keypair::new().pubkey(),
            price,
            "short",
        )],
        &world.session,
    )
    .await
    .expect_err("a float that cannot front the payment must refuse it");
    assert!(
        error.contains("Custom(1)"),
        "expected Custom(1), got {error}"
    );

    let opened = token_budget(&context, world.budget).await;
    let spent_before = opened.spent;
    let vault_before = token_balance(&context, world.vault).await;
    let buyer_tokens_before = token_balance(&context, world.buyer_token_account).await;

    // The refill itself: the buyer's transfers alone, no `open_token_budget`, so
    // no limit, no vault and no counter was touched.
    top_up_token_floats(&mut context, &world).await;

    // Each side now holds exactly one more payment's worth, and nothing else
    // about the budget moved.
    assert_eq!(
        lamports(&context, world.session.pubkey()).await,
        records + TX_FEE,
        "the session key should hold one more payment's advance"
    );
    assert_eq!(
        lamports(&context, world.budget).await,
        Rent::default().minimum_balance(TOKEN_BUDGET_SPACE),
        "the budget holds its rent and nothing more: the pool funds the records"
    );
    let after = token_budget(&context, world.budget).await;
    assert_eq!(
        after.monthly_limit, limit,
        "a float top-up must not change the limit"
    );
    assert_eq!(
        after.spent, spent_before,
        "a float top-up must not change what was spent"
    );
    assert_eq!(after.month_index, opened.month_index);
    assert_eq!(
        token_balance(&context, world.vault).await,
        vault_before,
        "a float top-up must not move tokens"
    );
    assert_eq!(
        token_balance(&context, world.buyer_token_account).await,
        buyer_tokens_before
    );

    // The seller the float could not fund now settles, and the counter follows.
    pay_from_token_budget(&mut context, &world, seller, price).await;
    assert_eq!(
        token_balance(&context, associated_token_address(seller, world.mint)).await,
        price - token_fee(price)
    );
    assert_eq!(
        token_budget(&context, world.budget).await.spent,
        spent_before + price
    );
}

/// Task 3.5's split on the silent path: the price pays everything. The seller
/// receives the price less the fee in tokens and no lamports at all, the fee lands
/// in the treasury's own account for the mint, the pool refunds the two records the
/// session key advanced and takes the closed invoice's rent back, and the vault
/// falls by exactly the price.
#[tokio::test]
async fn pay_from_token_budget_splits_the_price_and_closes_to_the_treasury() {
    let (mut context, world) = TokenWorld::start().await;
    open_token_budget(&mut context, &world, TEST_TOKEN_BALANCE).await;

    let seller = Keypair::new().pubkey();
    let seller_token_account = associated_token_address(seller, world.mint);
    // The seller's account is there before the payment: their setup cost, not
    // the buyer's.
    seed_seller_token_account(&mut context, world.mint, seller);
    let treasury_token_account = associated_token_address(treasury_pda(), world.mint);
    let reference = Keypair::new().pubkey();
    let amount = 700_000;
    let fee_t = token_fee(amount);
    let invoice = token_payment_pda(&seller, &reference, &world.mint, amount);

    let session_before = lamports(&context, world.session.pubkey()).await;
    let treasury_before = lamports(&context, treasury_pda()).await;
    let vault_before = token_balance(&context, world.vault).await;

    send_with_fee_payer(
        &mut context,
        &[pay_from_token_budget_instruction(
            &world,
            world.mint,
            world.vault,
            seller,
            reference,
            amount,
            "split",
        )],
        &world.session,
    )
    .await
    .expect("pay_from_token_budget failed");

    // The split, and the vault down by exactly the price.
    assert_eq!(
        token_balance(&context, seller_token_account).await,
        amount - fee_t
    );
    assert_eq!(token_balance(&context, treasury_token_account).await, fee_t);
    assert_eq!(
        token_balance(&context, world.vault).await,
        vault_before - amount
    );
    // The seller's proceeds are the price less the fee, and no lamports.
    assert_eq!(lamports(&context, seller).await, 0);

    // The session key got both rents back and paid only its own signature: the
    // seller's account existed before the payment, and the buyer funded none of
    // its rent.
    assert_eq!(
        lamports(&context, world.session.pubkey()).await,
        session_before - TX_FEE
    );

    // The invoice closed to the pool, which keeps its rent and pays the receipt's;
    // the receipt is the record that stays, and the session key is its payer.
    assert_eq!(lamports(&context, invoice).await, 0);
    assert_eq!(
        lamports(&context, treasury_pda()).await,
        treasury_before - receipt_rent()
    );
    let kept = receipt(&context, receipt_pda(&invoice)).await;
    assert_eq!(kept.invoice, invoice);
    assert!(kept.settled_at > 0);
    assert_eq!(token_budget(&context, world.budget).await.spent, amount);
}

/// Task 3.5's funding guard: the pool advances both records, so a pool that cannot
/// is refused before a base unit moves — the same rule `settle_payment` applies to
/// the receipt, now on the silent path.
#[tokio::test]
async fn pay_from_token_budget_rejects_a_treasury_that_cannot_fund_the_records() {
    let (mut context, world) = TokenWorld::start().await;
    open_token_budget(&mut context, &world, TEST_TOKEN_BALANCE).await;

    // A withdrawal stops at the reserve — one refund plus the account's rent —
    // so take the pool to exactly that and let a real settlement spend it: the
    // first token payment is funded out of the reserve, and only the rent floor
    // is left after it, which is the state the guard exists to refuse.
    let funded = lamports(&context, treasury_pda()).await;
    send(
        &mut context,
        &[withdraw_treasury_instruction(
            world.admin.pubkey(),
            funded - treasury_reserve(),
        )],
        &[&world.admin],
    )
    .await
    .expect("withdraw_treasury failed");
    assert_eq!(lamports(&context, treasury_pda()).await, treasury_reserve());

    let amount = 700_000;
    // Smaller than the second payment, so the month's limit still has room for
    // it: what has to refuse the second one is the pool, not the budget.
    let first = 300_000;
    let first_seller = Keypair::new().pubkey();
    seed_seller_token_account(&mut context, world.mint, first_seller);
    send_with_fee_payer(
        &mut context,
        &[pay_from_token_budget_instruction(
            &world,
            world.mint,
            world.vault,
            first_seller,
            Keypair::new().pubkey(),
            first,
            "reserve",
        )],
        &world.session,
    )
    .await
    .expect("the reserve must fund one token settlement");
    // The invoice's rent comes back to the pool when the invoice closes; the
    // marker's does not, so that is what the settlement spent out of the reserve.
    assert_eq!(
        lamports(&context, treasury_pda()).await,
        treasury_reserve() - receipt_rent(),
        "a settlement spends the refund the reserve held"
    );

    let seller = Keypair::new().pubkey();
    seed_seller_token_account(&mut context, world.mint, seller);
    let reference = Keypair::new().pubkey();
    let vault_before = token_balance(&context, world.vault).await;
    let error = send_with_fee_payer(
        &mut context,
        &[pay_from_token_budget_instruction(
            &world,
            world.mint,
            world.vault,
            seller,
            reference,
            amount,
            "poor pool",
        )],
        &world.session,
    )
    .await
    .expect_err("a pool that cannot fund the records must refuse the payment");
    assert!(
        error.to_string().contains(TREASURY_UNDERFUNDED),
        "expected {TREASURY_UNDERFUNDED}, got {error}"
    );

    // Nothing moved: the seller's account is theirs and still empty, and no
    // tokens, invoice, receipt or spend were made.
    assert_eq!(token_balance(&context, world.vault).await, vault_before);
    assert_eq!(
        token_balance(&context, associated_token_address(seller, world.mint)).await,
        0,
        "a refused payment must not move a base unit into the seller's account"
    );
    let invoice = token_payment_pda(&seller, &reference, &world.mint, amount);
    assert_eq!(lamports(&context, invoice).await, 0);
    assert!(account(&context, receipt_pda(&invoice)).await.is_none());
    assert_eq!(
        token_budget(&context, world.budget).await.spent,
        first,
        "the funded settlement's spend, and nothing from the refused one"
    );
}

/// The pool is the canonical one: a treasury with the right owner but the wrong
/// seeds is refused before it can fund anything, and with it the treasury's token
/// account, which is checked against the treasury it was passed with.
#[tokio::test]
async fn pay_from_token_budget_rejects_a_substituted_treasury() {
    let (mut context, world) = TokenWorld::start().await;
    open_token_budget(&mut context, &world, TEST_TOKEN_BALANCE).await;

    let seller = Keypair::new().pubkey();
    seed_seller_token_account(&mut context, world.mint, seller);
    let reference = Keypair::new().pubkey();
    let amount = 700_000;
    let mut instruction = pay_from_token_budget_instruction(
        &world,
        world.mint,
        world.vault,
        seller,
        reference,
        amount,
        "pool",
    );

    // The treasury's slot, swapped for a system account the program never derives.
    let stand_in = Keypair::new().pubkey();
    instruction.accounts[10].pubkey = stand_in;

    let error = send_with_fee_payer(&mut context, &[instruction], &world.session)
        .await
        .expect_err("a treasury that is not the pool must be refused");
    // Custom(2006) is Anchor's `ConstraintSeeds`.
    assert!(
        error.to_string().contains("Custom(2006)"),
        "expected Custom(2006), got {error}"
    );

    // Nothing moved: the vault still holds the price it did, and the budget is
    // unspent.
    assert_eq!(
        token_balance(&context, world.vault).await,
        TEST_TOKEN_BALANCE
    );
    assert_eq!(token_budget(&context, world.budget).await.spent, 0);
}

/// A `fee_bps` of 10,000 on the silent path: `fee_for`'s `amount - 1` cap leaves the
/// seller one base unit, exactly as on the payer's path.
#[tokio::test]
async fn a_ten_thousand_bps_policy_leaves_the_budget_seller_one_base_unit() {
    let (mut context, world) = TokenWorld::start().await;

    // Set the way an admin changes it, so this is a live `Config`.
    send(
        &mut context,
        &[update_config_instruction(
            world.admin.pubkey(),
            10_000,
            0,
            FeeMode::SellerPays,
            TEST_SESSION_FLOAT,
        )],
        &[&world.admin],
    )
    .await
    .expect("update_config failed");

    open_token_budget(&mut context, &world, TEST_TOKEN_BALANCE).await;
    let seller = Keypair::new().pubkey();
    let amount = 700_000;
    pay_from_token_budget(&mut context, &world, seller, amount).await;

    assert_eq!(
        token_balance(&context, associated_token_address(seller, world.mint)).await,
        1
    );
    assert_eq!(
        token_balance(
            &context,
            associated_token_address(treasury_pda(), world.mint)
        )
        .await,
        amount - 1
    );
}

/// Task 3.3's positive case: the canonical vault is drained into the buyer's own
/// account and closed, and both rents come back with it.
#[tokio::test]
async fn closing_a_token_budget_drains_the_canonical_vault_and_returns_its_rent() {
    let (mut context, world) = TokenWorld::start().await;
    open_token_budget(&mut context, &world, 500_000).await;
    pay_from_token_budget(&mut context, &world, Keypair::new().pubkey(), 200_000).await;
    assert_eq!(token_balance(&context, world.vault).await, 300_000);

    let buyer_before = lamports(&context, world.buyer.pubkey()).await;
    let budget_before = lamports(&context, world.budget).await;
    send(
        &mut context,
        &[close_token_budget_instruction(
            world.buyer.pubkey(),
            world.budget,
            world.mint,
            world.vault,
            world.buyer_token_account,
        )],
        &[&world.buyer],
    )
    .await
    .expect("close_token_budget failed");

    // What the vault still held is back in the buyer's own account.
    assert_eq!(
        token_balance(&context, world.buyer_token_account).await,
        TEST_TOKEN_BALANCE - 200_000
    );
    // The vault and the budget are gone, and everything they held came back
    // with them: the vault's rent plus the budget's rent and leftover float.
    assert_eq!(lamports(&context, world.vault).await, 0);
    assert_eq!(lamports(&context, world.budget).await, 0);
    assert_eq!(
        lamports(&context, world.buyer.pubkey()).await,
        buyer_before + token_account_rent() + budget_before
    );
}

/// Task 3.3's wrong-mint case on the close path: the mint argument must be the
/// budget's own, and a close against another mint strands nothing.
#[tokio::test]
async fn closing_a_token_budget_against_another_mint_fails_with_mint_mismatch() {
    let (mut context, world) = TokenWorld::start().await;
    open_token_budget(&mut context, &world, 500_000).await;

    // Another mint with its own canonical vault for the same budget, so every
    // account type and ATA constraint is satisfied and only the budget's own
    // mint can reject the close.
    let (other_mint, other_vault) = seed_other_mint(&mut context, &world, 400_000);
    let other_buyer_account = associated_token_address(world.buyer.pubkey(), other_mint);
    context.set_account(
        &other_buyer_account,
        &spl_token_account(other_mint, world.buyer.pubkey(), 0).into(),
    );

    let error = send(
        &mut context,
        &[close_token_budget_instruction(
            world.buyer.pubkey(),
            world.budget,
            other_mint,
            other_vault,
            other_buyer_account,
        )],
        &[&world.buyer],
    )
    .await
    .expect_err("a close against another mint must fail");
    assert!(
        error.contains(MINT_MISMATCH),
        "expected {MINT_MISMATCH}, got {error}"
    );

    // Nothing moved: the stand-in vault still holds its tokens and the budget is
    // still open with its own mint and its own vault.
    assert_eq!(token_balance(&context, other_vault).await, 400_000);
    assert_eq!(token_budget(&context, world.budget).await.mint, world.mint);
    assert_eq!(token_balance(&context, world.vault).await, 500_000);
}

/// Task 3.3's wrong-mint case on the pay path: the mint named must be the one
/// the budget was opened for.
#[tokio::test]
async fn paying_from_another_mints_vault_fails_with_mint_mismatch() {
    let (mut context, world) = TokenWorld::start().await;
    open_token_budget(&mut context, &world, 500_000).await;

    let (other_mint, other_vault) = seed_other_mint(&mut context, &world, 400_000);
    // The treasury's account for that mint too, so every account resolves and the
    // only thing left to reject is the budget's own mint.
    send(
        &mut context,
        &[open_treasury_token_account_instruction(
            world.admin.pubkey(),
            other_mint,
        )],
        &[&world.admin],
    )
    .await
    .expect("open_treasury_token_account failed");

    let other_seller = Keypair::new().pubkey();
    seed_seller_token_account(&mut context, other_mint, other_seller);
    let error = send(
        &mut context,
        &[pay_from_token_budget_instruction(
            &world,
            other_mint,
            other_vault,
            other_seller,
            Keypair::new().pubkey(),
            100_000,
            "vault",
        )],
        &[&world.session],
    )
    .await
    .expect_err("a payment from another mint must fail");
    assert!(
        error.contains(MINT_MISMATCH),
        "expected {MINT_MISMATCH}, got {error}"
    );

    assert_eq!(token_balance(&context, other_vault).await, 400_000);
    assert_eq!(token_budget(&context, world.budget).await.spent, 0);
    assert_eq!(token_balance(&context, world.vault).await, 500_000);
}

/// Task 3.3's decoy case: an account with the right mint and the right authority
/// but the wrong address is not a vault — neither for a payment, which would
/// pay a seller out of it, nor for a close, which would hand its whole balance
/// and its rent to the buyer.
#[tokio::test]
async fn a_decoy_vault_with_the_same_mint_and_authority_is_rejected() {
    let (mut context, world) = TokenWorld::start().await;
    open_token_budget(&mut context, &world, 500_000).await;

    // Same mint, same authority as the canonical vault, different address: a
    // place to strand the budget's tokens, not a vault.
    let decoy = Keypair::new().pubkey();
    context.set_account(
        &decoy,
        &spl_token_account(world.mint, world.budget, 1_000_000).into(),
    );

    let decoy_seller = Keypair::new().pubkey();
    seed_seller_token_account(&mut context, world.mint, decoy_seller);
    let error = send(
        &mut context,
        &[pay_from_token_budget_instruction(
            &world,
            world.mint,
            decoy,
            decoy_seller,
            Keypair::new().pubkey(),
            100_000,
            "vault",
        )],
        &[&world.session],
    )
    .await
    .expect_err("a decoy vault must be rejected");
    assert!(
        error.contains(NOT_ASSOCIATED_TOKEN_ACCOUNT),
        "expected {NOT_ASSOCIATED_TOKEN_ACCOUNT}, got {error}"
    );

    assert_eq!(token_balance(&context, decoy).await, 1_000_000);
    assert_eq!(token_balance(&context, world.vault).await, 500_000);
    assert_eq!(token_budget(&context, world.budget).await.spent, 0);

    // The close is where a vault's tokens would leave for good, so the decoy has
    // to be refused there too: the budget stays open with its own vault intact.
    let error = send(
        &mut context,
        &[close_token_budget_instruction(
            world.buyer.pubkey(),
            world.budget,
            world.mint,
            decoy,
            world.buyer_token_account,
        )],
        &[&world.buyer],
    )
    .await
    .expect_err("a decoy vault must not be closable in the real vault's place");
    assert!(
        error.contains(NOT_ASSOCIATED_TOKEN_ACCOUNT),
        "expected {NOT_ASSOCIATED_TOKEN_ACCOUNT}, got {error}"
    );
    assert_eq!(token_balance(&context, decoy).await, 1_000_000);
    assert_eq!(token_budget(&context, world.budget).await.mint, world.mint);
    assert_eq!(token_balance(&context, world.vault).await, 500_000);
}

/// Task 3.1's zero-fee case on the payer's token path, proved rather than
/// described: a zero `transfer` is still a CPI, and the token program refuses a
/// transfer into a frozen account even when it moves nothing. The treasury's
/// account for the mint is frozen here, so a settlement that succeeds is one that
/// skipped the fee transfer — and the control at the end shows the freeze is a
/// real refusal and not a no-op.
#[tokio::test]
async fn a_zero_fee_settle_token_payment_sends_no_fee_transfer() {
    let (mut context, world) = TokenWorld::start().await;

    // A 0% policy with no fixed fee, set the way an admin changes one.
    send(
        &mut context,
        &[update_config_instruction(
            world.admin.pubkey(),
            0,
            0,
            FeeMode::SellerPays,
            TEST_SESSION_FLOAT,
        )],
        &[&world.admin],
    )
    .await
    .expect("update_config failed");

    // The fee's destination, frozen: a transfer of any amount into it fails.
    let treasury_token_account = associated_token_address(treasury_pda(), world.mint);
    context.set_account(
        &treasury_token_account,
        &frozen_token_account(world.mint, treasury_pda()).into(),
    );

    let seller = Keypair::new().pubkey();
    let seller_token_account = associated_token_address(seller, world.mint);
    context.set_account(
        &seller_token_account,
        &spl_token_account(world.mint, seller, 0).into(),
    );
    let amount = 700_000;

    send(
        &mut context,
        &[settle_token_payment_instruction(
            world.buyer.pubkey(),
            seller,
            Keypair::new().pubkey(),
            world.mint,
            amount,
            "no fee",
        )],
        &[&world.buyer],
    )
    .await
    .expect("a zero-fee payment must settle without touching the fee account");

    // The seller has the whole price and the fee's account is untouched.
    assert_eq!(token_balance(&context, seller_token_account).await, amount);
    assert_eq!(token_balance(&context, treasury_token_account).await, 0);
    assert_eq!(
        token_balance(&context, world.buyer_token_account).await,
        TEST_TOKEN_BALANCE - amount
    );

    // The control: the same frozen account, now with a fee to send, refuses the
    // payment — so the settlement above skipped a transfer that would have
    // failed.
    send(
        &mut context,
        &[update_config_instruction(
            world.admin.pubkey(),
            TEST_FEE_BPS,
            TEST_FIXED_FEE,
            FeeMode::SellerPays,
            TEST_SESSION_FLOAT,
        )],
        &[&world.admin],
    )
    .await
    .expect("update_config failed");
    let refused_seller = Keypair::new().pubkey();
    let refused_token_account = associated_token_address(refused_seller, world.mint);
    context.set_account(
        &refused_token_account,
        &spl_token_account(world.mint, refused_seller, 0).into(),
    );
    // Smaller than the first invoice: the buyer paid that one already, and this
    // control is about the frozen account, not the balance.
    let control_amount = 100_000;
    let error = send(
        &mut context,
        &[settle_token_payment_instruction(
            world.buyer.pubkey(),
            refused_seller,
            Keypair::new().pubkey(),
            world.mint,
            control_amount,
            "fee",
        )],
        &[&world.buyer],
    )
    .await
    .expect_err("a fee into a frozen account must be refused");
    // Custom(17) is the token program's `AccountFrozen`.
    assert!(
        error.contains("Custom(17)"),
        "expected the token program's AccountFrozen, got {error}"
    );
    // Nothing moved on the refused payment.
    assert_eq!(token_balance(&context, refused_token_account).await, 0);
    assert_eq!(
        token_balance(&context, world.buyer_token_account).await,
        TEST_TOKEN_BALANCE - amount
    );
}

/// The same proof on the silent path: a zero fee means no fee CPI at all, so the
/// budget pays a seller with the treasury's account frozen, and the control shows
/// the frozen account would have refused a fee.
#[tokio::test]
async fn a_zero_fee_budget_payment_sends_no_fee_transfer() {
    let (mut context, world) = TokenWorld::start().await;

    send(
        &mut context,
        &[update_config_instruction(
            world.admin.pubkey(),
            0,
            0,
            FeeMode::SellerPays,
            TEST_SESSION_FLOAT,
        )],
        &[&world.admin],
    )
    .await
    .expect("update_config failed");

    let treasury_token_account = associated_token_address(treasury_pda(), world.mint);
    context.set_account(
        &treasury_token_account,
        &frozen_token_account(world.mint, treasury_pda()).into(),
    );

    let limit = TEST_TOKEN_BALANCE;
    let price = 200_000;
    open_token_budget(&mut context, &world, limit).await;
    let seller = Keypair::new().pubkey();
    pay_from_token_budget(&mut context, &world, seller, price).await;

    assert_eq!(
        token_balance(&context, associated_token_address(seller, world.mint)).await,
        price,
        "with no fee the seller receives the whole price"
    );
    assert_eq!(token_balance(&context, treasury_token_account).await, 0);
    assert_eq!(token_budget(&context, world.budget).await.spent, price);

    // The control: 2% into the same frozen account refuses the payment, and the
    // budget, the vault and the seller are as they were.
    send(
        &mut context,
        &[update_config_instruction(
            world.admin.pubkey(),
            TEST_FEE_BPS,
            TEST_FIXED_FEE,
            FeeMode::SellerPays,
            TEST_SESSION_FLOAT,
        )],
        &[&world.admin],
    )
    .await
    .expect("update_config failed");
    let refused_seller = Keypair::new().pubkey();
    seed_seller_token_account(&mut context, world.mint, refused_seller);
    let error = send_with_fee_payer(
        &mut context,
        &[pay_from_token_budget_instruction(
            &world,
            world.mint,
            world.vault,
            refused_seller,
            Keypair::new().pubkey(),
            price,
            "fee",
        )],
        &world.session,
    )
    .await
    .expect_err("a fee into a frozen account must be refused");
    assert!(
        error.to_string().contains("Custom(17)"),
        "expected the token program's AccountFrozen, got {error}"
    );
    assert_eq!(token_budget(&context, world.budget).await.spent, price);
    assert_eq!(token_balance(&context, world.vault).await, limit - price);
    assert_eq!(
        token_balance(
            &context,
            associated_token_address(refused_seller, world.mint)
        )
        .await,
        0,
        "the refused payment must leave the seller's own account untouched"
    );
}

// ---------------------------------------------------------------------------
// events
// ---------------------------------------------------------------------------

/// The raw payloads of the events a simulated transaction emits.
///
/// `process_transaction` reports only success, and `emit!` writes through
/// `sol_log_data`, which the runtime logs as `Program data: <base64>`. So the
/// events are read from a simulation of the same transaction: the instruction
/// runs for real, nothing is committed, and the log lines are ours to decode.
async fn simulated_events(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    signers: &[&Keypair],
) -> Vec<Vec<u8>> {
    let blockhash = context.last_blockhash;
    let payer = context.payer.pubkey();
    let mut all_signers: Vec<&Keypair> = vec![&context.payer];
    all_signers.extend_from_slice(signers);
    let tx =
        Transaction::new_signed_with_payer(instructions, Some(&payer), &all_signers, blockhash);
    let simulation = context
        .banks_client
        .simulate_transaction(tx)
        .await
        .expect("simulation failed");
    let logs = simulation
        .simulation_details
        .expect("the simulation returned no logs")
        .logs;
    let payloads: Vec<Vec<u8>> = logs
        .iter()
        .filter_map(|log| log.strip_prefix("Program data: "))
        .map(|encoded| {
            STANDARD
                .decode(encoded)
                .expect("a program data log is base64")
        })
        .collect();
    assert!(
        !payloads.is_empty(),
        "the simulated instruction logged no program data: {logs:?}"
    );
    payloads
}

/// The payloads that are events of type `T`, decoded. An event is the type's
/// 8-byte discriminator followed by its borsh body, so the fields below are
/// read exactly as the program wrote them.
fn events_of<T>(payloads: &[Vec<u8>]) -> Vec<T>
where
    T: AnchorDeserialize + Discriminator,
{
    let width = T::DISCRIMINATOR.len();
    payloads
        .iter()
        .filter_map(|payload| {
            let (discriminator, body) = payload.split_at(width);
            if discriminator != T::DISCRIMINATOR {
                return None;
            }
            Some(T::deserialize(&mut &body[..]).expect("the event payload did not decode"))
        })
        .collect()
}

/// The settle event is now the record: the price, the fee that went to the
/// treasury, what the seller keeps, who paid — and the two facts the marker no
/// longer stores, the invoice it closed and the note.
#[tokio::test]
async fn settle_payment_emits_the_split_it_paid() {
    let mut context = started().await;
    let payer = context.payer.pubkey();
    let seller = Keypair::new().pubkey();
    let reference = Keypair::new().pubkey();
    let amount = LAMPORTS_PER_SOL / 10;

    let payloads = simulated_events(
        &mut context,
        &[settle_instruction(
            payer, seller, reference, amount, "coffee",
        )],
        &[],
    )
    .await;

    let settled: Vec<PaymentSettled> = events_of(&payloads);
    assert_eq!(settled.len(), 1, "one event per settlement, no duplicates");
    let event = &settled[0];
    assert_eq!(event.amount, amount);
    assert_eq!(event.fee, fee(amount));
    assert_eq!(
        event.seller_proceeds,
        amount - fee(amount) - receipt_rent(),
        "the seller keeps the price less the fee and the marker's rent"
    );
    assert_eq!(event.payer, payer);
    assert_eq!(event.seller, seller);
    assert_eq!(
        event.mint,
        Pubkey::default(),
        "a SOL invoice is the empty mint"
    );
    assert_eq!(event.reference, reference);
    assert_eq!(
        event.invoice,
        payment_pda(&seller, &reference, amount),
        "the event names the invoice the QR carried"
    );
    assert_eq!(event.memo, "coffee", "the note moved here from the receipt");
}

/// The token path emits the same shape with the mint set, so an indexer reads
/// one event type for both currencies.
#[tokio::test]
async fn settle_token_payment_emits_the_mint_beside_the_split() {
    let (mut context, world) = TokenWorld::start().await;
    let seller = Keypair::new().pubkey();
    let seller_token_account = associated_token_address(seller, world.mint);
    // The seller's account is their own setup step, not part of the payment.
    context.set_account(
        &seller_token_account,
        &spl_token_account(world.mint, seller, 0).into(),
    );
    let reference = Keypair::new().pubkey();
    let amount = 700_000;

    let payloads = simulated_events(
        &mut context,
        &[settle_token_payment_instruction(
            world.buyer.pubkey(),
            seller,
            reference,
            world.mint,
            amount,
            "spl",
        )],
        &[&world.buyer],
    )
    .await;

    let settled: Vec<PaymentSettled> = events_of(&payloads);
    assert_eq!(settled.len(), 1);
    let event = &settled[0];
    assert_eq!(event.amount, amount);
    assert_eq!(event.fee, token_fee(amount));
    assert_eq!(event.seller_proceeds, amount - token_fee(amount));
    assert_eq!(event.payer, world.buyer.pubkey());
    assert_eq!(event.seller, seller);
    assert_eq!(event.mint, world.mint);
    assert_eq!(event.reference, reference);
}

/// The allowance path pays from the session key, and the event says so: `payer`
/// is the session key, not the wallet that funded the pot.
#[tokio::test]
async fn pay_from_token_budget_emits_the_session_key_as_the_payer() {
    let (mut context, world) = TokenWorld::start().await;
    let amount = 250_000;
    open_token_budget(&mut context, &world, 1_000_000).await;

    let seller = Keypair::new().pubkey();
    seed_seller_token_account(&mut context, world.mint, seller);
    let reference = Keypair::new().pubkey();
    let payloads = simulated_events(
        &mut context,
        &[pay_from_token_budget_instruction(
            &world,
            world.mint,
            world.vault,
            seller,
            reference,
            amount,
            "silent",
        )],
        &[&world.session],
    )
    .await;

    let settled: Vec<PaymentSettled> = events_of(&payloads);
    assert_eq!(settled.len(), 1);
    let event = &settled[0];
    assert_eq!(event.amount, amount);
    assert_eq!(event.fee, token_fee(amount));
    assert_eq!(event.seller_proceeds, amount - token_fee(amount));
    assert_eq!(event.payer, world.session.pubkey());
    assert_eq!(event.seller, seller);
    assert_eq!(event.mint, world.mint);
    assert_eq!(event.reference, reference);
}

/// The policy event carries what the admin installed, so a payment can be
/// priced at the block it settled in rather than with today's rate.
#[tokio::test]
async fn update_config_emits_the_policy_it_installed() {
    let (mut context, admin) = started_with_policy(
        None,
        TEST_FEE_BPS,
        TEST_FIXED_FEE,
        FeeMode::SellerPays,
        TREASURY_LAMPORTS,
        &[],
    )
    .await;

    let payloads = simulated_events(
        &mut context,
        &[update_config_instruction(
            admin.pubkey(),
            350,
            1_000,
            FeeMode::BuyerPays,
            TEST_SESSION_FLOAT,
        )],
        &[&admin],
    )
    .await;

    let updates: Vec<ConfigUpdated> = events_of(&payloads);
    assert_eq!(updates.len(), 1);
    let event = &updates[0];
    assert_eq!(event.admin, admin.pubkey());
    assert_eq!(event.policy.fee_bps_below, 350);
    assert_eq!(event.policy.flat_usdc_base_units, 1_000);
    assert_eq!(event.fee_mode, FeeMode::BuyerPays);
    assert_eq!(event.session_fee_float, TEST_SESSION_FLOAT);
}

/// Opening the SOL allowance says who may spend it and how much: the event is
/// the refill's own record, since `open_budget` also tops a returning buyer up.
#[tokio::test]
async fn open_budget_emits_the_limit_and_the_session_key() {
    let mut context = started().await;
    let buyer = Keypair::new();
    let session = Keypair::new();
    context.set_account(&buyer.pubkey(), &system_account(LAMPORTS_PER_SOL).into());
    let limit = LAMPORTS_PER_SOL / 20;

    let payloads = simulated_events(
        &mut context,
        &[open_budget_instruction(
            buyer.pubkey(),
            session.pubkey(),
            limit,
        )],
        &[&buyer],
    )
    .await;

    let opened: Vec<BudgetOpened> = events_of(&payloads);
    assert_eq!(opened.len(), 1);
    let event = &opened[0];
    assert_eq!(event.buyer, buyer.pubkey());
    assert_eq!(event.session_key, session.pubkey());
    assert_eq!(event.mint, Pubkey::default());
    assert_eq!(event.monthly_limit, limit);
}

/// The token allowance is the same event with the mint set, so one subscription
/// covers both budgets.
#[tokio::test]
async fn open_token_budget_emits_the_mint_and_the_limit() {
    let (mut context, world) = TokenWorld::start().await;
    let limit = 250_000;

    let payloads = simulated_events(
        &mut context,
        &[open_token_budget_instruction(
            world.buyer.pubkey(),
            world.budget,
            world.mint,
            world.buyer_token_account,
            world.vault,
            world.session.pubkey(),
            limit,
        )],
        &[&world.buyer],
    )
    .await;

    let opened: Vec<BudgetOpened> = events_of(&payloads);
    assert_eq!(opened.len(), 1);
    let event = &opened[0];
    assert_eq!(event.buyer, world.buyer.pubkey());
    assert_eq!(event.session_key, world.session.pubkey());
    assert_eq!(event.mint, world.mint);
    assert_eq!(event.monthly_limit, limit);
}
