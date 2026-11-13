# Plan: Contract Security Hardening

## Status

In Progress — revision 2 (2026-11-12); Phase 0 implemented 2026-11-13.

## Revision Notes

Revision 2 applies the review of 2026-11-12 and splits the 2026-11-09 draft in
two:

- This plan keeps the security work: the unsafe close path, replay, the
  receipt-backed settlement model, and the API/web status semantics.
- Fee policy, treasury, budget accounting, session-key floats, events, and
  Token-2022 moved to `plans/0002-fee-model-and-treasury.md`.

Changes from revision 1:

| Change | Reason |
| --- | --- |
| `pending` / `settled` / `missing` redefined against the receipt, not against `Payment.state` | Rev 1 defined `pending` as "a payment account exists with `state == Pending`", but the account is created and closed inside settle, so that state can never be read |
| Explicit `InvoiceTooSmall` guard before any `amount - rent` subtraction | Rev 1 only capped the fee; `amount - R_p` underflows for small invoices |
| Receipt uses `init_if_needed` plus a `settled_at` guard so `AlreadySettled` is reachable | Anchor's `init` runs before the handler, so a pre-`init` check could never execute; rev 1 also relied on Anchor's generic "account already in use" |
| Receipt rent is paid by the payer and disclosed here; `0002` moves it to the treasury | Keeps this plan free of Config; the one assertion this rewrites is called out in `0002` Task 1.2 |
| Regression tests land with the fix that makes them pass | Rev 1 asked for red tests in Phase 0 and added the CI job in Phase 0, leaving `main` red |
| Receipt layout test added | Prevents an under-allocated receipt from being discovered in production |
| Deploy order adds a pre-upgrade cleanup of old payment accounts | Old settled accounts would collide with the new `init` and make their invoices unpayable |

## Linked Docs

- `plans/README.md` — plan conventions and lifecycle.
- `plans/0002-fee-model-and-treasury.md` — the fee model that supersedes the
  interim rent handling here.
- `README.md` — project overview and the current Anchor claim.
- `apps/contracts/programs/payvand/src/lib.rs` — entrypoints and documented
  guarantees.
- `apps/contracts/programs/payvand/src/state.rs` — account layouts.
- `apps/contracts/programs/payvand/src/instructions/sol.rs` — SOL invoices and
  budgets.
- `apps/contracts/programs/payvand/src/instructions/token.rs` — token invoices
  and budgets.
- `apps/contracts/programs/payvand/src/constants.rs` — seeds, spaces, floats.
- `apps/api/src/payments.rs` — on-chain decoders and `PaymentStatus`.
- `apps/web/app/composables/useBudget.ts` — payment builders and UI checks.
- `apps/web/app/composables/usePaymentCheckout.ts` — direct settle builder.
- `apps/web/app/composables/usePaymentApi.ts` — status fetch and types.
- `apps/web/app/pages/pay.vue` — buyer checkout and exact-price display.
- `apps/web/app/assets/payvand.idl.json` — client IDL.

Findings 1, 2, 10, 11, 13, 14, 18, and the new 20, 21, 23 come from the
contract review of 2026-11-09 and the plan review of 2026-11-12.

## Objective

Remove the critical theft path, make settlement single-use, and give the API and
UI a status model that is true for an account that only exists inside the settle
transaction — then ship that as one small, verified devnet release, so the fee
model in `0002` starts from a correct base.

## Scope

In scope:

- Delete `close_payment` / `close_token_payment` and their contexts.
- Create a permanent receipt PDA on settle, close the payment account in the
  same transaction, and use the receipt's `settled_at` as the replay guard
  (with `AlreadySettled` as the named error).
- Define `pending` / `settled` / `missing` against the receipt and update the
  IDL, API decoder, and web types.
- Guard every `amount - rent` subtraction with `InvoiceTooSmall`.
- Return a named `AlreadySettled` error instead of an opaque `init` collision.
- Anchor contract tests for close removal, replay, auto-close, and the status
  decoder; a CI job that builds and tests the program.
- Anchor CLI/runtime version alignment.
- Receipt size/layout tests.
- Devnet rollout of the program, IDL, API, and web.

