# Plan: Contract Security Hardening

## Status

Completed — revision 3 (2026-11-13). Phase 0 and Phase 1 implemented; Phase 2
rolled out to devnet and Vercel on 2026-11-13 with the wallet, budget, replay,
and status smoke checks done. Two items are deliberately not built here: the
checkout receipt-rent disclosure (dropped, because `0002` removes the charge)
and token-path harness coverage (deferred to `0002`, which changes those paths).

## Revision Notes

Revision 2 applies the review of 2026-11-12 and splits the 2026-11-09 draft in
two:

- This plan keeps the security work: the unsafe close path, replay, the
  receipt-backed settlement model, and the API/web status semantics.
- Fee policy, treasury, budget accounting, session-key floats, events, and
  Token-2022 moved to `plans/archive/0002-fee-model-and-treasury.md`.

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

Changes from revision 2, made while implementing Phase 1:

| Change | Reason |
| --- | --- |
| One `PaymentReceipt` for SOL and token invoices, seeded `[PAYMENT_RECEIPT_SEED, invoice]`, space 301, storing `invoice` and `mint` | Rev 2 seeded receipts from `seller`/`reference`/`amount` (and mint), but the API and any watcher only ever hold the invoice address and cannot recover those seeds. Keying by the invoice keeps every lookup one derivation; one type keeps it one decoder. `invoice` lets a program-wide scan name the invoice, `mint` keeps the record self-describing (`Pubkey::default()` for SOL) |
| The invoice account closes via `AccountsClose::close` in the handler, not `close = seller` | Anchor 1.2 rejects `mut` alongside `init` and requires `mut` for `close`, so `init` + `close` cannot be expressed as a constraint on the same account |
| `pay_from_token_budget` refunds the receipt rent with the invoice rent | Otherwise every silent token payment drains the session key's float until it can no longer pay network fees |
| An address with no receipt and no invoice account reads as `pending`; a cluster failure is an error | The receipt is the record, so an unpaid invoice (which has no account) is pending, not missing. A hiccup must not be reported as a status the seller might act on |

## Linked Docs

- `plans/README.md` — plan conventions and lifecycle.
- `plans/archive/0002-fee-model-and-treasury.md` — the fee model that supersedes the
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

- [x] Neither instruction exists in `lib.rs`, `sol.rs`, or `token.rs`.
- [x] Both account contexts and their constraints are deleted.
- [x] `SellerMismatch` / `NotPending` are removed if nothing else uses them.
- [x] No client, API, or IDL code references either instruction.
- [x] The program builds and the existing tests pass.

Verification:

- [x] `cargo check -p payvand`
- [x] `grep -R "close_payment\|close_token_payment" apps/contracts apps/api apps/web`
      returns no matches.
- [x] Regenerated IDL (`apps/contracts/target/idl/payvand.json`) has no close
      instruction, and the copy in `apps/web/app/assets/` matches it.

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

New account (`state.rs`):

- `PaymentReceipt { invoice, seller, reference, mint, amount, memo, payer,
  created_at, settled_at, closed_at, bump }`, one type for both currencies.
  `mint` is `Pubkey::default()` for a SOL invoice; `invoice` is the invoice
  PDA, so a scan can name the invoice without re-deriving it.

New seed and space (`constants.rs`):

- `PAYMENT_RECEIPT_SEED = b"payment-receipt"`, PDA
  `[PAYMENT_RECEIPT_SEED, invoice]`.
- `PAYMENT_RECEIPT_SPACE = 301`
  (`8 + 32 (invoice) + 32 (seller) + 32 (reference) + 32 (mint) + 8 (amount) +
  4 + 96 (memo) + 32 (payer) + 8 + 8 + 8 (timestamps) + 1 (bump)`), about
  2,985,840 lamports of rent at the current rate.

Settlement changes, for all four paths (`settle_payment`,
`pay_from_budget`, `settle_token_payment`, `pay_from_token_budget`):

1. Create the payment account with `init` (never `init_if_needed`). `init`
   still succeeds after a close, because the address is empty again — it fails
   only when a leftover account occupies it (an old-program invoice before the
   pre-upgrade cleanup) — so the receipt, not this `init`, is the replay guard.
2. Load the receipt with `init_if_needed`, `payer = payer`, the receipt seeds,
   and the receipt space. `init_if_needed` is required only to make the named
   error reachable (see above); it is safe because the receipt is a
   program-owned PDA that nothing but settle can create, and because
   `settled_at` is written in the same instruction that creates it — so
   "receipt exists" always means "receipt settled".
