# Plan: Contract Security Hardening

## Status

Draft

## Linked Docs

- `plans/README.md` — plan conventions and lifecycle.
- `README.md` — project overview and the current Anchor 1.1.2 claim.
- `apps/contracts/programs/payvand/src/lib.rs` — entrypoints and documented guarantees.
- `apps/contracts/programs/payvand/src/state.rs` — account layouts.
- `apps/contracts/programs/payvand/src/instructions/sol.rs` — SOL invoices and budgets.
- `apps/contracts/programs/payvand/src/instructions/token.rs` — token invoices and budgets.
- `apps/contracts/programs/payvand/src/constants.rs` — seeds, spaces, floats.
- `apps/api/src/payments.rs` — on-chain account decoders and `PaymentStatus`.
- `apps/web/app/composables/useBudget.ts` — budget/payment builders and UI checks.
- `apps/web/app/composables/useSessionKey.ts` — session-key storage and forgetting.
- `apps/web/app/pages/budget.vue` — open/close/rotate flows.
- `apps/web/app/pages/sell.vue` — create-payment page and fee/rent note.
- `apps/web/app/pages/shops/[id].vue` — shop page and fee/rent note.
- `apps/web/app/pages/pay.vue` — buyer checkout and exact-price display.
- `apps/web/app/assets/payvand.idl.json` — client IDL.

The findings referenced throughout are listed in the appendix and come from the
contract review of 2026-11-09.

## Objective

Fix the security, correctness, and data-integrity issues found in the Payvand
Anchor program review, in a safe order: establish a test and CI baseline first,
then close the critical theft/replay paths, then implement a transparent
seller-pays fee/rent model with automatic invoice-account close, then correct
budget accounting, token floats, session-key lifecycle, and finally add events,
cleanup, and a verified devnet rollout.

## Scope

In scope:

- On-chain program fixes for all 18 review findings.
- A configurable fee/rent policy: the payer pays the displayed price, the
  protocol fee comes out of the seller's proceeds (and covers the permanent
  record rent for SOL invoices), and the temporary invoice-account rent is
  returned automatically in the settle transaction.
- Automatic close of the payment account during settlement; the permanent
  receipt remains as the invoice record.
- IDL, API decoder, and web-client changes required by the program changes.
- Fee/rent estimates on the create-payment, shop, and buyer-checkout pages.
- Token-denominated fees so a seller can pay the fee without holding SOL.
- Anchor contract tests and a CI job that builds/tests the program.
- Devnet rollout notes and a backward-compatibility assessment.

Out of scope:

- New product features (discounts, new currencies, a full indexer).
- Mainnet deployment.
- Payer-side gasless/relayer support: the payer still needs SOL for the
  network fee and the temporary token-invoice rent unless a sponsor is added.
- A complete rewrite of the payment history model; the plan keeps the current
  account model and adds a permanent receipt account instead.

## Dependencies

Product decisions:

- Should a same-month top-up reset the monthly counter? **Recommendation: no.**
  Only a calendar-month change resets `spent`.
- Who bears the fee and rent? **Recommendation: the seller bears the protocol
  fee, deducted from the seller's proceeds. For SOL invoices the permanent
  receipt rent also comes out of the fee; for token invoices the fee is
  token-denominated and the protocol SOL treasury funds the receipt rent. The
  payer pays exactly the displayed price. For SOL invoices the temporary
  payment-account rent is funded from the price and returned to the seller
  automatically when the payment account is closed in the same transaction. For
  token invoices the payer advances the SOL rent; closing the invoice account to
  the seller returns `R_p` to the seller, and the checkout includes it in the
  payer's SOL estimate.**
- Should the payment account be closed automatically? **Recommendation: yes.**
  Close it in the settle transaction so the temporary rent returns without a
  second signature or a separate claim.
- Should a settled payment keep a permanent on-chain record? **Recommendation:
  yes, via a permanent receipt PDA created on settle.**
- Should Token-2022 be supported now? **Recommendation: document unsupported;
  treat it as a separate plan.**

Technical decisions:

- Replay prevention: permanent receipt PDA vs. in-place `Closed` tombstone.
  **Recommendation: permanent receipt PDA.** A receipt account is created on
  the first settle with `init` (not `init_if_needed`), so a second settle
  fails because the receipt already exists. The `Payment` / `TokenPayment`
  account is created, settled, and closed in the same transaction; the
  temporary rent returns automatically. The receipt remains as the permanent
  record. This avoids changing the existing `Payment` / `TokenPayment` layout and
  removes the need for any seller reclaim transaction.
- Fee/rent policy: hardcoded payer-paid vs. configurable seller-paid.
  **Recommendation: configurable seller-paid.** Add a `Config` PDA with
  `fee_bps`, `fixed_fee_lamports`, `fee_recipient`, `fee_mode`, and
  `session_fee_float`. The default is `SellerPays`: for SOL invoices the buyer
  pays `amount`, the seller receives `amount - fee`, the fee recipient receives
  `fee - receipt_rent`, the temporary rent is returned to the seller, and the
  permanent receipt rent is covered by the fee. A `BuyerPays` mode remains
  available for future products.
- Token fees: SOL-denominated vs. token-denominated. **Recommendation:
  token-denominated.** The fee is a percentage of the token amount and is split
  between the seller's ATA and the fee recipient's ATA, so the seller does not
  need SOL. The SOL network fee and the temporary token-account rent remain
  with the payer/session key (or a future sponsor).
- Anchor version: align on the version already resolved in `Cargo.lock`
  (`1.2.1`), or pin everything to `=1.1.2`. **Recommendation: align on
  `1.2.1`** because the lockfile already uses it; update `Anchor.toml`,
  `Cargo.toml`, and `README.md`.
- Budget accounting: the SOL pot pays exactly the invoice amount, the session
  key is reimbursed the rents it advanced, and `spent` stays exactly equal to
  the price. For token budgets the vault pays only tokens; the session key
  covers the SOL rents from its float. The seller's share is reduced by the
  same fee split.

Design invariants:

- **The seller never signs to create an invoice or QR code.** The invoice PDA is
  derived locally from the seller's public key.
