//! Contract test harness: an in-process bank plus the settlement paths for the
//! SOL instructions, including the Phase 1 receipt and replay rules.
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
//! PDA helpers below derive addresses from the program's own `constants`
//! module, so a seed change breaks the tests instead of silently passing
//! against a copy.
//!
// `solana-program-test` marks its API "agave-unstable" with deprecation
// warnings; this suite is written against that API deliberately.
#![allow(deprecated)]

use anchor_lang::prelude::Rent;
use anchor_lang::{system_program::ID as SYSTEM_PROGRAM_ID, AccountDeserialize};
use payvand::constants::{
    BUDGET_SEED, BUDGET_SPACE, CONFIG_SEED, CONFIG_SPACE, MIN_SESSION_FEE_FLOAT, PAYMENT_MAX_MEMO,
    PAYMENT_RECEIPT_SEED, PAYMENT_RECEIPT_SPACE, PAYMENT_SEED, PAYMENT_SPACE, TREASURY_SEED,
};
use payvand::state::{
    Budget, Config, FeeMode, Payment, PaymentReceipt, PaymentState, TokenPayment,
};
use sha2::{Digest, Sha256};
use solana_account::Account;
use solana_clock::Clock;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_pubkey::Pubkey;
use solana_sdk_ids::bpf_loader_upgradeable;
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
    ProgramTest::new("payvand", payvand::id(), None)
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

/// `["config"]` — the singleton fee policy.
fn config_pda() -> Pubkey {
    Pubkey::find_program_address(&[CONFIG_SEED], &payvand::id()).0
}