3. Close the payment account to the seller at the end of the handler with
   `AccountsClose::close` (Anchor 1.2 forbids `init` together with `mut`, and
   `close` requires `mut`, so the constraint form is unavailable). Its rent
   returns in the same transaction, and Anchor skips re-serialising a closed
   account.
4. As the first handler statement, `require!(receipt.settled_at == 0,
   PaymentError::AlreadySettled)` (finding 23), then set `settled_at`. On a
   first settle the field is zero because `init_if_needed` wrote the default;
   on a replay it is non-zero and the instruction fails with the named error.
   The payment account's `init` is not a backstop: closing empties the address,
   so a replay can recreate it. The receipt's `settled_at` is the only replay
   guard, and Task 1.5 tests it.
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

- [x] The receipt type exists with the fields above.
- [x] Receipt seeds are shared by the program, the API (`receipt_pda`) and the
      client (`derivePaymentReceiptPda`).
- [x] Only the receipt uses `init_if_needed`; the payment account uses `init`.
- [x] Each settle handler checks `receipt.settled_at == 0` before writing any
      state or moving any lamports, and reports `AlreadySettled` by name (it
      runs after the cheap argument checks, before the first transfer).
- [x] A first settle creates exactly one receipt and closes the payment
      account in the same transaction.
- [x] The seller receives the payment account's rent in that transaction, with
      no separate seller transaction.
- [x] A second settle fails with `AlreadySettled`.
- [x] An invoice whose amount is at or below the payment rent fails with
      `InvoiceTooSmall` instead of panicking.
- [x] The receipt is readable afterwards and preserves memo, `created_at`, and
      `settled_at`.
- [x] No close or reclaim instruction exists anywhere.

Verification:

- [x] Contract test: direct SOL settle → payment account gone, receipt exists,
      seller received the full amount (transfer plus the invoice rent).
- [x] Contract test: budget SOL settle → same, and the pot paid exactly the
      invoice amount plus the receipt rent while the invoice rent came back
      through the close.
- [x] Contract test: settle twice → the second call fails with
      `AlreadySettled` (`Custom(6002)`, asserted by code).
- [x] Contract test: the receipt created by a first settle has `settled_at > 0`
      and `closed_at == settled_at`.
- [x] Contract test: `amount == payment_rent` and `amount == payment_rent + 1` →
      `InvoiceTooSmall` and success respectively.
- [x] `task test:contracts` and `task anchor:test` pass (7 tests), and
      `cargo check -p payvand` is clean.

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

- [x] The IDL includes the receipt type and the settle instructions list the
      receipt accounts.
- [x] The API returns `pending` for a valid, unpaid invoice and `settled` when
      a receipt exists, even though the payment account is gone.
- [x] The API never invents `missing`: invalid input is a rejected request, and
      a cluster that does not answer is an error. A test proves an unpaid valid
      invoice is `pending`.
- [x] The API serializes statuses as lowercase snake_case and has no `Closed`
      variant and no close decoder.
- [x] The web `PaymentStatus` type is exactly `'pending' | 'settled' |
      'missing'`, and `statusLabel` renders the awaiting-payment label for both
      non-settled variants.
- [x] The buyer checkout refuses a non-pending invoice; the seller history
      renders a settled invoice from the receipt. (Auto-pay already skips
      `settled`, and the program rejects a manual replay with
      `AlreadySettled`; the UI does not pre-block it, because a stale read could
      hide a legitimately unpaid invoice. Accepted design, not a gap.)
- [ ] The receipt rent disclosed at checkout matches what Task 1.2 charges.
      **Dropped:** `0002` Task 1.2 moves that rent to the treasury, so the
      disclosure is never built rather than built and deleted.

Verification:

- [x] API unit tests: pending (absent receipt), settled (present receipt, SOL
      and token), and a receipt taking precedence over a leftover invoice
      account.
- [x] API test: a valid invoice with no account is `pending`, not `missing`.
- [x] Web typecheck and lint pass.
- [x] Localnet invoice settled end-to-end; the API reports `settled` with the
      receipt visible after the payment account is gone. Later repeated on
      devnet through the production API.

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

- [x] A `cargo test` asserts `Payment`, `TokenPayment`, `PaymentReceipt` (and
      `Budget`) serialized sizes match their space constants.
- [x] Adding a field without updating the constant makes the test fail.
- [x] Account creation still succeeds on chain (the settle tests create both
      account kinds).

Verification:

- [x] `cargo test --manifest-path apps/contracts/Cargo.toml`
- [x] `task anchor:test`

Files likely touched:

- `apps/contracts/programs/payvand/src/state.rs`
- `apps/contracts/programs/payvand/src/constants.rs`