Out of scope (all in `0002`):

- Protocol fees, `Config`, the treasury, and the seller-pays fee split.
- Treasury-funded receipt rent. This plan advances the receipt rent from the
  payer and discloses it; `0002` replaces that with a treasury reimbursement.
- Budget accounting (`spent`), same-month refills, session-key floats.
- Token fee splitting, token budget float sizing, ATA rent policy.
- Events, Token-2022, `InitSpace` for the other accounts.
- Payer-side gasless/relayer support and mainnet deployment.

## Dependencies

Product decisions (made, so this plan can proceed):

- **Remove the close instructions rather than authorize them.** The app has no
  users and no pre-upgrade invoices worth keeping; deletion removes the attack
  surface instead of maintaining a dead path.
- **Receipt PDA, created with `init`, is the replay guard.** An in-place `Closed`
  tombstone would require a second instruction and a signature.
- **`pending` means "no receipt yet".** An unpaid invoice has no on-chain
  account at all, so `pending` cannot mean "an account exists in state
  Pending". `missing` is reserved for input the program could never have
  created — an unparseable address, a zero amount, or a wrong-length reference.
- **The receipt rent is paid by the payer in this release**, and the checkout
  discloses it. `0002` refunds it from the treasury so the payer pays exactly
  the displayed price. Accepted one-time rework: `0002` changes one balance
  assertion in Task 1.5 (see the note there).
- **No `Config` account in this release.** The receipt funding rule is a
  constant here and a policy in `0002`.

Technical decisions:

- **Anchor 1.2.1 with exact pins** (`anchor-lang = "=1.2.1"`,
  `anchor-spl = "=1.2.1"`, `anchor_version = "1.2.1"`). `Cargo.lock` already
  resolves 1.2.1 because `"1.1.2"` is a caret range; exact pins stop the drift
  from recurring.
- **The payment account's `state` byte stays in the layout** and the program
  writes `Settled` only. `PaymentState::Pending` is retained for layout/decoder
  compatibility and documented as never written.
- **Status is derived from receipt existence, never from `Payment.state`.**

Open questions:

- Who holds the program upgrade authority, and when is the devnet upgrade
  window? Needed for Phase 2.
- Can the existing devnet payment accounts be closed before the upgrade (Phase
  2, step 1), or must they be tolerated as unpayable?
- Does `anchor test` have a working local validator on the CI runner, or should
  the CI job run `cargo test` plus a `solana-program-test` suite instead?

## Findings To Phase Mapping

| Finding | Severity | Phase |
| --- | --- | --- |
| 1. Unused close handlers do not authorize the seller | Critical | 1 |
| 2. Settled invoices can be replayed after close | High | 1 |
| 10. No contract tests; CI does not build the program | Medium | 0 |
| 11. Anchor CLI/runtime version drift | Medium | 0 |
| 13. Close handlers never check `state == Settled` | Low | 1 |
| 14. `SellerMismatch` / `NotPending` become unused | Low | 1 |
| 18. Frontend carried a reclaim flow for invoice rent | Low | 1 |
| 20. `amount - rent` underflows for small invoices | High | 1 |
| 21. Status model contradicts an account that only exists during settle | High | 1 |
| 23. Replay collision surfaced as an opaque `init` error | Low | 1 |

## Phases

### Phase 0: Baseline, test harness, and version alignment

Goal: make the program buildable and testable in CI before changing behavior.

#### Task 0.1: Add the Anchor test harness

Description: Create the in-process contract suite in
`apps/contracts/programs/payvand/tests/settle.rs` (the plan's Rust
`solana-program-test` option) and run it through the existing
`[scripts] test = "cargo test"` hook, so `anchor test` and `task verify` execute
it without a validator, a wallet, or a cluster. The Rust suite is chosen over a
TypeScript harness because the repo has no Anchor JS client and no test runner,
and because the helpers can import the program's own seed constants instead of
duplicating them.

Helpers: `payment_pda(seller, reference, amount)` and `budget_pda(buyer)` derive
from `PAYMENT_SEED` / `BUDGET_SEED`; instruction data is built as
`sha256("global:<name>")[..8]` plus Borsh args.