- **The payer signs settlement and pays the displayed price.** The network fee
  is paid by the transaction fee payer (the payer or the buyer's session key).
- **The seller does not sign to settle or reclaim rent.** The payment account
  is closed in the same settle transaction, and its rent goes back to the
  seller automatically.
- **The seller pays the protocol fee**, deducted from the seller's proceeds. The
  fee covers the permanent receipt rent for SOL invoices; for token invoices the
  fee is token-denominated and the protocol funds the receipt rent from its SOL
  treasury (or the payer, if disclosed). The seller needs no SOL for token
  invoices.
- **There is no legacy reclaim path.** The app has no users and no pre-upgrade
  invoices, so `close_payment` / `close_token_payment` are removed rather than
  kept as dead code.

Open questions:

- Who holds the program upgrade authority, and when is the devnet upgrade
  window?
- How large should `Config::session_fee_float` be once it covers the payer's
  network fee and the temporary token-invoice SOL rents (e.g., 0.05 SOL
  instead of 0.01 SOL)?
- Fee scope: one global `Config`, or per-shop/per-seller fee overrides?
- Should the fee be percentage-only, fixed-only, or both? The plan supports
  both for SOL and percentage-only for tokens.
- For token invoices, who funds the permanent receipt rent in SOL when the
  protocol treasury is empty: the platform, the payer, or a seller SOL deposit?
- Do we want payer-side gasless/relayer support so a payer with no SOL can pay
  a token invoice?

See the full list under "Open Questions" near the end of this plan.

## Findings To Phase Mapping

| Finding | Severity | Phase |
| --- | --- | --- |
| 1. Unused close handlers do not authorize the seller | Critical | 1 |
| 2. Settled invoices can be replayed after close | High | 1 |
| 3. SOL budget `spent` excludes invoice rent | High | 2 |
| 4. Refills reset the monthly counter | High | 2 |
| 5. Token session-key float is consumed by ATA rent | High | 3 |
| 6. Session-key fee float is never returned | High | 4 |
| 7. `close_token_budget` accepts the wrong mint | Medium | 3 |
| 8. Hidden rent/fee costs; no fee transparency | Medium | 2 and 4 |
| 9. No events; the invoice record disappears on close | Medium | 1 and 5 |
| 10. No contract tests; CI does not build the program | Medium | 0 |
| 11. Anchor CLI/runtime version mismatch | Medium | 0 |
| 12. Vault is not constrained to the canonical ATA | Medium | 3 |
| 13. Removed close handlers did not check `state == Settled` | Low | 1 |
| 14. `SellerMismatch` / `NotPending` become unused after removal | Low | 1 |
| 15. SPL Token only; no Token-2022 | Low | 5 |
| 16. Manual account spaces | Low | 5 |
| 17. Old session-key float stranded on rotation | Low | 4 |
| 18. Frontend never reclaimed invoice rent | Low | 1 and 4 |

## Phases

### Phase 0: Baseline, test harness, and version alignment

Goal: make the program buildable and testable in CI before changing behavior,
and remove the version ambiguity that would make test results hard to trust.

#### Task 0.1: Add the Anchor test harness

Description: Create `apps/contracts/tests/payvand.ts` (or a Rust
`solana-program-test` suite if the team prefers) and wire it to `anchor test`.
The first tests should reproduce the critical bugs in red before the fixes:
close authorization, close/replay, double settle, and budget cap.

Acceptance criteria:

- [ ] `anchor test` runs a local validator and executes at least one happy-path
      test for `settle_payment` and one for `pay_from_budget`.
- [ ] Tests exist for the close-authorization and replay findings (they may be
      marked `todo`/`failing` until Phase 1, but the harness must run).
- [ ] Test helpers derive the payment and budget PDAs from the same seeds as
      the program.
- [ ] `anchor test` is documented in `DEV.md` or `plans/README.md`.
- [ ] A new `task anchor:test` runs `anchor test`, and `task verify` includes
      it alongside the existing `cargo test` contract suite.

Verification:

- [ ] `cd apps/contracts && anchor test` passes (or fails only on the
      intentionally red tests).
- [ ] `cargo check -p payvand` still passes.
- [ ] `task verify` runs both the `cargo test` and `anchor test` contract
      suites.

Files likely touched:

- `apps/contracts/tests/payvand.ts` (new)
- `apps/contracts/Anchor.toml`
- `apps/contracts/package.json` (if the test runner needs dependencies)
- `Taskfile.yml`
- `DEV.md`

Dependencies: None.

Estimated scope: Medium.

#### Task 0.2: Add a CI contract job

Description: Add a job to `.github/workflows/ci.yml` that installs the Solana
and Anchor toolchains, runs `anchor build`, and runs `anchor test` for
`apps/contracts`. Cache the toolchains if practical.

Acceptance criteria:

- [ ] CI builds the program on every PR that touches `apps/contracts/**`.
- [ ] CI runs the Anchor tests (or at least `cargo test`) for the program.
- [ ] The job fails on a compile error or a failing contract test.
- [ ] The existing web/API jobs remain unchanged.

Verification:

- [ ] Open a draft PR that touches a contract file and confirm the new job
      runs.
- [ ] `act` or a local equivalent is not required; the workflow syntax is
      validated by GitHub.

Files likely touched:

- `.github/workflows/ci.yml`
- `apps/contracts/Anchor.toml` (if the test command changes)

Dependencies: Task 0.1.

Estimated scope: Small.

#### Task 0.3: Align the Anchor CLI and runtime versions

Description: Pick one Anchor version and make `Anchor.toml`, the program
`Cargo.toml`, `Cargo.lock`, `README.md`, and the local toolchain agree. The
current state is CLI `1.1.2` vs. `anchor-lang`/`anchor-spl` `1.2.1`.

Acceptance criteria:

- [ ] `anchor --version` matches `Anchor.toml`'s `anchor_version`.
- [ ] `Cargo.toml` uses the same version (or a precise `=x.y.z` pin).
- [ ] `Cargo.lock` resolves that version.
- [ ] `README.md` no longer claims a different version.
- [ ] `anchor build` and `anchor test` pass with the chosen version.

Verification:

- [ ] `anchor --version`
- [ ] `cargo tree -p payvand | rg 'anchor-(lang|spl)'`
- [ ] `cd apps/contracts && anchor build && anchor test`

Files likely touched:

- `apps/contracts/Anchor.toml`
- `apps/contracts/programs/payvand/Cargo.toml`
- `apps/contracts/Cargo.lock`
- `README.md`

Dependencies: Task 0.1 (so the version change is tested).

Estimated scope: Small.

---

### Phase 1: Remove the unsafe close path and prevent replay

Goal: eliminate the critical theft path, make settlement single-use, and keep a
permanent payment record after the temporary account closes.

#### Task 1.1: Remove the unused close instructions

Description: Delete `close_payment` and `close_token_payment` entirely, along
with their `ClosePayment` / `CloseTokenPayment` account contexts. The app has no
users and no pre-upgrade invoices, so the only thing these instructions provide
today is an unauthenticated way for any signer to close a settled invoice and
redirect its rent. Removal is safer than adding `has_one = seller`, because it
deletes the attack surface instead of maintaining a dead code path.

Also remove the now-unused `SellerMismatch` / `NotPending` error variants if
nothing else references them, and delete the close builders/decoders from the
IDL, API, and web client.

Acceptance criteria:

- [ ] `close_payment` and `close_token_payment` no longer exist in `lib.rs`,
      `sol.rs`, or `token.rs`.
- [ ] The `ClosePayment` / `CloseTokenPayment` contexts and their account
      constraints are deleted.
- [ ] No client, API, or IDL code references either close instruction.
- [ ] `SellerMismatch` / `NotPending` are removed if unused, or kept only if a
      remaining instruction needs them.
- [ ] The program builds and the existing tests pass after removal.

Verification:

- [ ] `cargo check -p payvand` passes.
- [ ] `grep -R "close_payment\\|close_token_payment" apps/contracts apps/api apps/web`
      returns no matches.
- [ ] Anchor test: the IDL contains no close instruction.

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
rent and proving an invoice was settled. The payment account becomes a
short-lived account that is created, settled, and closed in the same
transaction; the permanent receipt keeps the record.

New account types in `state.rs`:

- `PaymentReceipt { seller, reference, amount, memo, payer, created_at,
  settled_at, closed_at, bump }`.
- `TokenPaymentReceipt { seller, reference, mint, amount, memo, payer,
  created_at, settled_at, closed_at, bump }`.

The receipt is the permanent invoice record. It keeps the memo and
`created_at` as well as the payment facts, so closing the larger payment
account does not erase the invoice from the chain.

New seeds in `constants.rs`:

- `PAYMENT_RECEIPT_SEED = b"payment-receipt"`, PDA
  `[PAYMENT_RECEIPT_SEED, seller, reference, amount]`.
- `TOKEN_PAYMENT_RECEIPT_SEED = b"token-payment-receipt"`, PDA
  `[TOKEN_PAYMENT_RECEIPT_SEED, seller, reference, mint, amount]`.
- `PAYMENT_RECEIPT_SPACE` / `TOKEN_PAYMENT_RECEIPT_SPACE`.

Settlement changes:

1. `settle_payment` / `pay_from_budget` create the SOL payment account and the
   SOL receipt with `init` (not `init_if_needed`) in the same transaction that
   settles the invoice.
2. `settle_token_payment` / `pay_from_token_budget` do the same for the token
   payment account and token receipt.
3. Add `close = seller` to the payment account in each settle context. Anchor
   closes it at the end of the instruction, so the temporary rent goes back to
   the seller automatically.
4. The receipt is funded by the payer/session key. Its rent is a permanent cost
   and is covered by the protocol fee for SOL invoices, or by the protocol SOL
   treasury (funded from fees) for token invoices, in Phase 2.
5. The receipt exists after the payment account is gone, so a re-created
   invoice with the same `(seller, reference, amount)` fails on `init` with
   "account already in use" and is therefore not replayable.
6. Set `receipt.closed_at = receipt.settled_at` in the settle transaction,
   because the payment account is closed immediately. Document that `settled`
   is the terminal API status and `closed_at` is the auto-close timestamp.
7. Because the payment account is always fresh under `init`, remove the
   `init_if_needed` first-touch/replay branch from the handlers; the receipt
   `init` is the replay guard.

Acceptance criteria:

- [ ] `PaymentReceipt` and `TokenPaymentReceipt` exist with the fields above.
- [ ] `PAYMENT_RECEIPT_SEED` / `TOKEN_PAYMENT_RECEIPT_SEED` are used in both
      the program and the client.
- [ ] A first settle creates exactly one receipt per invoice.
- [ ] The payment account is closed by the end of the settle transaction.
- [ ] The seller receives the payment account's rent in the same settle
      transaction, with no separate seller transaction.
- [ ] A second settle against the same invoice fails because the receipt
      `init` collides, with a clear error.
- [ ] The receipt remains readable after settle and records the full invoice.
- [ ] No close instruction or reclaim instruction exists anywhere.

Verification:

- [ ] Anchor test: direct SOL settle → payment account gone, receipt exists,
      seller balance increased by the payment-account rent.
- [ ] Anchor test: budget SOL settle → same, and the pot paid exactly the
      price.
- [ ] Anchor test: settle, then attempt to settle the same invoice again; the
      second attempt fails.
- [ ] Anchor test: read the receipt and assert `settled_at > 0` and the memo is
      preserved.
- [ ] `anchor test` and `cargo check -p payvand` pass.

Files likely touched:

- `apps/contracts/programs/payvand/src/state.rs`
- `apps/contracts/programs/payvand/src/constants.rs`
- `apps/contracts/programs/payvand/src/instructions/sol.rs`
- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/contracts/tests/payvand.ts`
- `apps/web/app/composables/useBudget.ts` (settle builders)
- `apps/web/app/composables/usePaymentCheckout.ts` (direct settle builder)

Dependencies: Task 1.1.

Estimated scope: Medium.

#### Task 1.3: Update the IDL, API, and web status handling

Description: The payment account is now created, settled, and closed inside the
settle transaction, and the receipt is the permanent record. Define the status
semantics once and use them in the API, IDL, and UI.

Status semantics:

- `pending` — a payment account exists with `state == Pending` and no receipt.
- `settled` — a receipt exists. This is the terminal state, because the payment
  account closes atomically in the same transaction. `closed_at` equals
  `settled_at` on these records.
- `missing` — neither a payment account nor a receipt is readable.

The API returns `settled_at_unix` and `closed_at_unix`, plus receipt-derived
fields, so the seller's history can show the record after the payment account is
gone. The UI label for `settled` reads "Paid" and adds "rent returned" when
`closed_at > 0`. The buyer checkout refuses `settled` and `missing`.

Acceptance criteria:

- [ ] The IDL includes `PaymentReceipt` and `TokenPaymentReceipt`, and the
      settle instructions list the receipt accounts.
- [ ] The API decodes receipts and reports `Settled` when a receipt exists,
      even when the payment account is gone.
- [ ] The API has no `Closed` status and no close-instruction decoder.
- [ ] The API serializes statuses as snake_case (`"pending"`, `"settled"`,
      `"missing"`).
- [ ] The web `PaymentStatus` type has exactly `'pending' | 'settled' |
      'missing'`, and the status label handles all three values.
- [ ] The buyer checkout refuses to pay a non-pending invoice.
- [ ] The seller's payment history renders a settled invoice from the receipt
      after the payment account is gone.

Verification:

- [ ] API unit tests cover pending/settled/missing for both account types.
- [ ] API test: a receipt with `closed_at == settled_at` reports `settled`.
- [ ] Web typecheck passes.
- [ ] A devnet/localnet invoice is settled and the API reports `settled` with
      the receipt visible.

Files likely touched:

- `apps/contracts/programs/payvand/src/state.rs`
- `apps/web/app/assets/payvand.idl.json` (regenerated)
- `apps/api/src/payments.rs`
- `apps/web/app/composables/usePaymentApi.ts`
- `apps/web/app/composables/usePaymentCheckout.ts`
- `apps/web/app/pages/*.vue` (status display)

Dependencies: Task 1.2.

Estimated scope: Medium.

#### Task 1.4: Add auto-close and replay regression tests

Description: Lock in the Phase 1 behavior with tests that would fail on the old
code.

Acceptance criteria:

- [ ] Test: a direct SOL settle creates the receipt, closes the payment
      account, and returns the payment-account rent to the seller in one
      transaction.
- [ ] Test: a budget SOL settle does the same, the pot pays exactly `amount`,
      and `spent == amount`.
- [ ] Test: the seller's lamport balance increases by the payment-account rent
      during settle (no separate seller transaction).
- [ ] Test: a second settle against the same invoice fails on the receipt
      `init` collision.
- [ ] Test: the removed close instructions are absent from the IDL, and no
      caller can close a settled invoice.
- [ ] Test: the receipt remains readable after the payment account is gone and
      preserves the memo, `created_at`, and `settled_at`.

Verification:

- [ ] `cd apps/contracts && anchor test`
- [ ] CI contract job is green.

Files likely touched:

- `apps/contracts/tests/payvand.ts`

Dependencies: Tasks 1.1–1.3.

Estimated scope: Medium.

---

### Phase 2: Fee/rent economics and budget accounting

Goal: make the payer pay exactly the displayed price, make the seller bear the
protocol fee and permanent record rent, return the temporary invoice rent
automatically, and make `spent` mean exactly what left the budget for invoice
amounts.

#### Task 2.1: Add the `Config` PDA and the seller-pays fee/rent split

Description: Introduce a program-wide `Config` account and rework the
settlement math so the payer pays exactly the displayed price, the seller bears
the protocol fee and the permanent receipt rent, and the temporary payment
account rent is returned to the seller in the same transaction.

`Config` account (`state.rs`):

- `admin: Pubkey` — can update the fee policy.
- `fee_bps: u16` — protocol fee in basis points, applied to the invoice amount.
- `fixed_fee_lamports: u64` — optional flat SOL fee added to `fee_bps` for SOL
  invoices.
- `fee_recipient: Pubkey` — receives the fee.
- `fee_mode: FeeMode` — `SellerPays` (default) or `BuyerPays`.
- `session_fee_float: u64` — target float for a session key (replaces the
  hardcoded `SESSION_FEE_FLOAT`).
- `bump: u8`.

Add an `initialize_config` / `update_config` admin instruction. The config PDA
is a singleton (`[CONFIG_SEED]`), created once per deployment. The client caches
it and refreshes on a TTL.

Fee rules:

- SOL: `fee = min(amount * fee_bps / 10_000 + fixed_fee_lamports, amount - 1)`
  so the seller always receives at least one lamport.
- Token: `fee = min(amount * fee_bps / 10_000, amount - 1)`. The flat SOL fee
  does not apply to token invoices; the token fee is denominated in the token.
- Require `fee >= receipt_rent` for SOL invoices. If the configured fee is
  lower than the permanent receipt rent, fail with `FeeTooLow` rather than
  silently under-funding the receipt. The admin must set a fee that covers the
  receipt rent.
- `SellerPays` is the default. `BuyerPays` is kept as a config branch for
  future products; it is not used by the current UI.

Direct SOL math (payer wallet, `SellerPays`):

- Payer signs and pays `amount` for the invoice, plus the network fee.
- `init` advances `R_p` (payment rent) and `R_r` (receipt rent) from the payer.
- Program transfers `amount - R_p - fee` to the seller and `fee - R_r` to the
  fee recipient.
- `close = seller` returns `R_p` to the seller at the end of the instruction.
- Net seller: `amount - fee`. Net payer: `amount` plus the network fee. The
  receipt rent `R_r` is inside the fee.

Budget SOL math (`SellerPays`):

- The pot pays exactly `amount`. The session key advances `R_p` and `R_r` via
  `init`, then the pot reimburses the session key `R_p + R_r`, pays the seller
  `amount - R_p - fee`, and pays the fee recipient `fee - R_r`.
- `close = seller` returns `R_p` to the seller.
- Net seller: `amount - fee`. Session key net SOL: zero. `spent += amount`.
- The pot check becomes `pot >= amount` (not `amount + invoice_rent`), because
  the rent is netted out of the price.

Token math (`SellerPays`):

- The payer transfers `amount - fee_t` tokens to the seller's ATA and `fee_t`
  tokens to the fee recipient's ATA (created on demand).
- The payer advances `R_p`, `R_r`, the seller-ATA rent, and the fee-recipient
  ATA rent in SOL. `close = seller` returns `R_p` to the seller.
- The seller needs no SOL: the fee is deducted in the token, and the seller
  receives the payment-account rent back.
- Token budgets: the session key advances the SOL rents from its float; the
  vault pays only tokens. Remove `TOKEN_BUDGET_FEE_FLOAT`; the token budget
  account no longer holds a SOL float.

Token fee and receipt-rent caveat (why the seller needs no SOL, but the protocol
needs a SOL treasury):

- The fee is deducted in the token, so the seller needs no SOL to receive its
  proceeds. A seller with zero SOL can be paid.
- The permanent receipt is a Solana account, so its rent `R_r` is denominated in
  SOL. A token transfer cannot pay it.
- Token fees do not generate SOL, so the protocol treasury must be funded from
  SOL-invoice fees, a grant, or token-to-SOL conversion. The payer advances `R_r`
  when the receipt is created with `init`; the protocol should reimburse `R_r`
  from that treasury. Until it is funded, the payer bears `R_r` and the checkout
  discloses it. This is the one place where the "seller bears the permanent
  record rent" rule is implemented at the protocol level rather than by
  deducting from the seller's token proceeds.
- The seller's token proceeds are always `amount - fee_t`; the receipt rent is
  not an extra seller deduction and does not depend on the seller holding SOL.
- Because the payer funds the token invoice account, closing it to the seller
  also sends the seller `R_p` SOL even though the seller never held SOL. This is
  a deliberate onboarding/rebate choice, and the buyer checkout must include
  `R_p` in the payer's SOL estimate. If product wants the payer to keep that
  rent instead, use `close = payer` (or the session key for budget payments) for
  token invoices; the seller still needs no SOL.
- The payer still needs SOL for the network fee and the temporary token-account
  rents. Removing those costs requires a relayer/sponsor, which is out of scope
  and tracked as a follow-up.

UI transparency:

- Add a shared `estimateInvoiceCosts(amount, mint?)` helper that reads `Config`
  and `Rent::minimum_balance` and returns the exact fee, seller proceeds,
  payer total, temporary rent, permanent receipt rent, and network-fee
  estimate.
- Show a note on the create-payment page (`sell.vue`) and the shop page
  (`shops/[id].vue`) with an example: "You receive X after the Y% fee. The
  payer pays exactly P for the invoice (plus the Solana network fee). The fee
  covers the permanent receipt for SOL invoices; for token invoices the protocol
  funds it from fee revenue. The temporary invoice rent comes back to you
  automatically."
- Show the same breakdown on the buyer checkout (`pay.vue`) before signing,
  including the SOL network/rent estimate for token invoices.
- Rounding: compute fees in base units with integer math and document the
  rounding direction (floor, favouring the seller).

Acceptance criteria:

- [ ] `Config` exists with the fields above and an admin-only update path.
- [ ] `fee_bps` and `fixed_fee_lamports` produce the documented SOL fee.
- [ ] `fee_bps` produces the documented token fee.
- [ ] `SellerPays` gives the seller `amount - fee` and the payer pays exactly
      `amount` for SOL invoices.
- [ ] For SOL invoices, the fee recipient receives `fee - R_r` and the receipt
      account holds `R_r`; for token invoices, the fee recipient receives
      `fee_t` tokens and the payer advances `R_r`.
- [ ] The payment account is closed to the seller in the same transaction and
      the seller receives `R_p`.
- [ ] `spent` increases by exactly `amount` for budget payments.
- [ ] Token sellers need no SOL and receive `amount - fee_t` tokens.
- [ ] The token checkout estimate includes the payer's `R_p`, `R_r`, ATA rents,
      and network fee.
- [ ] `estimateInvoiceCosts` is used by the create-payment, shop, and checkout
      pages.
- [ ] For SOL invoices, a fee lower than the receipt rent fails with
      `FeeTooLow`.

Verification:

- [ ] Anchor test: direct SOL settle with a known fee; assert payer balance
      delta is exactly `amount + network fee`, seller delta is `amount - fee`,
      fee recipient delta is `fee - R_r`, and the receipt holds `R_r`.
- [ ] Anchor test: budget SOL settle; assert pot delta is `amount`, session key
      delta is zero, seller delta is `amount - fee`, and `spent == amount`.
- [ ] Anchor test: token settle; assert seller receives `amount - fee_t`, fee
      recipient receives `fee_t`, and the seller needs no SOL.
- [ ] Anchor test: `BuyerPays` still gives the seller `amount` and charges the
      payer `amount + fee`.
- [ ] Web typecheck and a localnet/manual check of the three pages.

Files likely touched:

- `apps/contracts/programs/payvand/src/state.rs`
- `apps/contracts/programs/payvand/src/constants.rs`
- `apps/contracts/programs/payvand/src/instructions/sol.rs`
- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/contracts/programs/payvand/src/instructions/config.rs` (new)
- `apps/contracts/programs/payvand/src/lib.rs`
- `apps/contracts/tests/payvand.ts`
- `apps/web/app/composables/useBudget.ts`
- `apps/web/app/composables/usePaymentCheckout.ts`
- `apps/web/app/pages/sell.vue`
- `apps/web/app/pages/shops/[id].vue`
- `apps/web/app/pages/pay.vue`

Dependencies: Phase 1.

Estimated scope: Large.

#### Task 2.2: Make the budget pay exactly the invoice amount

Description: With the fee/rent split from Task 2.1, the SOL pot pays exactly
`amount`: it reimburses the session key for `R_p + R_r`, pays the seller
`amount - R_p - fee`, and pays the fee recipient `fee - R_r`. Replace the old
`pot >= amount + invoice_rent` check with `pot >= amount`, and replace the old
single-rent refund with the documented reimbursement.

For tokens, the session key advances the SOL rents and is not reimbursed in
SOL, because the vault holds only tokens. Size `Config::session_fee_float` to
cover several token payments (invoice rent, receipt rent, seller-ATA rent,
fee-recipient ATA rent, network fees) and remove `TOKEN_BUDGET_FEE_FLOAT` from
`open_token_budget`.

Acceptance criteria:

- [ ] `pay_from_budget` debits the pot by exactly `amount`.
- [ ] `pay_from_budget` reimburses exactly `R_p + R_r` to the session key, no
      more.
- [ ] `budget.spent` increases by exactly `amount`.
- [ ] The session key's float is the only source of token-invoice rents.
- [ ] The UI's `availablePot` and `remainingThisMonth` agree on what a payment
      costs.
- [ ] `open_token_budget` no longer charges `TOKEN_BUDGET_FEE_FLOAT`, and the
      client cost estimate is updated.

Verification:

- [ ] Anchor test: pay from budget, assert `budget.spent == amount`, the pot
      decreased by exactly `amount`, and the seller received `amount - fee`.
- [ ] Anchor test: the session key's SOL balance is unchanged by a SOL budget
      payment.
- [ ] Anchor test: the session key's balance decreases only by the documented
      token rents and network fee.
- [ ] UI typecheck and the budget page smoke test pass.

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/sol.rs`
- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/contracts/programs/payvand/src/constants.rs`
- `apps/web/app/composables/useBudget.ts`
- `apps/web/app/pages/budget.vue`
- `apps/contracts/tests/payvand.ts`

Dependencies: Task 2.1.

Estimated scope: Medium.

#### Task 2.3: Preserve `spent` on same-month top-ups

Description: Change `open_budget` and `open_token_budget` so `spent` resets
only when `budget.month_index != current_month_index()`. A same-month top-up
should raise the pot/vault and the `monthly_limit` while preserving the
already-spent amount.

Acceptance criteria:

- [ ] Same-month `open_budget` preserves `spent` and updates `monthly_limit`.
- [ ] Same-month `open_token_budget` preserves `spent` and updates
      `monthly_limit`.
- [ ] A calendar-month change resets `spent` to zero and updates
      `month_index`.
- [ ] The `budget.vue` copy no longer says a refill restarts the monthly
      counter.
- [ ] The rotate flow preserves `spent` unless the month changed.

Verification:

- [ ] Anchor test: spend, top up in the same month, assert `spent` is preserved.
- [ ] Anchor test: simulate a month change (or use a test clock) and assert
      `spent` resets.
- [ ] Web copy review.

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/sol.rs`
- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/web/app/pages/budget.vue`
- `apps/web/app/composables/useBudget.ts`
- `apps/contracts/tests/payvand.ts`

Dependencies: Tasks 2.1–2.2.

Estimated scope: Small.

#### Task 2.4: Update the UI remaining/obstacle logic

Description: After the fee/rent split and budget accounting changes, make the
UI's "remaining" and preflight checks match the program:

- `remainingThisMonth` is `monthly_limit - spent` (accurate for invoice
  amounts).
- `silentPaymentObstacle` checks `availablePot >= amount` and checks the
  session key's SOL balance for the temporary rent + permanent receipt rent +
  network fee (the fee is deducted from the seller, not the payer).
- `tokenBudgetObstacle` checks the session key's SOL balance for the invoice
  rent + seller-ATA rent + fee-recipient ATA rent + receipt rent + network fee.
- `openBudgetCost` / `openTokenBudgetCost` reflect `Config::session_fee_float`
  and the new rent model.
- The fee/rent estimate helper from Task 2.1 feeds the create-payment, shop,
  and checkout notes, so the displayed estimate and the preflight check share
  one source of truth.

Acceptance criteria:

- [ ] The UI never says a payment can ride the budget when the session key
      cannot pay the rent/fee.
- [ ] The "remaining" number matches what the user can actually spend.
- [ ] The open/refill cost breakdown matches the program's transfers.
- [ ] The error messages name the missing resource (pot vs. session-key SOL).
- [ ] The create-payment, shop, and checkout pages show the fee/rent estimate
      from the shared helper.

Verification:

- [ ] Web typecheck and lint pass.
- [ ] Manual localnet/devnet: exhaust the session-key float and confirm the UI
      warns before submission instead of failing on-chain.
- [ ] Manual: change `Config` and confirm the displayed estimates update after
      the cache TTL.

Files likely touched:

- `apps/web/app/composables/useBudget.ts`
- `apps/web/app/composables/usePaymentCheckout.ts`
- `apps/web/app/pages/budget.vue`
- `apps/web/app/pages/sell.vue`
- `apps/web/app/pages/shops/[id].vue`
- `apps/web/app/pages/pay.vue`

Dependencies: Tasks 2.1–2.3.

Estimated scope: Medium.

#### Task 2.5: Add budget and fee-accounting tests

Description: Cover the monthly cap, refill behavior, and the fee/rent split.

Acceptance criteria:

- [ ] Test: a payment exactly at the limit succeeds; one lamport over fails
      with `BudgetExhausted`.
- [ ] Test: a same-month refill does not increase the remaining spendable
      amount beyond the new limit minus the preserved `spent`.
- [ ] Test: a month change resets the counter.
- [ ] Test: the pot/vault balances match the expected amounts after a payment.
- [ ] Test: a compromised session key cannot drain more than the pot/vault
      balance (the existing balance check is the backstop).
- [ ] Test: for SOL invoices the seller receives `amount - fee` and the fee
      recipient receives `fee - receipt_rent`; for token invoices the seller
      receives `amount - fee_t` tokens and the fee recipient receives `fee_t`
      tokens.
- [ ] Test: for SOL invoices a fee lower than the receipt rent fails with
      `FeeTooLow`.

Verification:

- [ ] `cd apps/contracts && anchor test`

Files likely touched:

- `apps/contracts/tests/payvand.ts`

Dependencies: Tasks 2.1–2.4.

Estimated scope: Small.

---

### Phase 3: Token budget safety

Goal: make silent token payments survive new sellers, prevent wrong-mint or
decoy-vault closes from stranding funds, and make token fees payable without
seller SOL.

#### Task 3.1: Size the session-key float for token payments

Description: For token invoices the session key advances the payment rent, the
receipt rent, the seller-ATA rent (when new), the fee-recipient ATA rent (when
new), and the network fee. The payment rent comes back to the seller via
`close = seller`; the other SOL costs are consumed. The token fee is deducted in
the token, so the seller needs no SOL, but the session key float must be large
enough to cover several token payments.

Changes:

- Remove the budget-float refund and `TOKEN_BUDGET_FEE_FLOAT`; the token budget
  account no longer holds a SOL float because the vault pays only tokens.
- Raise the float target via `Config::session_fee_float` (e.g., 0.05 SOL) to
  cover several invoice rents + seller-ATA rents + fee-recipient ATA rents +
  receipt rents + network fees.
- Make `open_budget` / `open_token_budget` top the session key up to the
  configured target, and add an explicit "top up session key" path so the buyer
  can refill the float without changing the budget limit.
- The client checks the float before a silent payment and shows the exact
  shortfall.

Acceptance criteria:

- [ ] A sequence of token payments to N new sellers does not exhaust the
      session key before the configured float is used.
- [ ] The UI warns when the session key cannot cover the next payment's rent
      (invoice + receipt + seller ATA + fee-recipient ATA) and network fee.
- [ ] The buyer can top up the session key float without changing the budget
      limit.
- [ ] The cost breakdown in `openTokenBudgetCost` matches the program.
- [ ] The token budget account no longer charges `TOKEN_BUDGET_FEE_FLOAT`.

Verification:

- [ ] Anchor test: pay to several fresh sellers and assert the session key
      still has enough lamports (or that the expected top-up is required).
- [ ] Anchor test: the seller's token balance is `amount - fee_t` and the seller
      holds no SOL before the payment.
- [ ] Manual localnet/devnet: make 5+ token payments to fresh seller ATAs.
- [ ] Web typecheck/lint.

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/contracts/programs/payvand/src/constants.rs`
- `apps/contracts/programs/payvand/src/instructions/config.rs`
- `apps/web/app/composables/useBudget.ts`
- `apps/web/app/pages/budget.vue`
- `apps/contracts/tests/payvand.ts`

Dependencies: Phase 2.

Estimated scope: Medium.

#### Task 3.2: Enforce `mint == budget.mint` in `close_token_budget`

Description: Add
`require_keys_eq!(mint.key(), budget.mint, PaymentError::MintMismatch)` before
touching the vault, or add a `has_one = mint`-style constraint. The budget PDA
seeds already use `budget.mint`; the passed `mint` must match it.

Acceptance criteria:

- [ ] Closing with the correct mint drains the real vault and closes the
      budget.
- [ ] Closing with a different mint fails with `MintMismatch`.
- [ ] The real vault cannot be stranded by a wrong-mint close.

Verification:

- [ ] Anchor test: wrong-mint close fails.
- [ ] Anchor test: correct-mint close returns the tokens and the vault rent.

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/contracts/tests/payvand.ts`

Dependencies: Phase 0.

Estimated scope: Small.

#### Task 3.3: Constrain the vault to the canonical ATA

Description: In `pay_from_token_budget` and `close_token_budget`, replace the
`token::mint` / `token::authority` vault constraints with
`associated_token::mint = mint` / `associated_token::authority = budget`, or add
an explicit canonical-ATA check. `open_token_budget` already does this.

Acceptance criteria:

- [ ] A decoy token account for the same mint and authority is rejected.
- [ ] The canonical ATA is the only vault accepted by pay/close.
- [ ] The vault's rent is returned to the buyer on close.

Verification:

- [ ] Anchor test: a decoy vault is rejected.
- [ ] Anchor test: the canonical vault is drained and closed.

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/contracts/tests/payvand.ts`

Dependencies: Task 3.2.

Estimated scope: Small.

#### Task 3.4: Add token budget regression tests

Description: Cover the token-specific edge cases.

Acceptance criteria:

- [ ] Test: wrong-mint close fails.
- [ ] Test: decoy vault is rejected.
- [ ] Test: paying to a seller with no ATA creates it and the session key pays
      the rent.
- [ ] Test: a seller with no SOL receives `amount - fee_t` tokens.
- [ ] Test: the fee recipient receives `fee_t` tokens and its ATA is created on
      demand.
- [ ] Test: the payment account is closed and the seller receives its rent in
      the same settle transaction.
- [ ] Test: the token monthly limit is enforced and preserved across a
      same-month top-up.
- [ ] Test: a settled token invoice cannot be replayed.

Verification:

- [ ] `cd apps/contracts && anchor test`

Files likely touched:

- `apps/contracts/tests/payvand.ts`

Dependencies: Tasks 3.1–3.3.

Estimated scope: Small.

---

### Phase 4: Session-key lifecycle and accurate docs

Goal: stop losing the session-key float and correct the fee/rent messaging now
that the payment account auto-closes and there is no reclaim step.

#### Task 4.1: Sweep the session key before forgetting or rotating

Description: The program cannot move lamports out of the session key because
the session key is not a PDA. The client must sign a `system_program::transfer`
from the session key to the buyer for `balance - fee` before:

- `forgetSessionSigner` in `budget.vue` (after the last budget is closed), and
- rotating to a new session key.

Add a `sweepSessionKey(buyer)` helper in `useSessionKey.ts` or `useBudget.ts`,
build the transfer, sign with the session key, send it, and confirm. If the
sweep fails, do not forget the key silently; show the balance and a retry
action.

Acceptance criteria:

- [ ] Closing the last budget sweeps the session key's lamports back to the
      buyer before the key is forgotten.
- [ ] Rotating on the current device sweeps the old key's lamports (if the
      current device holds the old key).
- [ ] A rotated-out device detects that its key is no longer the budget's
      `session_key` and offers to sweep its remaining float.
- [ ] The success message no longer claims "every lamport back" unless the
      sweep succeeded.
- [ ] The UI shows the session-key balance and a "recover" action when it is
      non-zero.

Verification:

- [ ] Unit/integration test for the sweep helper (mocked or localnet).
- [ ] Manual localnet/devnet: close all budgets and confirm the session key's
      balance is zero (minus the fee).
- [ ] Manual: rotate to a new key and confirm the old key's balance is swept.

Files likely touched:

- `apps/web/app/composables/useSessionKey.ts`
- `apps/web/app/composables/useBudget.ts`
- `apps/web/app/pages/budget.vue`
- `apps/web/app/composables/usePaymentCheckout.ts` (if it creates keys)

Dependencies: Phase 2.

Estimated scope: Medium.

#### Task 4.2: Update fee and rent messages and docs

Description: Correct the false claims in `lib.rs` and the UI:

- `lib.rs:84-85` — "the buyer gets every lamport back" is false until the
  session-key sweep lands; after the sweep, say "the buyer gets the pot and
  the session-key float back".
- `lib.rs:135-136` — "every token and lamport comes back" is false until the
  sweep lands and the wrong-mint/canonical-vault fixes land.
- `budget.vue:408` — "everything left went back to your wallet" should reflect
  the sweep result.
- Document the fee/rent model: the payer pays the displayed price, the seller
  receives the price minus the protocol fee, the fee covers the permanent
  receipt rent, and the temporary invoice rent returns to the seller
  automatically.
- Document that token fees are deducted in the token, so the seller needs no
  SOL, while the payer still needs SOL for the network fee and temporary rents
  until a relayer/sponsor exists.

Acceptance criteria:

- [ ] The doc comments match the actual behavior.
- [ ] The UI success messages are accurate.
- [ ] The README/DEV docs describe the fee/rent model and the session-key
      float recovery.
- [ ] The create-payment and shop pages carry the fee/rent note from Task 2.1.

Verification:

- [ ] Doc review.
- [ ] Manual close/rotate flow.
- [ ] Manual create-payment/shop/checkout review.

Files likely touched:

- `apps/contracts/programs/payvand/src/lib.rs`
- `apps/web/app/pages/budget.vue`
- `apps/web/app/pages/sell.vue`
- `apps/web/app/pages/shops/[id].vue`
- `apps/web/app/pages/pay.vue`
- `README.md`
- `DEV.md`

Dependencies: Task 4.1.

Estimated scope: Small.

#### Task 4.3: Add session-key lifecycle tests

Description: Cover the sweep and rotation behavior.

Acceptance criteria:

- [ ] Test: after closing the last budget, the session key's balance is swept
      (minus the fee).
- [ ] Test: after rotation, the old key's balance is swept or the old device
      is prompted.
- [ ] Test: the UI does not forget a key with a non-zero balance without a
      warning.

Verification:

- [ ] Client test suite (or a documented manual checklist if there is no
      browser test harness).

Files likely touched:

- `apps/web/app/composables/useSessionKey.ts`
- `apps/web/app/composables/useBudget.ts`
- `apps/web/app/pages/budget.vue`
- `apps/web/app/**/*.test.ts` (if a test harness exists)

Dependencies: Task 4.1.

Estimated scope: Small.

---

### Phase 5: Events, observability, and cleanup

Goal: make the program observable and clean up the remaining low-severity
issues.

#### Task 5.1: Add program events

Description: Add `#[event]` structs and `emit!` calls for:

- `PaymentSettled` (SOL) — includes `amount`, `fee`, `seller_proceeds`,
  `receipt_rent`, `rent_returned`, `payer`, `receipt`, and `timestamp`.
- `TokenPaymentSettled` — includes `amount`, `fee`, `seller_proceeds`,
  `receipt_rent`, `rent_returned`, `mint`, `payer`, `receipt`, and `timestamp`.
- `ConfigUpdated` — `fee_bps`, `fixed_fee_lamports`, `fee_recipient`,
  `fee_mode`, `session_fee_float`.
- `BudgetOpened` / `BudgetClosed`
- `TokenBudgetOpened` / `TokenBudgetClosed`

Include the payment/budget address, seller/buyer, reference, mint (if any),
amount, fee, payer/session key, and timestamp. Keep the existing `msg!` logs
for human-readable explorers.

Acceptance criteria:

- [ ] Events are defined and emitted on every state change.
- [ ] The IDL includes the events.
- [ ] The API/indexer can consume the events (or the plan documents that it
      still reads accounts).
- [ ] Settle emits the fee/rent split, so an indexer can reconstruct the
      seller's proceeds without re-deriving the config.
- [ ] No event exists for a close path, because no close path exists.

Verification:

- [ ] Anchor test asserts the event data (via logs or a program-test event
      harness).
- [ ] `anchor build` regenerates the IDL with the events.

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/sol.rs`
- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/contracts/programs/payvand/src/instructions/config.rs`
- `apps/contracts/programs/payvand/src/lib.rs` (or a new `events.rs`)
- `apps/web/app/assets/payvand.idl.json`
- `apps/contracts/tests/payvand.ts`

Dependencies: Phase 2 (the event data should include the fee/rent split).

Estimated scope: Medium.

#### Task 5.2: Decide and document Token-2022 support

Description: The program uses `anchor_spl::token` and will reject Token-2022
mints. Either:

- Document the limitation in `README.md` / `DEV.md` and keep the current
  SPL Token implementation, or
- Create a separate plan to migrate to `anchor_spl::token_interface` and
  `transfer_checked`, including how transfer fees/hooks affect the invoice
  amount.

Acceptance criteria:

- [ ] The product docs state which token programs are supported.
- [ ] If Token-2022 is deferred, there is a linked follow-up plan or issue.
- [ ] The API/UI do not offer Token-2022 mints unless supported.

Verification:

- [ ] Doc review.
- [ ] Attempting a Token-2022 mint fails with a clear error.

Files likely touched:

- `README.md`
- `DEV.md`
- `apps/contracts/programs/payvand/src/instructions/token.rs` (only if
  migrating)

Dependencies: None.

Estimated scope: Small.

#### Task 5.3: Use `InitSpace` or add layout tests

Description: Replace the hand-computed `CONFIG_SPACE`, `BUDGET_SPACE`,
`PAYMENT_SPACE`, `TOKEN_PAYMENT_SPACE`, `TOKEN_BUDGET_SPACE`,
`PAYMENT_RECEIPT_SPACE`, and `TOKEN_PAYMENT_RECEIPT_SPACE` with
`#[derive(InitSpace)]` + `space = 8 + Account::INIT_SPACE`, or keep the
constants and add a test that asserts the serialized size matches. The `String`
fields need `#[max_len(PAYMENT_MAX_MEMO)]`.

Acceptance criteria:

- [ ] Account sizes are derived or asserted by a test.
- [ ] Adding/removing a field cannot silently under-allocate.
- [ ] The on-chain layout is unchanged (or a migration plan exists if it
      changes).

Verification:

- [ ] `cargo test -p payvand` (layout test).
- [ ] `anchor test` (create/settle still work).

Files likely touched:

- `apps/contracts/programs/payvand/src/state.rs`
- `apps/contracts/programs/payvand/src/constants.rs`
- `apps/contracts/tests/payvand.ts`

Dependencies: Phase 1 (the receipt account types must be in place first).

Estimated scope: Small.

#### Task 5.4: Remove dead code and unused errors

Description: After Phase 1 removed the close instructions, `SellerMismatch` and
`NotPending` should be unused and must be removed. Remove any other dead code or
stale comments (e.g., logs or docs that mention closing a settled invoice).

Acceptance criteria:

- [ ] No unused error variants.
- [ ] No stale comments that contradict the code.
- [ ] `cargo clippy -p payvand -- -D warnings` is clean (or the project's
      agreed lint level).

Verification:

- [ ] `cargo clippy -p payvand -- -D warnings`
- [ ] `cargo check -p payvand`

Files likely touched:

- `apps/contracts/programs/payvand/src/errors.rs`
- `apps/contracts/programs/payvand/src/instructions/*.rs`
- `apps/contracts/programs/payvand/src/lib.rs`

Dependencies: Phase 1.

Estimated scope: Small.

---

### Phase 6: Verification, rollout, and archival

Goal: prove the fixes end-to-end, deploy in the right order, and archive the
plan.

#### Task 6.1: Full verification

Description: Run the complete verification bar.

Acceptance criteria:

- [ ] `task verify` passes (lint, typecheck, API tests, and the contract
      suite including `anchor test`).
- [ ] `cd apps/contracts && anchor build && anchor test` passes.
- [ ] `cargo clippy -p payvand -- -D warnings` passes.
- [ ] The IDL is regenerated and copied to
      `apps/web/app/assets/payvand.idl.json`.
- [ ] The API unit tests cover the receipt-backed `settled` status and the
      `missing` status.
- [ ] The fee/rent estimate helper is tested against the program's math.
- [ ] The web app typechecks and the affected pages work on localnet.

Verification:

- [ ] `task verify`
- [ ] `cd apps/contracts && anchor test`
- [ ] Manual localnet smoke test (see Task 6.2).

Files likely touched:

- `apps/web/app/assets/payvand.idl.json`
- Any files changed in earlier phases.

Dependencies: Phases 0–5.

Estimated scope: Medium.

#### Task 6.2: Devnet rollout

Description: Deploy in a safe order and smoke-test the real flows.

Rollout order:

1. Confirm the program upgrade authority and back up
   `apps/contracts/target/deploy/payvand-keypair.json`.
2. `anchor build` and `anchor deploy` to devnet.
3. Initialize the `Config` PDA with the devnet fee policy (`fee_bps`,
   `fixed_fee_lamports`, `fee_recipient`, `fee_mode = SellerPays`,
   `session_fee_float`). Confirm `fee >= receipt_rent`.
4. Copy the new IDL to `apps/web/app/assets/payvand.idl.json`.
5. Deploy the API (it must understand the receipt-backed `settled` status
   before/with the program).
6. Deploy the web app.
7. Smoke-test on devnet.

Smoke tests:

- [ ] Pay a SOL invoice from a wallet and confirm the wallet balance decreased
      by exactly the displayed price plus the network fee; the seller received
      `price - fee`; the fee recipient received `fee - receipt_rent`; the
      payment account is gone; the receipt remains; and a replay fails.
- [ ] Pay a SOL invoice from a budget and confirm `spent == price`, the pot
      decreased by exactly the price, and the session key balance is unchanged.
- [ ] Confirm the deployed IDL has no close instructions, and a settled invoice
      cannot be closed or replayed.
- [ ] Open a token budget, pay a fresh seller with no SOL, confirm the seller
      received `amount - fee_t` tokens and the fee recipient received `fee_t`,
      then close the budget and verify the vault is drained.
- [ ] Pay a direct token invoice from a payer with SOL and confirm the seller
      receives the payment-account rent back automatically.
- [ ] Attempt a wrong-mint `close_token_budget` (must fail).
- [ ] Close all budgets and verify the session-key sweep.
- [ ] Verify the create-payment, shop, and checkout pages show the fee/rent
      estimate and match the on-chain amounts.
- [ ] Verify the API reports `pending`, `settled`, and `missing` correctly.

Acceptance criteria:

- [ ] The devnet program ID is unchanged (or the new ID is documented).
- [ ] The IDL in the web app matches the deployed program.
- [ ] The smoke tests pass.
- [ ] No user-facing error messages reference the old behavior.

Verification:

- [ ] `solana program show <PROGRAM_ID>`
- [ ] Manual devnet checklist above.
- [ ] API/web logs are clean.

Files likely touched:

- `apps/web/app/assets/payvand.idl.json`
- `apps/api` deployment config (if any)
- `README.md` / `DEV.md` (deployment notes)

Dependencies: Task 6.1.

Estimated scope: Medium.

#### Task 6.3: Update docs and archive the plan

Description: Update the project docs and move this plan to `plans/archive/`
once all tasks are complete.

Acceptance criteria:

- [ ] `README.md` describes the final Anchor version, the receipt-backed
      `settled` status, and the seller-pays fee/rent model.
- [ ] `plans/README.md` lists this plan as completed and archived.
- [ ] The plan file is moved to `plans/archive/`.
- [ ] Any follow-up work (e.g., Token-2022, relayer/sponsor support) is
      captured in a new plan or issue.

Verification:

- [ ] Doc review.
- [ ] `git status` shows only the intended changes.

Files likely touched:

- `README.md`
- `plans/README.md`
- `plans/archive/0001-contract-security-hardening.md`

Dependencies: Task 6.2.

Estimated scope: Small.

---

## Checkpoints

- [ ] **Checkpoint 0:** Contract test harness and CI job exist; `anchor test`
      runs; Anchor versions are aligned.
- [ ] **Checkpoint 1:** The close instructions are removed from the program and
      IDL; a settled invoice cannot be replayed; settlement auto-closes the
      payment account and leaves a receipt; the API/web report `settled` from
      the receipt; the critical regression tests pass.
- [ ] **Checkpoint 2:** The payer pays exactly the displayed price for SOL
      invoices; the seller receives `price - fee`; the fee recipient receives
      `fee - receipt_rent`; the temporary rent returns to the seller
      automatically; `spent` equals the invoice amount; same-month refills
      preserve `spent`; the create-payment, shop, and checkout pages show the
      fee/rent estimate.
- [ ] **Checkpoint 3:** Token payments survive fresh sellers; sellers need no
      SOL and receive `amount - fee_t`; wrong-mint and decoy-vault closes fail;
      the session-key float covers token rents; token tests pass.
- [ ] **Checkpoint 4:** Closing/rotating sweeps the session-key float; there is
      no reclaim flow anywhere; the docs no longer claim false guarantees and
      explain the fee/rent model.
- [ ] **Checkpoint 5:** Events are emitted with the fee/rent split and in the
      IDL; Token-2022 is documented; account sizes (including `Config` and
      receipts) are tested.
- [ ] **Checkpoint 6:** `task verify`, `anchor test`, and the devnet smoke
      tests pass; the plan is archived.

## Risks And Mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| The receipt/status change is deployed without the API/web update | High | Deploy the API and web with the program; add API tests for pending/settled/missing; use a coordinated rollout. |
| The session-key float is too small for token rents | High | Size `Config::session_fee_float` from measured rent + network costs, top it up on open, and add a UI shortfall check before silent payments. |
| Existing devnet accounts become unreadable | High | The `Payment`/`TokenPayment` layouts are unchanged and the new receipt/config accounts are additive; add a read test against an existing account before/after upgrade. If a layout change is needed, add a migration plan. |
| The configured fee is lower than the receipt rent | High | For SOL invoices, validate `fee >= receipt_rent` on `update_config` and again at settle (`FeeTooLow`); add a config test. |
| The fee config is set too high and the seller receives nothing | High | Cap the fee at `amount - 1`, validate `fee_bps <= 10_000`, and add a config test. |
| The `Config` admin key is compromised | High | Store the admin in a multisig or a program-owned PDA with a timelock; document the authority. |
| The auto-close sends the payment rent to the wrong account | High | Use `close = seller` and add a test asserting the seller's balance delta; never accept an arbitrary close destination. |
| Token invoices still require payer SOL for the network fee and temporary rents | Medium | Document the limitation on the checkout page; keep the token price exact; track relayer/sponsor support as a follow-up plan. |
| The fee-recipient ATA rent is a new payer cost | Medium | Show it in the token checkout estimate; cache the fee-recipient ATA so it is created once per mint. |
| Rounding leaves a zero-value transfer or a zero seller share | Medium | Floor the fee in the seller's favour, skip zero-value token transfers, and test small amounts. |
| The receipt rent is perceived as a new cost | Medium | Show it inside the fee on the create-payment/shop pages and in the settle event; explain that it is a permanent on-chain record. |
| Removing the close instructions breaks an old client | Medium | Update the client and API in the same release; there are no users or pre-upgrade invoices to preserve. |
| The monthly-cap change conflicts with the current UI copy | Medium | Update `budget.vue` and the docs in the same PR; add a test for same-month top-ups. |
| The token float change increases the buyer's upfront cost | Medium | Show the exact cost in `openTokenBudgetCost`; let the buyer top up the float separately. |
| The Anchor version upgrade changes the IDL/build output | Medium | Run `anchor build`/`anchor test` before and after; diff the IDL; keep the lockfile committed. |
| The devnet upgrade authority is unavailable | High | Confirm the authority and keypair before starting Phase 6; if unavailable, pause and escalate. |

## Open Questions

- What are the initial devnet `fee_bps`, `fixed_fee_lamports`, and
  `session_fee_float` values? They must satisfy `fee >= receipt_rent` at the
  smallest invoice the product allows.
- Should `Config` be one global policy or support per-shop/per-seller
  overrides? The plan assumes one global policy for now.
- Should the fee recipient be a plain wallet or a program-owned treasury PDA
  that can also fund the permanent receipt rent for token invoices?
- For token invoices, who funds the permanent receipt rent in SOL when the
  protocol treasury is empty: the platform, the payer, or a seller SOL
  deposit? The plan recommends the protocol treasury, with a disclosed payer
  fallback.
- For token invoices, should the temporary payment rent return to the seller
  (the current plan, effectively a payer-funded SOL rebate) or to the payer who
  advanced it? Returning it to the payer makes the payer's net SOL cost only the
  permanent receipt rent plus the ATA rents.
- Do we want payer-side gasless/relayer support so a payer with no SOL can pay
  a token invoice? This is out of scope but tracked as a follow-up.
- Should the seller's ATA rent be treated as a gift to the seller, or should
  the seller be required to have the ATA before an invoice can be paid? The
  recommendation is to keep the current "buyer onboards the seller" UX and
  disclose the cost.
- Is a full event indexer in scope, or are events only for observability?
- Do we want to support Token-2022 in the same release, or defer it?

## Appendix: Findings Checklist

Use this checklist to confirm every review finding is addressed.

- [ ] **1 (Critical):** `close_payment` / `close_token_payment` are removed, so
      no signer can close a settled invoice and steal its rent.
- [ ] **2 (High):** A settled invoice cannot be replayed after the payment
      account is auto-closed.
- [ ] **3 (High):** `spent` matches the amount that leaves the budget.
- [ ] **4 (High):** Same-month refills do not reset the monthly counter.
- [ ] **5 (High):** Token silent payments survive fresh seller ATAs.
- [ ] **6 (High):** The session-key float is returned on close/rotate.
- [ ] **7 (Medium):** `close_token_budget` rejects a wrong mint.
- [ ] **8 (Medium):** Fee and rent costs are disclosed; the payer pays exactly
      the displayed price; the seller bears the fee and permanent record rent.
- [ ] **9 (Medium):** Events are emitted; the receipt keeps the payment record
      after the temporary account is closed.
- [ ] **10 (Medium):** Contract tests exist and CI builds/tests the program.
- [ ] **11 (Medium):** Anchor CLI and runtime versions match.
- [ ] **12 (Medium):** The vault is the canonical ATA.
- [ ] **13 (Low):** The `state == Settled` close check is moot because the close
      handlers are removed.
- [ ] **14 (Low):** `SellerMismatch` / `NotPending` are removed as unused.
- [ ] **15 (Low):** Token-2022 support is decided and documented.
- [ ] **16 (Low):** Account spaces are derived or tested.
- [ ] **17 (Low):** Rotating does not strand the old key's float.
- [ ] **18 (Low):** The seller's temporary invoice rent is returned
      automatically, and no reclaim flow exists in the frontend.

## Definition Of Done

- [ ] All appendix findings are checked.
- [ ] `task verify`, `anchor test`, and `cargo clippy -p payvand -- -D
      warnings` pass.
- [ ] The devnet smoke tests pass.
- [ ] The payer pays exactly the displayed price for SOL invoices.
- [ ] The seller receives the displayed price minus the protocol fee, and the
      temporary invoice rent is returned automatically.
- [ ] No close or reclaim instruction exists in the program, IDL, API, or web
      client.
- [ ] Token sellers need no SOL; token fees are deducted in the token.
- [ ] The create-payment, shop, and checkout pages show the fee/rent estimate.
- [ ] The IDL, API, and web client are deployed together.
- [ ] The docs and comments match the deployed behavior.
- [ ] This plan is moved to `plans/archive/` and listed in
      `plans/README.md`.