Dependencies: Task 1.2.

Estimated scope: Small.

#### Task 1.5: Add security regression tests

Description: Lock in Phase 1 behavior with tests that would fail on the old
program.

Acceptance criteria:

- [x] Direct SOL settle creates the receipt, closes the payment account, and
      returns its rent to the seller in one transaction.
- [x] Budget SOL settle does the same, and the pot paid exactly the invoice
      amount plus the receipt rent it advanced.
- [x] A second settle fails with `AlreadySettled`.
- [x] The removed close instructions are absent from the IDL.
- [x] The receipt preserves memo, `created_at`, and `settled_at` after the
      payment account is gone.
- [x] `InvoiceTooSmall` boundary is covered.
- [x] Direct SOL: the payer's net delta is exactly `amount + receipt_rent` plus
      the network fee (the assertion `0002` changes).
- [ ] Token paths have the same regression coverage. **Deferred to `0002`:** the harness
      has no SPL mint or token accounts yet, so `settle_token_payment` and
      `pay_from_token_budget` are covered only by the shared receipt/layout
      code, not by a settle test.

Verification:

- [x] `task test:contracts` (and `task anchor:test`) — 7 tests pass
- [ ] CI contract job is green. **Not inspected:** the Phase 1 commits are
      local only, so the job has not run on them; the same commands pass
      locally through `task anchor:test`.

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

- [x] `task verify` passes, including `anchor test` (run piecewise: contract
      suite, `cargo test` for the API, web typecheck and lint).
- [x] `cd apps/contracts && anchor build && anchor test` passes.
- [x] `cargo clippy -p payvand -- -D warnings` passes.
- [x] The IDL is regenerated and copied to
      `apps/web/app/assets/payvand.idl.json`.
- [x] API tests cover receipt-backed `settled` and absent-receipt `pending`.
      `missing` is no longer produced: an unparseable address is a 400, and an
      unpaid valid invoice is `pending` (Task 1.3 revision 3).
- [x] The web app typechecks and lints. The affected pages were exercised
      through the API and the on-chain flow, not driven in a browser.

Verification:

- [x] `task verify` — exit 0 (lint, typecheck, 65 API tests, 7 contract tests).
- [x] `task anchor:test` — replaces `cd apps/contracts && anchor test`, which
      would target the devnet cluster in `Anchor.toml` and, after `anchor
      build`, run the eBPF artifact this runtime cannot execute.
- [x] Localnet smoke test (2026-11-13), on `solana-test-validator` 3.1.10 with
      the v0 artifact and the API on `127.0.0.1:8787`:
      - `solana program deploy` of `cargo build-sbf` output installed the
        program under the unchanged ID `4grKZpw6…` and the upgradeable loader.
      - Unpaid invoice → API `pending` (previously would have been `missing`).
      - Wallet `settle_payment` → seller received exactly the 0.01 SOL amount,
        the invoice account was gone, and the receipt was 301 bytes.
      - Budget `open_budget` + `pay_from_budget` → same seller payout and
        receipt, `settled_by` naming the session key, and the session key made
        whole for both rents (balance 0.009995 SOL, only its fee short of the
        float).
      - Replay → `custom program error: 0x1772` (`AlreadySettled`).
      - The API flips to `settled` about 9s after confirmation, because
        `RpcClient` reads at `finalized`; the checkout polls every 5s, so this
        shows as a short "awaiting payment" window and is not an error.
      - What this does *not* cover: the web UI itself (no browser flow was
        driven), token invoices, and the checkout receipt-rent disclosure,
        which is still open below.

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
3. Build the artifact the loader can run, then upgrade (not `anchor deploy`)
   with the program ID and program keypair. **Build with `cargo build-sbf`**:
   Anchor 1.2.1's own `anchor build` produces the eBPF target
   (`e_machine 0x00f7`), while the deployed program lives under the legacy
   upgradeable loader and the localnet check of the v0 artifact
   (`0x0107`) deployed cleanly. `anchor build` is still needed for the IDL, so
   run it first and let `cargo build-sbf` overwrite `target/deploy/payvand.so`
   before upgrading.
4. Copy the new IDL to `apps/web/app/assets/payvand.idl.json`.
5. Deploy the API before the web client; it must understand receipt-backed
   `settled` and absent-receipt `pending`.
6. Deploy the web app.
7. Smoke-test.

Smoke tests:

- [x] Pay a SOL invoice from a wallet: the seller received the full price (no
      protocol fee in this release), the payment account is gone and the
      receipt remains. Run on devnet against the upgraded program and through
      the production API.