The suite executes the **built SBF artifact**: `ProgramTest` is constructed with
no native processor and `SBF_OUT_DIR` pointing at `target/deploy`, so
`solana-program-test` loads `payvand.so` and runs it through the same entrypoint
the cluster uses. No unsafe lifetime shim is involved. Consequences that belong
in the tooling: the program must be built before the suite runs, so
`task test:contracts` builds it first, and the harness fails with a rebuild hint
when the `.so` is missing or older than `src/` rather than testing an old
artifact. One test asserts the program account is executable and BPF-loader
owned, which proves the artifact is what ran.

Write the first tests for the happy paths only; tests for a bug land in the task
that fixes it, so `main` is never red for a known reason.

Acceptance criteria:

- [x] `cargo test --manifest-path apps/contracts/Cargo.toml` executes a
      happy-path `settle_payment` and a happy-path `pay_from_budget`.
- [x] The suite runs the built `target/deploy/payvand.so`, and a test proves
      the program account is executable and BPF-loader owned.
- [x] A missing or stale artifact fails with a rebuild hint instead of
      silently passing.
- [x] `anchor test` still passes, building the program first and then running
      the same suite through `[scripts] test`.
- [x] Test helpers derive payment and budget PDAs from the program's seeds and
      constants.
- [x] The harness is documented in `DEV.md`.
- [x] A `task anchor:test` target builds with Anchor and runs the suite; the
      existing `task test:contracts` runs the suite alone.

Verification:

- [x] `cargo test --manifest-path apps/contracts/Cargo.toml`
- [x] `cd apps/contracts && anchor test` (with the CLI pinned in Task 0.3)
- [x] `cargo check -p payvand` still passes.

Files likely touched:

- `apps/contracts/programs/payvand/tests/settle.rs` (new)
- `apps/contracts/programs/payvand/Cargo.toml` (dev-dependencies)
- `apps/contracts/Anchor.toml`
- `Taskfile.yml`
- `DEV.md`

Dependencies: None.

Estimated scope: Medium.

#### Task 0.2: Add a CI contract job

Description: Add a job to `.github/workflows/ci.yml` for `apps/contracts` that
installs Rust and the Solana CLI, caches the Cargo build and the Solana
platform tools, builds the deployable artifact with `cargo build-sbf`, and then
runs `cargo test --manifest-path apps/contracts/Cargo.toml`. Because the suite
executes the `.so`, CI must produce the same SBF binary a deploy would upload;
Anchors' own CLI is not needed, so `avm` stays a local concern.

Acceptance criteria:

- [x] CI builds the deployable SBF program (`cargo build-sbf`) and runs the
      contract suite against it on every push.
- [x] CI fails on a compile error or a failing contract test.
- [x] The existing web/API jobs are unchanged.
- [x] No test in the suite is committed in a knowingly failing state, so the
      job is green on `main` from the moment it lands.

Verification:

- [ ] Open a draft PR that touches a contract file and confirm the job runs.
- [x] `task verify` runs the contract suite through `test:contracts`; the
      SBF build and suite run together only in `task anchor:test`.

Files likely touched:

- `.github/workflows/ci.yml`
- `Taskfile.yml`

Dependencies: Task 0.1.

Estimated scope: Small.

#### Task 0.3: Align the Anchor CLI and runtime versions

Description: `Anchor.toml` declares CLI `1.1.2`, `Cargo.toml` requires
`anchor-lang = "1.1.2"` (a caret range), and `Cargo.lock` already resolves
`1.2.1`. Pin everything to `=1.2.1` and make the toolchain, `Anchor.toml`,
`README.md`, and the lockfile agree.

Acceptance criteria:

- [x] `anchor --version` prints the version in `Anchor.toml`.
- [x] `Cargo.toml` uses exact `=1.2.1` pins for `anchor-lang` and `anchor-spl`.
- [x] `Cargo.lock` resolves exactly 1.2.1 (no caret resolution to a newer
      minor).
- [x] `README.md` states the same version.
- [x] `anchor build` passes with it, and the contract suite runs on the pinned
      runtime.

Verification:

