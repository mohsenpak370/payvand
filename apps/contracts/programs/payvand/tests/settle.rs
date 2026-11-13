//! Contract test harness: an in-process bank plus the happy paths for the two
//! SOL settlement instructions.
//!
//! The tests run the program's real SBF binary — `target/deploy/payvand.so` —
//! inside `solana-program-test`, so they need no cluster, no
//! `solana-test-validator` process and no wallet, but they do need the program
//! built first:
//!
//! ```bash
//! cd apps/contracts && anchor build      # or: cargo build-sbf
//! cargo test --manifest-path apps/contracts/Cargo.toml
//! ```
//!
//! `task test:contracts`, `task anchor:test` and CI all build the program
//! before running the suite. The harness fails loudly (instead of skipping or
//! running an old artifact) when the `.so` is missing or older than `src/`.
//!
//! PDA helpers below derive addresses from the program's own `constants`
//! module, so a seed change breaks the tests instead of silently passing
//! against a copy.
//
// `solana-program-test` marks its API "agave-unstable" with deprecation
// warnings; this suite is written against that API deliberately.
#![allow(deprecated)]

use anchor_lang::{system_program::ID as SYSTEM_PROGRAM_ID, AccountDeserialize};
use payvand::constants::{BUDGET_SEED, BUDGET_SPACE, PAYMENT_SEED, PAYMENT_SPACE};
use payvand::state::{Budget, Payment, PaymentState};
use sha2::{Digest, Sha256};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;

const LAMPORTS_PER_SOL: u64 = 1_000_000_000;

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
            "{} is older than the program source: rebuild with `anchor build` (or `cargo build-sbf`)",
            program.display()
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

/// `["budget", buyer]` — the same seeds the program uses.
fn budget_pda(buyer: &Pubkey) -> Pubkey {
    let (pda, _bump) = Pubkey::find_program_address(&[BUDGET_SEED, buyer.as_ref()], &payvand::id());
    pda
}

/// Sends `instructions` with the test payer as fee payer, plus `signers`.
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

async fn payment(context: &ProgramTestContext, address: Pubkey) -> Payment {
    let account = account(context, address)
        .await
        .expect("payment account missing");
    Payment::try_deserialize(&mut account.data.as_slice()).expect("payment did not decode")
}

async fn budget(context: &ProgramTestContext, address: Pubkey) -> Budget {
    let account = account(context, address)
        .await
        .expect("budget account missing");
    Budget::try_deserialize(&mut account.data.as_slice()).expect("budget did not decode")
}

/// A fresh program-test instance whose payer also acts as the buyer.
async fn started() -> ProgramTestContext {
    program_test().start_with_context().await
}

// ---------------------------------------------------------------------------
// settle_payment
// ---------------------------------------------------------------------------

#[tokio::test]
async fn settle_payment_creates_and_settles_the_invoice() {
    let mut context = started().await;
    let payer = context.payer.pubkey();
    let seller = Keypair::new().pubkey();
    let reference = Keypair::new().pubkey();
    let amount = LAMPORTS_PER_SOL / 10;
    let memo = "coffee";
    let invoice = payment_pda(&seller, &reference, amount);

    let settle = Instruction {
        program_id: payvand::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(invoice, false),
            AccountMeta::new(seller, false),
            AccountMeta::new_readonly(reference, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: instruction_data("settle_payment", &amount_and_memo(amount, memo)),
    };

    send(&mut context, &[settle], &[])
        .await
        .expect("settle failed");

    // The payer paid exactly the invoice amount.
    assert_eq!(lamports(&context, seller).await, amount);

    // The invoice account records the payment.
    let payment = payment(&context, invoice).await;
    assert_eq!(payment.seller, seller);
    assert_eq!(payment.reference, reference);
    assert_eq!(payment.amount, amount);
    assert_eq!(payment.memo, memo);
    assert_eq!(payment.payer, payer);
    assert!(payment.settled_at > 0);
    assert_eq!(payment.state, PaymentState::Settled);
}

// ---------------------------------------------------------------------------
// open_budget + pay_from_budget
// ---------------------------------------------------------------------------

#[tokio::test]
async fn pay_from_budget_settles_the_invoice_and_records_the_spend() {
    let monthly_limit = LAMPORTS_PER_SOL / 4;
    let session = Keypair::new();
    let mut test = program_test();
    // The session key pays its own network fees and invoice rent.
    test.add_account(session.pubkey(), system_account(LAMPORTS_PER_SOL));
    let mut context = test.start_with_context().await;

    let buyer = context.payer.pubkey();
    let budget_address = budget_pda(&buyer);

    let open = Instruction {
        program_id: payvand::id(),
        accounts: vec![
            AccountMeta::new(buyer, true),
            AccountMeta::new(budget_address, false),
            AccountMeta::new(session.pubkey(), false),
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
    assert!(lamports(&context, budget_address).await >= monthly_limit);

    let seller = Keypair::new().pubkey();
    let reference = Keypair::new().pubkey();
    let amount = LAMPORTS_PER_SOL / 20;
    let memo = "silent coffee";
    let invoice = payment_pda(&seller, &reference, amount);

    let pay = Instruction {
        program_id: payvand::id(),
        accounts: vec![
            AccountMeta::new(session.pubkey(), true),
            AccountMeta::new(budget_address, false),
            AccountMeta::new(invoice, false),
            AccountMeta::new(seller, false),
            AccountMeta::new_readonly(reference, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: instruction_data("pay_from_budget", &amount_and_memo(amount, memo)),
    };
    send(&mut context, &[pay], &[&session])
        .await
        .expect("pay_from_budget failed");

    // The seller is paid from the pot, and the budget records exactly what it
    // spent for the invoice.
    assert_eq!(lamports(&context, seller).await, amount);
    let spent = budget(&context, budget_address).await;
    assert_eq!(spent.spent, amount);
    assert_eq!(spent.monthly_limit, monthly_limit);

    // The invoice records the session key as its payer.
    let payment = payment(&context, invoice).await;
    assert_eq!(payment.seller, seller);
    assert_eq!(payment.amount, amount);
    assert_eq!(payment.memo, memo);
    assert_eq!(payment.payer, session.pubkey());
    assert_eq!(payment.state, PaymentState::Settled);

    // The session key advanced and was refunded the invoice rent, so it is no
    // poorer by the rent — only by the network fees it signed for.
    let session_balance = lamports(&context, session.pubkey()).await;
    assert!(
        session_balance > LAMPORTS_PER_SOL - 10_000,
        "session key lost more than its fees: {session_balance}"
    );
}

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

// Sanity checks on the spaces the tests rely on; Task 1.4 turns these into the
// full layout assertions. `PAYMENT_SPACE` is 230 because the memo holds 96
// bytes; the token payment adds a mint and lands at 262.
#[test]
fn account_spaces_match_the_sizes_used_by_the_program() {
    assert_eq!(PAYMENT_SPACE, 230);
    assert_eq!(BUDGET_SPACE, 97);
}