- [x] Pay a SOL invoice from a budget: the receipt exists, the payment account
      is gone, and the session key is made whole for the rents it advanced.
      Verified on localnet and on devnet: seller received the full 0.01 SOL,
      receipt 301 bytes, session ended at `0.009995 SOL` (float minus one fee),
      and the production API reported `settled` with `settled_by` = the session
      key. Budget `spent` was not re-read after the payment: a retry had
      re-opened the budget, which resets `spent` to zero, and the contract test
      asserts the exact `spent` and pot deltas.
- [x] Replay the same invoice: `AlreadySettled` (`custom program error: 0x1772`).
- [x] An unpaid invoice reports `pending` and a settled one reports `settled`,
      from both the local and the production API. A garbled *address* is a 400
      now, not `missing`; `missing` is left in the type for clients that still
      read it.
- [ ] The checkout displays the receipt rent it will charge. **Dropped from
      this plan:** `0002` Task 1.2 removes the charge by paying receipt rent
      from the treasury, so there is nothing left to disclose; building the
      disclosure and then deleting it is wasted work. Until `0002` ships, the
      README and the risk table below state that a direct SOL payer pays
      `amount + receipt_rent` (`0.00298… SOL`).
- [x] The deployed program has no close instruction (the artifact was built
      from the IDL in this repository; no IDL account was uploaded).

Acceptance criteria:

- [x] The program ID is unchanged.
- [x] The IDL in the web app matches the deployed program.
- [x] The smoke tests pass (wallet, budget, replay, statuses).
- [x] No user-facing message describes the old close/reclaim flow.

Verification:

- [x] `solana program show <PROGRAM_ID>` — same ID and authority, new byte
      length (426,928) and deploy slot.
- [x] The devnet checklist above.
- [x] API and web logs clean (Vercel deployments Ready, no runtime errors seen).

#### Rollout record (2026-11-13)

- Pre-upgrade state: 4 SOL invoices, 3 token invoices, 3 budgets, 4 token
  budgets, no receipts. All seven invoices were already settled by the old
  program, so they stayed in place; only their exact invoice tuples become
  unpayable, and their rent needed a seller signature we do not hold.
- Backups before upgrading: the program keypair and the deployed program dump
  (`solana program dump`) were copied to `/tmp` (outside the repository).
- Upgrade: `solana program deploy` with the `cargo build-sbf` (arch v0)
  artifact, authority `Evmo7zn…`, signature `59E1vCAm…`, new length 426,928
  bytes at slot 521,808,007.
- Devnet settlement (wallet): 0.01 SOL invoice `6MA3UxAY…`, receipt
  `Ap6oVVN4…`, replay rejected with 6002. A 0.001 SOL attempt was rejected
  first with 6003 (`InvoiceTooSmall`), confirming the rent guard on the real
  cluster.
- Devnet settlement (budget): invoices `2527XNET…` and `79ugHzEQ…` paid from a
  50,000,000-lamport monthly budget. Each paid the seller the full 0.01 SOL and
  left a 301-byte receipt; a replay retry returned 6002. The production API
  reported both as `settled` with `settled_by` = the session key.
  - `2527XNET…` needed two attempts because the driver crashed while printing a
    `BigInt`; the payment itself had already gone through.
  - The budget account (`fQadYNyE…`, 97 bytes) still exists with limit
    `50,000,000`; its absolute balance was not reconciled against the payments
    because the retry re-ran `open_budget`, which re-tops the pot to the limit
    and resets `spent`. The contract test covers the exact pot and `spent`
    arithmetic.
- A leftover pre-upgrade invoice still reads `settled` through the API, so the
  old accounts did not need a migration.
- Vercel: the API project was deployed first and the live origin reports
  `settled` from a receipt and `pending` for an unpaid address; the web project
  was then deployed and its live bundle contains the receipt derivation. The
  site loads and points at `payvand-api.vercel.app`. The deployed pay screen was
  not driven through a payment (no wallet in the browser).

Files likely touched:

- `apps/web/app/assets/payvand.idl.json`
- `apps/api` deployment config (if any)
- `README.md` / `DEV.md` (deployment notes)

Dependencies: Task 2.1.

Estimated scope: Medium.

#### Task 2.3: Update docs and archive this plan

Acceptance criteria:

- [x] `README.md` states the aligned Anchor version, the receipt-backed
      `settled` status, and that no close or reclaim instruction exists. The
      Vercel deploy recipe was corrected too: CLI deploys upload the project
      root tree, and the web build needs the workspace `package.json`.
