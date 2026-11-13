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
    BUDGET_SEED, BUDGET_SPACE, PAYMENT_MAX_MEMO, PAYMENT_RECEIPT_SEED, PAYMENT_RECEIPT_SPACE,
    PAYMENT_SEED, PAYMENT_SPACE,
};
use payvand::state::{Budget, Payment, PaymentReceipt, PaymentState, TokenPayment};
use sha2::{Digest, Sha256};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_pubkey::Pubkey;
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

/// `settle_payment` with the accounts in the order the program declares them.
fn settle_instruction(
    payer: Pubkey,
    seller: Pubkey,
    reference: Pubkey,
    amount: u64,
    memo: &str,
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
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: instruction_data("settle_payment", &amount_and_memo(amount, memo)),
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

/// What `init` puts in a receipt account; the payer fronts this and, in the
/// direct path, keeps paying it.
fn receipt_rent() -> u64 {
    Rent::default().minimum_balance(PAYMENT_RECEIPT_SPACE)
}

/// What `init` puts in a payment account, and what its close hands the seller.
fn invoice_rent() -> u64 {
    Rent::default().minimum_balance(PAYMENT_SPACE)
}

/// A fresh program-test instance whose payer also acts as the buyer.
async fn started() -> ProgramTestContext {
    program_test().start_with_context().await
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

    send(
        &mut context,
        &[settle_instruction(payer, seller, reference, amount, memo)],
        &[],
    )
    .await
    .expect("settle failed");

    // The seller receives the full price: `amount - rent` from the transfer and
    // the invoice rent when the account closes.
    assert_eq!(lamports(&context, seller).await, amount);

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

    // The payer paid the price plus the receipt's rent (the invoice rent comes
    // back to it through the seller), and nothing else beyond the fee.
    let paid = payer_before - lamports(&context, payer).await;
    assert_eq!(paid, amount + receipt_rent() + TX_FEE);
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
    let rent = invoice_rent();

    // Exactly the rent: the transfer would be zero and the seller's own close
    // would hand it back its own rent, so the program refuses it.
    let too_small = send(
        &mut context,
        &[settle_instruction(payer, seller, reference, rent, "dust")],
        &[],
    )
    .await;
    let error = too_small.expect_err("an invoice that only covers rent must be refused");
    assert!(
        error.contains(INVOICE_TOO_SMALL),
        "expected {INVOICE_TOO_SMALL}, got {error}"
    );

    // One lamport more is a real invoice: the seller is paid the difference and
    // gets the rent back through the close, so it nets exactly one lamport.
    let amount = rent + 1;
    send(
        &mut context,
        &[settle_instruction(payer, seller, reference, amount, "one")],
        &[],
    )
    .await
    .expect("the smallest valid invoice must settle");
    assert_eq!(lamports(&context, seller).await, amount);
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
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: instruction_data("pay_from_budget", &amount_and_memo(amount, memo)),
    };
    send(&mut context, &[pay], &[&session])
        .await
        .expect("pay_from_budget failed");

    // The seller is paid the full price from the pot, and the budget records
    // exactly what it spent on the invoice.
    assert_eq!(lamports(&context, seller).await, amount);
    let spent = budget(&context, budget_address).await;
    assert_eq!(spent.spent, amount);
    assert_eq!(spent.monthly_limit, monthly_limit);

    // The pot paid the price plus the receipt it had to create; the invoice
    // rent it fronted came back through the close.
    let pot_after = lamports(&context, budget_address).await;
    assert_eq!(pot_before - pot_after, amount + receipt_rent());

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
}