- [x] `anchor --version`
- [x] `cargo tree -p payvand | rg 'anchor-(lang|spl)'`
- [x] `cd apps/contracts && anchor build`
- [x] `cargo test --manifest-path apps/contracts/Cargo.toml`

Files likely touched:

- `apps/contracts/Anchor.toml`
- `apps/contracts/programs/payvand/Cargo.toml`
- `apps/contracts/Cargo.lock`
- `README.md`

Dependencies: Task 0.1.

Estimated scope: Small.

---

### Phase 1: Remove the unsafe close path and prevent replay

Goal: eliminate the critical theft path, make settlement single-use, and give
the API and UI a status model that matches an account which only exists inside
the settle transaction.

#### Task 1.1: Remove the unused close instructions

Description: Delete `close_payment`, `close_token_payment`, the
`ClosePayment` / `CloseTokenPayment` contexts, and their handlers. `ClosePayment`
closes to a `seller: Signer` that is never compared to `payment.seller`, so any
signer can close a settled invoice and take its rent. Removal deletes the
attack surface instead of adding a check to dead code.

Acceptance criteria:

- [ ] Neither instruction exists in `lib.rs`, `sol.rs`, or `token.rs`.
- [ ] Both account contexts and their constraints are deleted.
- [ ] `SellerMismatch` / `NotPending` are removed if nothing else uses them.
- [ ] No client, API, or IDL code references either instruction.
- [ ] The program builds and the existing tests pass.

Verification:

- [ ] `cargo check -p payvand`
- [ ] `grep -R "close_payment\|close_token_payment" apps/contracts apps/api apps/web`
      returns no matches.
- [ ] Anchor test: the IDL has no close instruction.

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/sol.rs`
- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/contracts/programs/payvand/src/lib.rs`
- `apps/contracts/programs/payvand/src/errors.rs`
- `apps/web/app/assets/payvand.idl.json`
- `apps/contracts/tests/payvand.ts`

Dependencies: Phase 0.

Estimated scope: Small.

#### Task 1.2: Add a permanent receipt PDA and auto-close the payment account

Description: Separate the two jobs the payment account does today — holding
rent and proving an invoice was settled. The payment account becomes
short-lived (created, settled, and closed in one transaction); the receipt keeps
the record and its `settled_at` is the replay guard.

Why the guard is a state check and not a pre-`init` check: Anchor resolves and
creates `init` accounts while loading the instruction, before the handler body
runs. A handler-level `require!(receipt.data_is_empty())` therefore always sees
the account Anchor just created on the first settle, and on a replay it never
runs at all, because `init` fails first with a system error. A named
`AlreadySettled` is only reachable if the account is loaded with
`init_if_needed` and the handler inspects its state, so that is what this task
specifies — for the receipt only, never for the payment account.

New accounts (`state.rs`):

- `PaymentReceipt { seller, reference, amount, memo, payer, created_at,
  settled_at, closed_at, bump }`.
- `TokenPaymentReceipt { seller, reference, mint, amount, memo, payer,
  created_at, settled_at, closed_at, bump }`.

New seeds and spaces (`constants.rs`):

- `PAYMENT_RECEIPT_SEED = b"payment-receipt"`, PDA
  `[PAYMENT_RECEIPT_SEED, seller, reference, amount]`.
- `TOKEN_PAYMENT_RECEIPT_SEED = b"token-payment-receipt"`, PDA
  `[TOKEN_PAYMENT_RECEIPT_SEED, seller, reference, mint, amount]`.
- `PAYMENT_RECEIPT_SPACE = 237`, `TOKEN_PAYMENT_RECEIPT_SPACE = 269`
  (`8 + 32 + 32 + 8 + 4 + 96 + 32 + 8 + 8 + 8 + 1`, plus `32` for the mint).

Settlement changes, for all four paths (`settle_payment`,
`pay_from_budget`, `settle_token_payment`, `pay_from_token_budget`):

1. Create the payment account with `init` (never `init_if_needed`): it exists
   only inside the settle transaction, so a repeat settle cannot recreate it.