/// `["treasury"]` — the only account a fee may be sent to.
fn treasury_pda() -> Pubkey {
    Pubkey::find_program_address(&[TREASURY_SEED], &payvand::id()).0
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

/// Borsh `initialize_config` / `update_config` arguments.
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
    let mut data = fee_bps.to_le_bytes().to_vec();
    data.extend_from_slice(&fixed_fee_lamports.to_le_bytes());
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

async fn config(context: &ProgramTestContext, address: Pubkey) -> Config {
    let account = account(context, address)
        .await
        .expect("config account missing");
    Config::try_deserialize(&mut account.data.as_slice()).expect("config did not decode")
}

/// What `init` puts in a receipt account; the payer fronts it and the treasury
/// refunds it.
fn receipt_rent() -> u64 {
    Rent::default().minimum_balance(PAYMENT_RECEIPT_SPACE)
}

/// What `init` puts in a payment account, and what its close hands the seller.
fn invoice_rent() -> u64 {
    Rent::default().minimum_balance(PAYMENT_SPACE)
}

/// The treasury's own rent-exempt minimum: it holds no data.
fn treasury_rent() -> u64 {
    Rent::default().minimum_balance(0)
}

/// The seller-side fee under the default test policy.
fn fee(amount: u64) -> u64 {
    let proportional = (amount as u128 * TEST_FEE_BPS as u128 / 10_000) as u64;
    (proportional + TEST_FIXED_FEE).min(amount - 1)
}

/// The smallest amount the policy accepts: the first one above
/// `invoice_rent + fee`.
fn min_payable() -> u64 {
    let mut amount = invoice_rent();
    loop {
        if amount > invoice_rent() + fee(amount) {
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

    // The seller absorbs the fee: `amount - rent - fee` now and the invoice rent
    // when the account closes.
    assert_eq!(lamports(&context, seller).await, amount - fee(amount));

    // The invoice account is gone — settled invoices do not linger on chain.
    assert!(
        account(&context, invoice).await.is_none(),
        "the invoice account should be closed by the settle transaction"
    );

    // The receipt survives it, carries the invoice facts, and is exactly the
    // layout the program allocates.
    let written = receipt(&context, receipt_address).await;
    assert_eq!(written.invoice, invoice);
    assert_eq!(written.seller, seller);
    assert_eq!(written.reference, reference);
    assert_eq!(written.mint, Pubkey::default());
    assert_eq!(written.amount, amount);
    assert_eq!(written.memo, memo);
    assert_eq!(written.payer, payer);
    assert!(written.created_at > 0);
    assert_eq!(written.settled_at, written.created_at);
    assert_eq!(written.closed_at, written.settled_at);
    let receipt_account = account(&context, receipt_address)
        .await
        .expect("receipt account missing");
    assert_eq!(receipt_account.data.len(), PAYMENT_RECEIPT_SPACE);

    // The payer paid exactly the price (the treasury refunded the receipt rent
    // the payer fronted) plus the network fee.
    let paid = payer_before - lamports(&context, payer).await;
    assert_eq!(paid, amount + TX_FEE);

    // The treasury keeps the fee and loses the receipt rent it refunded.
    let treasury_delta = lamports(&context, treasury_pda()).await as i128 - treasury_before as i128;
    assert_eq!(treasury_delta, fee(amount) as i128 - receipt_rent() as i128);
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
    assert_eq!(lamports(&context, seller).await, floor - fee(floor));
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
    send(&mut context, &[pay], &[&session])
        .await
        .expect("pay_from_budget failed");

    // The seller absorbs the fee, and the budget records exactly what it spent
    // on the invoice.
    assert_eq!(lamports(&context, seller).await, amount - fee(amount));
    let spent = budget(&context, budget_address).await;
    assert_eq!(spent.spent, amount);
    assert_eq!(spent.monthly_limit, monthly_limit);

    // The pot pays exactly the price: the treasury refunds the receipt rent the
    // session key fronted, and the invoice rent comes back through the close.
    let pot_after = lamports(&context, budget_address).await;
    assert_eq!(pot_before - pot_after, amount);

    // The treasury keeps the fee and loses the receipt rent it refunded.
    let treasury_delta =
        lamports(&context, treasury_pda()).await as i128 - TREASURY_LAMPORTS as i128;
    assert_eq!(treasury_delta, fee(amount) as i128 - receipt_rent() as i128);

    // The invoice is closed and the receipt records the session key as payer.
    assert!(
        account(&context, invoice).await.is_none(),
        "the invoice account should be closed by the settle transaction"
    );
    let written = receipt(&context, receipt_address).await;
    assert_eq!(written.invoice, invoice);
    assert_eq!(written.seller, seller);
    assert_eq!(written.amount, amount);
    assert_eq!(written.memo, memo);
    assert_eq!(written.mint, Pubkey::default());
    assert_eq!(written.payer, session.pubkey());
    assert!(written.closed_at > 0);

    // The session key advanced both rents and was refunded both, and the test
    // payer covers the network fee, so its balance is untouched.
    let session_balance = lamports(&context, session.pubkey()).await;
    assert_eq!(session_balance, LAMPORTS_PER_SOL);
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
    assert_eq!(written.fee_bps, TEST_FEE_BPS);
    assert_eq!(written.fixed_fee_lamports, TEST_FIXED_FEE);
    assert_eq!(written.fee_mode, FeeMode::SellerPays);
    assert_eq!(written.session_fee_float, TEST_SESSION_FLOAT);
    assert_eq!(written.version, 1);
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

#[tokio::test]
async fn settle_payment_rejects_a_treasury_that_cannot_fund_the_receipt() {
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
    let amount = LAMPORTS_PER_SOL / 10;

    let attempt = send(
        &mut context,
        &[settle_instruction(
            payer, seller, reference, amount, "coffee",
        )],
        &[],
    )
    .await;
    let error = attempt.expect_err("an empty treasury must refuse the settlement");
    assert!(
        error.contains(TREASURY_UNDERFUNDED),
        "expected {TREASURY_UNDERFUNDED}, got {error}"
    );
    assert_eq!(
        lamports(&context, seller).await,
        0,
        "a refused settlement must not pay the seller"
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

#[tokio::test]
async fn fixed_fee_is_charged_on_top_of_the_rate() {
    let fixed = 1_000_000u64;
    let mut context = started_with_policy(
        None,
        TEST_FEE_BPS,
        fixed,
        FeeMode::SellerPays,
        TREASURY_LAMPORTS,
        &[],
    )
    .await
    .0;
    let payer = context.payer.pubkey();
    let seller = Keypair::new().pubkey();
    let reference = Keypair::new().pubkey();
    let amount = LAMPORTS_PER_SOL / 10;
    let expected_fee = amount / 100 * 2 + fixed;

    send(
        &mut context,
        &[settle_instruction(
            payer, seller, reference, amount, "coffee",
        )],
        &[],
    )
    .await
    .expect("settle with a fixed fee failed");

    assert_eq!(lamports(&context, seller).await, amount - expected_fee);
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
    assert_eq!(updated.fee_bps, 100);
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

    assert_eq!(lamports(&context, seller).await, amount);
    // The treasury still funds the receipt rent, so it only goes down.
    assert_eq!(
        lamports(&context, treasury_pda()).await,
        treasury_before - receipt_rent()
    );
}

#[tokio::test]
async fn withdraw_treasury_requires_the_admin_and_keeps_the_rent_minimum() {
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
    let surplus = lamports(&context, treasury).await - treasury_rent();

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
    let error = overdraft.expect_err("the treasury must keep its rent-exempt minimum");
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
    assert_eq!(lamports(&context, treasury).await, treasury_rent());
    // The test payer covers the network fee, not the admin.
    assert_eq!(lamports(&context, admin).await, payer_before + surplus);
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
        seller: Pubkey::from(key),
        reference: Pubkey::from(key),
        mint: Pubkey::from(key),
        amount: 1,
        memo: memo.clone(),
        payer: Pubkey::from(key),
        created_at: 1,
        settled_at: 2,
        closed_at: 2,
        bump: 255,
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

    let config = Config {
        admin: Pubkey::from(key),
        fee_bps: 200,
        fixed_fee_lamports: 1,
        fee_mode: FeeMode::SellerPays,
        session_fee_float: 30_000_000,
        bump: 255,
        treasury_bump: 254,
        version: 1,
    };
    assert_eq!(borsh_len(&config) + 8, CONFIG_SPACE);
}

// The spaces are hand-written byte sums; a pin on the arithmetic catches a
// change to the constant itself, and the test above catches a change to a
// struct that the constant was not updated for.
#[test]
fn account_spaces_match_the_sizes_used_by_the_program() {
    assert_eq!(PAYMENT_SPACE, 230);
    assert_eq!(BUDGET_SPACE, 97);

    // One receipt type for both currencies: invoice, seller, reference, mint,
    // amount, memo, payer, three timestamps, bump. No `state` field — the
    // account's existence is the state.
    let receipt = 8 + 32 + 32 + 32 + 32 + 8 + 4 + PAYMENT_MAX_MEMO + 32 + 8 + 8 + 8 + 1;
    assert_eq!(PAYMENT_RECEIPT_SPACE, receipt);
    assert_eq!(PAYMENT_RECEIPT_SPACE, 301);

    // admin, fee_bps, fixed_fee_lamports, fee_mode, session_fee_float, bump,
    // treasury_bump, version.
    assert_eq!(CONFIG_SPACE, 8 + 32 + 2 + 8 + 1 + 8 + 1 + 1 + 1);
    assert_eq!(CONFIG_SPACE, 62);
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