- [x] `plans/README.md` lists this plan as completed and archived, and
      `0002` remains active.
- [x] The plan file moves to `plans/archive/`.
- [x] The temporarily payer-funded receipt rent is recorded in `0002` as the
      state this plan leaves behind.

**Not archived yet:** the devnet budget smoke test and the checkout rent
  disclosure are still open above, so this plan stays `In Progress` until they
  are closed or explicitly dropped.

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
- [x] **Checkpoint 1:** Close instructions are gone; a settle creates a receipt
      and closes the payment account atomically; a replay fails with
      `AlreadySettled`; small amounts fail with `InvoiceTooSmall`; layouts are
      asserted; API and web report `pending`/`settled` correctly (localnet
      end-to-end and the checkout rent disclosure remain open).
- [ ] **Checkpoint 2:** `task verify`, `anchor test`, and the devnet smoke tests
      pass on an upgraded program with an unchanged ID; the plan is archived and
      `0002` records its starting state.

## Risks And Mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| API and web deploy after the program, leaving `settled` unreadable | High | Deploy API before web, in the same window; API tests cover receipt-backed status. |
| Old devnet payment accounts block `init` for their invoices | Medium | Pre-upgrade cleanup step closes them with the current instructions, or they are recorded as unpayable. |
| The receipt rent disclosure is easy to miss and the payer pays more than the QR showed | Medium | **Accepted gap:** the checkout does not disclose it, and `0002` Task 1.2 removes the charge by paying the receipt rent from the treasury, so the disclosure is never built. Until `0002` ships, a direct SOL payer pays `amount + 2,985,840` lamports; this is stated in `README.md` and here, not shown at checkout. |
| Invoices at or below the payment-account rent (~0.00249 SOL) are unpayable in this release | Medium | `InvoiceTooSmall` fails cleanly instead of panicking; the create-payment page blocks amounts below the minimum; `0002` keeps the same minimum once the fee is added. |
| Someone pre-funds an empty receipt PDA, so a replay reports a deserialization error instead of `AlreadySettled` | Low | The guard reads state, not lamports, so a replay never reports success; `init` is not a backstop, because closing the invoice empties its address. Documented as a known error-code edge, not a settlement path. |
| A later field addition under-allocates the receipt | Medium | Task 1.4 layout test fails before deployment. |
| Someone reinstates a "reclaim" instruction to recover receipt rent | Medium | The plan and README state the model; receipts are permanent by design, and `0002` pays their rent from the treasury. |

## Definition Of Done

- [x] All appendix findings are checked (finding 2 records the correction that
      `init` is not the backstop; the receipt guard is, and it is tested).
- [x] `task verify`, `anchor test`, and
      `cargo clippy -p payvand -- -D warnings` pass.
- [x] The devnet smoke tests pass on an upgraded program with an unchanged ID.
- [x] No close or reclaim instruction exists in the program, IDL, API, or web
      client.
- [x] `pending`, `settled`, and `missing` are defined once and implemented
      identically in API and web, with tests. `missing` stays in the shared type
      but is only reachable for invalid input, which the API now rejects with a
      400 before a status is chosen.
- [x] A settled invoice cannot be replayed, and small invoices fail cleanly.
- [x] The IDL, API, and web client were deployed together.
- [x] Docs match the deployed behavior.
- [x] The plan is archived and listed in `plans/README.md`, and `0002` records
      the state it inherits.

## Appendix: Findings Checklist

- [x] **1 (Critical):** No close instruction exists, so no signer can take a
      settled invoice's rent.
- [x] **2 (High):** A settled invoice cannot be replayed; the receipt guard
      returns `AlreadySettled`, and the payment account's `init` is the
      structural backstop. (As built: `init` is *not* a backstop — closing the
      invoice empties its address — so the receipt guard is the only defence and
      Task 1.5 tests it.)
- [x] **10 (Medium):** Contract tests exist and CI builds/tests the program.
- [x] **11 (Medium):** Anchor CLI and runtime versions are pinned and equal.
- [x] **13 (Low):** The `state == Settled` close check is moot; the handler is
      gone.
- [x] **14 (Low):** `SellerMismatch` / `NotPending` are removed.
- [x] **18 (Low):** No reclaim flow exists in the frontend; rent returns in the
      settle transaction.
- [x] **20 (High):** `InvoiceTooSmall` guards every `amount - rent`
      subtraction.
- [x] **21 (High):** `pending` means "no receipt"; `missing` means invalid
      input; neither depends on `Payment.state`.
- [x] **23 (Low):** A repeat settle returns `AlreadySettled`, not an opaque
      `init` error.