2. Load the receipt with `init_if_needed`, `payer = payer`, the receipt seeds,
   and the receipt space. `init_if_needed` is required only to make the named
   error reachable (see above); it is safe because the receipt is a
   program-owned PDA that nothing but settle can create, and because
   `settled_at` is written in the same instruction that creates it — so
   "receipt exists" always means "receipt settled".
3. Add `close = seller` to the payment account so its rent returns in the same
   transaction. Anchor closes it last.
4. As the first handler statement, `require!(receipt.settled_at == 0,
   PaymentError::AlreadySettled)` (finding 23), then set `settled_at`. On a
   first settle the field is zero because `init_if_needed` wrote the default;
   on a replay it is non-zero and the instruction fails with the named error.
   Keep the payment account's `init` as the structural backstop: even if the
   receipt check were removed, the payment `init` still prevents a replay.
   Document the one gap: if a third party pre-funds an empty receipt PDA with
   lamports, `init_if_needed` fails deserialization instead of returning
   `AlreadySettled`; the invoice remains unpaid and the error is not
   misreported as settled.
5. Guard the arithmetic before subtracting rent (finding 20):
   `require!(amount > payment_rent, InvoiceTooSmall)` so
   `amount - payment_rent` cannot underflow. `payment_rent` is
   `Rent::get()?.minimum_balance(PAYMENT_SPACE)` (2,491,680 lamports at the
   current rent rate: `Payment` is 230 bytes, since the memo holds 96).
6. Fund the receipt from the payer in this release. Payer total for a direct
   SOL invoice is `amount + receipt_rent` (+ network fee); the checkout
   discloses the receipt rent as the permanent record. `0002` replaces this
   with a treasury reimbursement so the payer pays exactly `amount` — the only
   assertion that changes then is the payer-balance line in Task 1.5.
7. Set `receipt.closed_at = receipt.settled_at`, because the payment account
   closes immediately. Document `settled` as terminal.

Acceptance criteria:

- [ ] Both receipt types exist with the fields above.
- [ ] Receipt seeds are shared by the program and the client.
- [ ] Only the receipt uses `init_if_needed`; the payment account uses `init`.
- [ ] `AlreadySettled` is the first check in each settle handler and reads
      `receipt.settled_at`, not lamports.
- [ ] A first settle creates exactly one receipt and closes the payment
      account in the same transaction.
- [ ] The seller receives the payment account's rent in that transaction, with
      no separate seller transaction.
- [ ] A second settle fails with `AlreadySettled`.
- [ ] An invoice whose amount is at or below the payment rent fails with
      `InvoiceTooSmall` instead of panicking.
- [ ] The receipt is readable afterwards and preserves memo, `created_at`, and
      `settled_at`.
- [ ] No close or reclaim instruction exists anywhere.

Verification:

- [ ] Anchor test: direct SOL settle → payment account gone, receipt exists,
      seller balance increased by the payment-account rent.
- [ ] Anchor test: budget SOL settle → same, and the pot paid exactly the
      invoice amount plus the rents it advanced.
- [ ] Anchor test: settle twice → the second call fails with
      `AlreadySettled` (assert the error code, not a raw system error).
- [ ] Anchor test: the receipt created by a first settle has `settled_at > 0`
      and `closed_at == settled_at`.
- [ ] Anchor test: `amount == payment_rent` and `amount == payment_rent + 1` →
      `InvoiceTooSmall` and success respectively.
- [ ] `anchor test` and `cargo check -p payvand` pass.

Files likely touched:

- `apps/contracts/programs/payvand/src/state.rs`
- `apps/contracts/programs/payvand/src/constants.rs`
- `apps/contracts/programs/payvand/src/errors.rs`
- `apps/contracts/programs/payvand/src/instructions/sol.rs`
- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/contracts/tests/payvand.ts`
- `apps/web/app/composables/useBudget.ts` (settle builders)
- `apps/web/app/composables/usePaymentCheckout.ts` (direct settle builder)

Dependencies: Task 1.1.

Estimated scope: Medium.

#### Task 1.3: Define and wire the status model

Description: Define the three statuses once and implement them identically in
the API, IDL, and web client. An unpaid invoice has **no on-chain account**, so
the status must be derived from the receipt, not from `Payment.state` (today
`apps/api/src/payments.rs:645` maps `state == 1` to `Settled` and everything
else to `Pending`, and `payments.rs:387` returns `Missing` when no account is
readable — that would label every unpaid invoice "missing").

Semantics:

- `pending` — the receipt PDA is empty and the invoice parameters are valid
  (amount > 0, reference is 32 bytes, seller parses). The invoice is simply
  unpaid. **This is the normal state of a QR that has not been paid.**
- `settled` — the receipt exists. Terminal; `closed_at == settled_at`.
- `missing` — reserved for input that can never name a Payvand invoice:
  unparseable base58, zero amount, wrong-length reference. An RPC failure is an
  error response, not `missing`.

Acceptance criteria:

- [ ] The IDL includes both receipt types and the settle instructions list the
      receipt accounts.
- [ ] The API returns `pending` for a valid, unpaid invoice and `settled` when
      a receipt exists, even though the payment account is gone.
- [ ] `missing` is returned only for invalid parameters, and the API has a test
      that proves an unpaid valid invoice is not `missing`.
- [ ] The API serializes statuses as lowercase snake_case and has no `Closed`
      variant and no close decoder.
- [ ] The web `PaymentStatus` type is exactly `'pending' | 'settled' |
      'missing'`, and the status label handles each one.
- [ ] The buyer checkout refuses a non-pending invoice; the seller history
      renders a settled invoice from the receipt.
- [ ] The receipt rent disclosed at checkout matches what Task 1.2 charges.

Verification:

- [ ] API unit tests: pending (absent receipt), settled (present receipt),
      missing (invalid params) for both account types.
- [ ] API test: a valid invoice with no account is `pending`, not `missing`.
- [ ] Web typecheck passes.
- [ ] Localnet invoice settled end-to-end; the API reports `settled` with the
      receipt visible after the payment account is gone.

Files likely touched:

- `apps/web/app/assets/payvand.idl.json` (regenerated)
- `apps/api/src/payments.rs`
- `apps/web/app/composables/usePaymentApi.ts`
- `apps/web/app/composables/usePaymentCheckout.ts`
- `apps/web/app/pages/pay.vue`, `apps/web/app/pages/shops/[id].vue`
  (status display)

Dependencies: Task 1.2.

Estimated scope: Medium.

#### Task 1.4: Assert receipt and payment layouts

Description: Add a test asserting serialized sizes equal the space constants,
so a later field addition cannot silently under-allocate an account. Keep the
hand-written constants here; `0002` converts `Config` and the budgets to
`InitSpace` where that is safe.

Acceptance criteria:

- [ ] A `cargo test` asserts `Payment`, `TokenPayment`, `PaymentReceipt`,
      `TokenPaymentReceipt` serialized sizes match their space constants.
- [ ] Adding a field without updating the constant makes the test fail.
- [ ] Account creation still succeeds on chain.

Verification:

- [ ] `cargo test -p payvand`
- [ ] `anchor test`

Files likely touched:

- `apps/contracts/programs/payvand/src/state.rs`
- `apps/contracts/programs/payvand/src/constants.rs`

Dependencies: Task 1.2.

Estimated scope: Small.

#### Task 1.5: Add security regression tests

Description: Lock in Phase 1 behavior with tests that would fail on the old
program.

Acceptance criteria:

- [ ] Direct SOL settle creates the receipt, closes the payment account, and
      returns its rent to the seller in one transaction.
- [ ] Budget SOL settle does the same, and the pot paid exactly the invoice
      amount plus the rents it advanced.
- [ ] A second settle fails with `AlreadySettled`.
- [ ] The removed close instructions are absent from the IDL.
- [ ] The receipt preserves memo, `created_at`, and `settled_at` after the
      payment account is gone.
- [ ] `InvoiceTooSmall` boundary is covered.
- [ ] Direct SOL: the payer's net delta is exactly `amount + receipt_rent` plus
      the network fee (the assertion `0002` changes).

Verification:

- [ ] `cd apps/contracts && anchor test`
- [ ] CI contract job is green.

Files likely touched:

- `apps/contracts/tests/payvand.ts`

Dependencies: Tasks 1.1–1.4.

Note for `0002`: Task 1.2 item 6 makes the payer bear the receipt rent; `0002`
Task 1.2 replaces it with treasury reimbursement, and the only assertion that
changes is the payer balance delta (from `amount + receipt_rent` to `amount`).

Estimated scope: Medium.

---

### Phase 2: Verification, rollout, and archival

Goal: prove the release end-to-end and deploy it in an order that cannot leave
the API unable to describe a settled payment.

#### Task 2.1: Full verification

Acceptance criteria:

- [ ] `task verify` passes, including `anchor test`.
- [ ] `cd apps/contracts && anchor build && anchor test` passes.
- [ ] `cargo clippy -p payvand -- -D warnings` passes.
- [ ] The IDL is regenerated and copied to
      `apps/web/app/assets/payvand.idl.json`.
- [ ] API tests cover receipt-backed `settled`, absent-receipt `pending`, and
      invalid-input `missing`.
- [ ] The web app typechecks and the affected pages work on localnet.

Verification:

- [ ] `task verify`
- [ ] `cd apps/contracts && anchor test`
- [ ] Localnet smoke test (Task 2.2).

Files likely touched:

- `apps/web/app/assets/payvand.idl.json`
- Any file changed in Phase 0–1.

Dependencies: Phases 0–1.

Estimated scope: Medium.

#### Task 2.2: Devnet rollout

Description: Deploy in this order and smoke-test the real flows. Unlike a fresh
deployment, this is an **upgrade**: the program ID must not change, and old
accounts from the current program must be dealt with first.

Rollout order:

1. **Pre-upgrade cleanup.** Settled payment accounts created by the current
   program still exist under
   `[payment, seller, reference, amount]`. After the upgrade, the same invoice
   can no longer be created (`init` collides) and cannot be settled again. Close
   them with the current `close_payment` / `close_token_payment` instructions
   before upgrading, or record them as unpayable. Decide per the open question
   above.
2. Confirm the upgrade authority and back up
   `apps/contracts/target/deploy/payvand-keypair.json`.
3. `anchor build`, then `anchor upgrade` (not `anchor deploy`) with the program
   ID and program keypair.
4. Copy the new IDL to `apps/web/app/assets/payvand.idl.json`.
5. Deploy the API before the web client; it must understand receipt-backed
   `settled` and absent-receipt `pending`.
6. Deploy the web app.
7. Smoke-test.

Smoke tests:

- [ ] Pay a SOL invoice from a wallet: the seller received the price minus the
      payment-account rent (this release has no protocol fee), the payment
      account is gone, the receipt remains, and the payer paid the disclosed
      receipt rent.
- [ ] Pay a SOL invoice from a budget: the receipt exists, the payment account
      is gone, and the session key is made whole for the rents it advanced.
- [ ] Replay the same invoice: `AlreadySettled`.
- [ ] An unpaid invoice reports `pending`; a settled one reports `settled`;
      a garbled reference reports `missing`.
- [ ] The checkout displays the receipt rent it will charge.
- [ ] The deployed IDL has no close instruction.

Acceptance criteria:

- [ ] The program ID is unchanged.
- [ ] The IDL in the web app matches the deployed program.
- [ ] The smoke tests pass.
- [ ] No user-facing message describes the old close/reclaim flow.

Verification:

- [ ] `solana program show <PROGRAM_ID>`
- [ ] The devnet checklist above.
- [ ] API and web logs are clean.

Files likely touched:

- `apps/web/app/assets/payvand.idl.json`
- `apps/api` deployment config (if any)
- `README.md` / `DEV.md` (deployment notes)

Dependencies: Task 2.1.

Estimated scope: Medium.

#### Task 2.3: Update docs and archive this plan

Acceptance criteria:

- [ ] `README.md` states the aligned Anchor version, the receipt-backed
      `settled` status, and that no close or reclaim instruction exists.
- [ ] `plans/README.md` lists this plan as completed and archived, and
      `0002` remains active.
- [ ] The plan file moves to `plans/archive/`.
- [ ] The temporarily payer-funded receipt rent is recorded in `0002` as the
      state this plan leaves behind.

Verification:

- [ ] Doc review.
- [ ] `git status` shows only intended changes.

Files likely touched:

- `README.md`
- `plans/README.md`
- `plans/archive/0001-contract-security-hardening.md`

Dependencies: Task 2.2.

Estimated scope: Small.

---

## Checkpoints

- [ ] **Checkpoint 0:** `anchor test` runs and is green, the CI job exists,
      Anchor versions are pinned and agree.
- [ ] **Checkpoint 1:** Close instructions are gone; a settle creates a receipt
      and closes the payment account atomically; a replay fails with
      `AlreadySettled`; small amounts fail with `InvoiceTooSmall`; layouts are
      asserted; API and web report `pending`/`settled`/`missing` correctly.
- [ ] **Checkpoint 2:** `task verify`, `anchor test`, and the devnet smoke tests
      pass on an upgraded program with an unchanged ID; the plan is archived and
      `0002` records its starting state.

## Risks And Mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| API and web deploy after the program, leaving `settled` unreadable | High | Deploy API before web, in the same window; API tests cover receipt-backed status. |
| Old devnet payment accounts block `init` for their invoices | Medium | Pre-upgrade cleanup step closes them with the current instructions, or they are recorded as unpayable. |
| The receipt rent disclosure is easy to miss and the payer pays more than the QR showed | Medium | Show it at checkout, in the settle event payload, and in the seller-facing note; `0002` removes the charge entirely. |
| Invoices at or below the payment-account rent (~0.00249 SOL) are unpayable in this release | Medium | `InvoiceTooSmall` fails cleanly instead of panicking; the create-payment page blocks amounts below the minimum; `0002` keeps the same minimum once the fee is added. |
| Someone pre-funds an empty receipt PDA, so a replay reports a deserialization error instead of `AlreadySettled` | Low | The guard reads state, not lamports, so a replay never reports success; the payment account's `init` remains the structural backstop. Documented as a known error-code edge, not a settlement path. |
| A later field addition under-allocates the receipt | Medium | Task 1.4 layout test fails before deployment. |
| Someone reinstates a "reclaim" instruction to recover receipt rent | Medium | The plan and README state the model; receipts are permanent by design, and `0002` pays their rent from the treasury. |

## Definition Of Done

- [ ] All appendix findings are checked.
- [ ] `task verify`, `anchor test`, and
      `cargo clippy -p payvand -- -D warnings` pass.
- [ ] The devnet smoke tests pass on an upgraded program with an unchanged ID.
- [ ] No close or reclaim instruction exists in the program, IDL, API, or web
      client.
- [ ] `pending`, `settled`, and `missing` are defined once and implemented
      identically in API and web, with tests.
- [ ] A settled invoice cannot be replayed, and small invoices fail cleanly.
- [ ] The IDL, API, and web client were deployed together.
- [ ] Docs match the deployed behavior.
- [ ] The plan is archived and listed in `plans/README.md`, and `0002` records
      the state it inherits.

## Appendix: Findings Checklist

- [ ] **1 (Critical):** No close instruction exists, so no signer can take a
      settled invoice's rent.
- [ ] **2 (High):** A settled invoice cannot be replayed; the receipt guard
      returns `AlreadySettled`, and the payment account's `init` is the
      structural backstop.
- [ ] **10 (Medium):** Contract tests exist and CI builds/tests the program.
- [ ] **11 (Medium):** Anchor CLI and runtime versions are pinned and equal.
- [ ] **13 (Low):** The `state == Settled` close check is moot; the handler is
      gone.
- [ ] **14 (Low):** `SellerMismatch` / `NotPending` are removed.
- [ ] **18 (Low):** No reclaim flow exists in the frontend; rent returns in the
      settle transaction.
- [ ] **20 (High):** `InvoiceTooSmall` guards every `amount - rent`
      subtraction.
- [ ] **21 (High):** `pending` means "no receipt"; `missing` means invalid
      input; neither depends on `Payment.state`.
- [ ] **23 (Low):** A repeat settle returns `AlreadySettled`, not an opaque
      `init` error.
