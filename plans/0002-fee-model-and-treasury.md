# Plan: Fee Model, Treasury, and Budget Accounting

## Status

In Progress — new (2026-11-12), split from the 2026-11-09 contract security
draft; revision 2 (2026-11-13) notes the receipt consolidation that landed in
`0001`; revision 3 (2026-11-13) records Phase 1 Tasks 1.1 and 1.2 as built and
tested on the program, with the client half (Tasks 1.3–1.5) and any deployment
still to come; revision 4 (2026-11-14) recomputes every fee and rent figure for
the 301-byte receipt and starts Task 1.4 in the web client; revision 5
(2026-11-15) records Phase 2 as built — the refill counter fixed, deployed to
devnet and checked on chain, Task 2.4's harness tests written — and ticks the
tasks the code and the cluster had satisfied (2.1, 2.3, 4.5), including the
receipt-rent credit that closed the gap Task 2.4 found. Phase 4 is under way:
the session-key sweep (4.1) is done and checked on devnet, and the documentation
that promised the old model (4.2) is corrected. Revision 6 (2026-11-16) makes
the token settle-side run in the Rust harness at all: the two SPL programs it
CPIs into are committed as fixtures and registered in the bank, so the fee
split, the float, the vault drain and the wrong-mint and decoy-vault
rejections now execute instead of being described around. That closed Task
3.3's program half (canonical-ATA vaults, `MintMismatch` on close) and most of
3.1 and 3.4. What it also showed is that the float cannot carry three fresh
sellers at the bank's rent and that there is no refill path, so Task 3.2 and
Checkpoint 3 stay open. Revision 7 (2026-11-16) adds that refill path and needs
no program change: the refund is read from the budget's lamports, so the buyer's
own transfers are the whole top-up, computed in the client as the difference
between one payment's need and what each account holds. Revision 8 (2026-11-16)
then sized the floats themselves for three such payments — 12,000,000 on the
session key and 18,000,000 on the budget, 0.03 SOL together — so the float
carries three fresh sellers before any top-up and Task 3.2 and Checkpoint 3 are
closed. Moving the token flows onto `Config::session_fee_float` and merging the
two constants stays deferred (revision 8). Phase 3's open work is 4.3–4.5.
Revision 10 (2026-11-17) audits the plan against the code and sorts what is left:
Phases 1–3 are built, and most of their unticked boxes are verification this repo
does not run (a manual localnet, an `anchor test` pass), not missing code. 4.1,
4.2 and 4.5 are done. Open work is grouped under **Remaining Work** below; the
substance of it is Task 4.3 (program events), the un-actioned half of Task 4.4
(no Token-2022 rejection, only `Config`'s size pinned), the boundary cases in 1.6
and 3.1, one closure decision in 3.4, and 5.2–5.3. Revision 11 (2026-11-17) puts
a chain read back beside the poll, on a schedule of its own: the API's `confirmed`
read closed the reload window the old per-tick read existed for, but a page whose
buyer has just switched currency is answered by the chain first, and an open tab
should not wait out a minute of API ticks to hear it. The schedule is
`chainReadGapAfter` in `app/utils/chainReadGap.ts` — a pure function of the read
count, tested like `holdSlop` — at most a minute apart, starting over on a
currency switch, with the poll's API read left exactly as it was. Revision 12
(2026-11-18) takes the closure decision Task 3.4 was waiting on: the payment
account closes to the pool that funds it, never to the seller, and
`pay_from_token_budget` gains the treasury accounts and the same fee split
`settle_token_payment` already has. The program change that implies is not
built.

## Remaining Work

Audited 2026-11-17 against `apps/contracts/programs/payvand` and `apps/web`. Each
line says where it stands, so a later pass does not have to re-derive it.

### Code

- **Task 4.3 — program events: not started.** The program logs with `msg!` only:
  no `#[event]`, no `emit!`, and `target/idl/payvand.json` has no `events`
  section. What it needs is unchanged: a settle event carrying the split, a
  `ConfigUpdated` event, and the IDL regenerated and copied.
- **Task 4.4 — Token-2022: decided, not enforced.** No program check rejects a
  Token-2022 mint with a named error; a payment into one fails inside the SPL
  CPI. The layout half is partly pinned: `CONFIG_SPACE` is asserted in the
  harness inside `initialize_config_records_the_upgrade_authority`), `BUDGET_SPACE` and
  `TOKEN_BUDGET_SPACE` are not.
- **Task 1.6 — closed (2026-11-18).** All three boundary cases now execute:
  `fee_floors_rather_than_rounds_at_ninety_nine_lamports`,
  `a_ten_thousand_bps_policy_admits_no_sol_invoice`, and
  `withdrawing_the_treasury_to_its_rent_underfunds_the_next_settle`.
- **Task 3.1 — the token cap is exercised (2026-11-18)**, by
  `a_ten_thousand_bps_policy_leaves_the_token_seller_one_base_unit`. What is
  left is the zero-fee transfer skip, which nothing pins. The handler's
  `require!(amount > fee_t)` cannot fire — `fee_for` caps the fee at
  `amount - 1` — so the cap test is as close to that floor as the program can be
  driven.
- **Task 3.5 — the revision-12 program change, written out and not started.**
  Task 3.4's close decision is taken, and the work it implies is now a numbered
  task in Phase 3 — the account list, the fee split, the pool-funded rents, the
  close, the IDL rebuild, the `useBudget.ts` builder and the harness assertions
  it moves — so nothing about it has to be re-derived here.

### Verification this repo does not run

- `anchor test` appears in 1.6, 5.1 and 5.3. The suite this repo runs is
  `cargo test --manifest-path apps/contracts/programs/payvand/Cargo.toml --test
  settle` against the committed SBF artifact and the SPL ELF fixtures.
- Manual localnet/browser checks: 1.4 (config TTL refresh), 1.5 (fee note on a
  settled invoice), 2.3 (float drained, UI warns), 4.2 (close/rotate and invoice
  flows in a browser).

### Rollout record

- **Task 5.2** — the devnet rollout is recorded in the revision notes (deployed,
  `Config` set, budgets opened and paid from). Three checkboxes stay open because
  nothing recorded them: the close-all-budgets sweep on devnet, `solana program
  show <PROGRAM_ID>`, and `solana account <CONFIG_PDA>` / `<TREASURY_PDA>`.

### Deferred by decision

- **Task 1.3 — `BuyerPays`.** Every settle path requires `SellerPays` and refuses
  `BuyerPays` with `FeeModeUnsupported` (`sol.rs`, `token.rs`), so the mode exists
  in `Config` and charges nothing. The charging half was never built and no
  product asks for it.
- Token-2022 migration (`token_interface`), and merging the two float constants
  onto `Config::session_fee_float` (revision 8).

### Archival

- **Task 5.3** — `plans/README.md` still lists this plan as `Draft`, the file has
  not moved to `plans/archive/`, and the follow-ups (Token-2022, relayer/sponsor,
  per-shop fees) are not written out. Everything here is ready for that step once
  4.3/4.4 are decided; nothing in the archive flow depends on code.

## Revision Notes

Revision 2 records one `0001` implementation change that moves this plan's
numbers:

- `0001` shipped a single `PaymentReceipt` (301 bytes, 2,985,840 lamports rent)
  keyed by the invoice address, instead of separate SOL and token receipts
  (237 and 269 bytes). `0001`'s revision notes explain why.
- Every `R_r` (receipt rent) figure in this plan therefore moves to 2,985,840:
  the treasury's per-invoice outlay, the break-even invoice (now
  2,985,840 / `fee_bps`), and the treasury delta tables.
- The money tables were recomputed from 2,985,840 in revision 4 (2026-11-14):
  break-even at 2% is 0.149292 SOL, the treasury floor before any settle is
  3,876,720 lamports, and `MIN_SESSION_FEE_FLOAT` in the program is 5,477,520.
  Keep the rent read over RPC as the single source of truth. No decision in this
  plan changes because of the consolidation.

Revision 3 also records what building Task 1.2 taught:

- The program cannot start a CPI while its current instruction frame has an
  outstanding lamport delta (`UnbalancedInstruction`). The treasury refund is a
  system transfer signed by the treasury seeds, so in the budget path it now
  runs **before** the direct lamport moves; the direct path refunds after the
  receipt write and the auto-close, which nets to zero.
- `Custom(102)` (`InstructionDidNotDeserialize`) is the error for an unknown
  `FeeMode` byte: Borsh rejects the instruction data before the handler runs,
  which is the named rejection the acceptance criteria asked for.
- The upgradeable loader's state variants are ordered `Uninitialized`, `Buffer`,
  `Program`, `ProgramData`, so the `ProgramData` bincode tag is `3`; the harness
  pins that against `bincode::serialize` instead of trusting a guess.

Revision 6 records what making the token flows executable taught (2026-11-16):

- `solana-program-test` has no SPL, but `ProgramTest::add_account` of a dumped
  ELF is what `add_program` does internally. The deployed token and
  associated-token programs are committed under
  `apps/contracts/programs/payvand/tests/fixtures/` (devnet dumps, addresses
  and hashes in the README there) and registered by `program_test()`, so the
  suite pays real invoices with no cluster and no network. Both dumps execute:
  the token program is the legacy SBF build, the associated-token program is the
  deployed eBPF build (`e_machine` `0x00f7`) and still runs here.
- The two token paths do not share a fee policy. `settle_token_payment` splits
  `amount` into `amount - fee_t` for the seller and `fee_t` for the treasury's
  account for the mint; `pay_from_token_budget` has no treasury account in its
  interface and moves the whole amount to the seller. Task 3.1's split is
  therefore verified on the payer path only — the budget path has no fee to
  split until the plan decides whether it should carry one.
- A fresh seller costs the float two things: its own account rent, paid by the
  session key, and the invoice and receipt rents, advanced by the session key
  and refunded by the budget. The invoice rent then follows the sale to the
  seller on the auto-close, so the buyer's float hands a fresh seller 2,714,400
  lamports at the bank's rent. That is the handler's documented behaviour, but
  it is not what Task 3.4's "payment account closed to the rent payer" says.
- `TOKEN_BUDGET_FEE_FLOAT` and `SESSION_FEE_FLOAT` are 10,000,000 each. At the
  harness's 6,960 lamports a byte a fresh seller needs 7,739,520 of them
  together, so the third consecutive payment is refused for want of lamports;
  at devnet's 5,080 a byte the same overhead is 5,648,960 and three fit. The
  float therefore meets Task 3.2's three-seller criterion on live rent by
  arithmetic, not by test, and only until the first refill-less exhaustion.

Revision 8 records Task 3.2's decision (2026-11-16):

- The task asked for three things: `Config::session_fee_float` as the token
  flows' target, no separate `TOKEN_BUDGET_FEE_FLOAT`, and three fresh-seller
  payments inside the float. Only the third is something a buyer feels, and it
  is the one the criterion measured, so it drove the choice.
- Raising the two constants closes it in two lines with no state change, no IDL
  change and no migration: `SESSION_FEE_FLOAT` 10,000,000 → 12,000,000 and
  `TOKEN_BUDGET_FEE_FLOAT` 10,000,000 → 18,000,000. At mainnet's 6,960 lamports a
  byte the third payment needs a session key that starts with 11,833,080 and a
  budget holding 17,100,720 of refund float; the rounded values leave about
  1,200,000 of headroom over three payments, and together they are the
  30,000,000 (0.03 SOL) this plan budgets for a session float.
- Merging the two into one `Config` float was not taken. It is not cheaper for
  the buyer: with the budget's refund gone the session key would have to front
  the 5,700,240 of records per payment that the SOL path never touches, so the
  same three payments would peak near 23,200,000 of float — and that one figure
  would then size every SOL budget as well. It also changes what an existing
  `session_fee_float` means on devnet and forces a client re-quote, for no change
  in what is locked or what the buyer keeps.
- What the buyer locks per token budget goes from 0.02 to 0.03 SOL, which is the
  figure this plan's session-float section already assumed; close and rotate
  still return everything unspent.
- Two fixes came with it. `openTokenBudgetCost` now quotes a refill's refund
  float as the gap `open_token_budget` charges instead of 0 — Task 3.2's last
  criterion that was a real defect rather than a preference. And the harness now
  sends allowance payments with the session key as fee payer, so the fee the
  float exists to cover is measured instead of being paid by the harness's
  default payer.

Revision 9 records the checkout's reload window (2026-11-17):

- The confirming stage holds the pay button away while **this page's own** send
  is in flight: `signature` is set, so `invoicePayable` is false and the stage
  reads `confirming` until the record says settled. A reload has no signature —
  it is a new page — and the record it reads first is the API's, which trails the
  chain by a few seconds. So the button came back for an invoice the chain
  already held, and the press earned `AlreadySettled` (6002): the error the
  confirming stage exists to prevent, one page load later.
- `load()` now asks the chain as part of loading: for every address this code can
  be paid on (SOL and each offered token) it reads that address's receipt at
  `confirmed`, and writes a settled record when one exists. The receipt is the
  only account worth reading — every `settle_*` closes its invoice in the same
  transaction, so a settled invoice leaves nothing else behind. A code that takes
  SOL and USDC has one address per currency, so every offered address is checked
  rather than the one the currency switch happens to be on; the reads run in
  parallel, so a load costs one round trip, not one per currency.
- The answer goes into `record`, not into a flag beside it, so the stage,
  `invoicePayable` and the poll read one state and cannot disagree. Only a
  positive answer moves the record: a read that errors, or a code whose accounts
  are both absent, leaves the invoice payable, because a flaky node must not talk
  a buyer out of a payment they can still make.
- The trail was the API's commitment, not the chain's: `RpcClient::new` reads at
  `finalized`, which on mainnet lags the `confirmed` a send waits for by a dozen
  seconds. `fetch_payment` now builds its client with `payment_rpc` —
  `new_with_commitment(endpoint, CommitmentConfig::confirmed())` — so the record
  catches up within a slot or two of the payment, the same level the checkout's
  own reads use and the wallet that sent it reports. `invoice_reads_use_confirmed_commitment`
  pins it, because the constructor's default is the bug.
- That made the poll's own chain read redundant, and it is gone: the record is
  `confirmed` now, so the next API tick — five seconds at most — is where an
  invoice paid on another device shows up. The page does keep a chain read of its
  own, by decision: a reload can still land in the slot or two between the cluster
  confirming a send and the API's node voting it `confirmed`, and the chain is
  where a currency the buyer has just switched to is answered first. It runs at
  load, on a currency switch, and then beside the poll on the schedule in
  `app/utils/chainReadGap.ts` — one tick behind the first poll, each gap doubling
  until it is a minute — so an invoice nobody has paid costs the buyer's RPC a
  read a minute instead of three every five seconds. `chainReadGapAfter` is a pure
  function of the read count with its own tests, the switch resets the count, and
  the poll's API read is untouched: the record still settles from the API alone.
  Every caller shares `watchAddresses()`, so the addresses watched at load, on a
  switch and on the schedule cannot drift.
- A settled record the chain produced is not the end of the poll: it has no payer
  until the API's node catches up, and the sheet names the payer from that field
  alone. So the poll stops on a settled record *that names its payer*, takes only
  `settled_by` from a lagging answer — never its status, which must not unsettle an
  invoice the chain holds — and stops once the payer arrives. `stops reading a
  settled record, API and chain alike` advances six intervals on a paid page and
  asserts neither count grows; `keeps a chain-settled page off the chain while the
  API still lags` holds a settled page to its load-time read across a minute of
  polling; and `reads the chain on a schedule that doubles, while the API keeps to
  every tick` plus `tops the gap out at a minute, so a long-open page still asks
  now and then` count the schedule itself — three chain reads in a minute, twelve
  in ten — so it cannot quietly become one per tick, and
  `starts the chain schedule over when the buyer switches currency` fails if the
  address just chosen waits behind the old schedule.
- Pinned in `usePaymentCheckout.test.ts`, in `a page that loads with the payment
  already on chain`: `offers no button when the receipt exists and the record has
  not caught up` fails if the load-time read is removed, `settles the open page
  from an API tick, and asks the chain nothing more` fails if the poll drops the
  API's answer, `names the payer once the API
  catches up with a payment the chain already held` fails if the poll stops on any
  settled record, and `still offers the invoice when the chain holds nothing for
  it` keeps the fix from turning every load into a paid page. The window is a rule the checkout enforces, not a lucky read.

Revision 12 records the closure decision Task 3.4 was waiting on (2026-11-18):

- The product rule, as the buyer states it: the payer pays exactly the price the
  seller named, in whatever currency the invoice is in; every fee comes out of
  that amount; SOL-denominated costs come out of the treasury's SOL pool; the
  token fee taken from the amount lands in an address we hold, so it can be
  swapped for SOL and put back into that pool; the seller receives the amount
  less the fee; and the rent of a closed account goes back to whoever paid it.
- That makes the criterion right and the handler wrong. Task 3.4 asked for the
  payment account to close to the rent payer; `pay_from_token_budget` closes it
  to the seller, who advanced nothing, so a fresh seller walked away with
  `amount` plus 2,714,400 of the buyer's lamports. A seller is owed
  `amount - fee_t`, and the invoice rent is not part of it.
- The shape the rule forces is the one `settle_token_payment` already has: the
  pool funds the records and receives the closed invoice. `pay_from_token_budget`
  gains `config`, `treasury` and `treasury_token_account`, moves `amount - fee_t`
  from the vault to the seller and `fee_t` to the treasury's account for the
  mint, refunds the session key's advanced invoice and receipt rents out of the
  pool through `pay_from_treasury` (a CPI, so before the direct lamport moves),
  and closes the invoice to the treasury. Both token paths then charge the same
  fee and carry the same rent. The budget's own refund float has nothing left to
  do: Task 3.2's `TOKEN_BUDGET_FEE_FLOAT` paid a job the pool now does, and its
  boundary needs re-measuring with it.
- What this restores is the plan's own invariant rather than inventing one: the
  pot pays exactly `amount`. The SOL budget path already holds that; the token
  one drains `invoice + receipt` on top of the price. With the pool funding the
  records, the pool's position per payment is one receipt rent out and `fee_t`
  in — the same 2,985,840 break-even the SOL path carries, which at 2% is a
  149,292,000-base-unit price whatever the mint's decimals — and the invoice rent
  comes back through the close.
- What it does not move: the ATA rent of a seller that has none is still
  advanced by the session key (Task 3.2 sized for it), which is the one cost of a
  fresh seller the pool does not carry.
- Not built, and nothing about the deployed program changes yet. The account
  list is the IDL and the client's builder, and the harness executes the
  committed SBF artifact, so the source change only counts once the program is
  rebuilt and the IDL copied; the work is Task 3.5.

## Linked Docs

- `plans/README.md` — plan conventions and lifecycle.
- `plans/archive/0001-contract-security-hardening.md` — prerequisite: receipt-backed
  settlement, auto-close, and the status model. This plan assumes `0001` is
  merged and deployed.
- `README.md` — project overview.
- `apps/contracts/programs/payvand/src/state.rs` — account layouts.
- `apps/contracts/programs/payvand/src/constants.rs` — seeds, spaces, floats.
- `apps/contracts/programs/payvand/src/instructions/sol.rs` — SOL invoices and
  budgets.
- `apps/contracts/programs/payvand/src/instructions/token.rs` — token invoices
  and budgets.
- `apps/contracts/programs/payvand/src/lib.rs` — entrypoints and documented
  guarantees.
- `apps/api/src/payments.rs` — on-chain decoders, rent fields.
- `apps/web/app/composables/useBudget.ts` — builders, rent reads, UI checks.
- `apps/web/app/composables/usePaymentCheckout.ts` — direct settle builder.
- `apps/web/app/composables/useSessionKey.ts` — session-key storage and
  forgetting.
- `apps/web/app/pages/budget.vue`, `sell.vue`, `shops/[id].vue`, `pay.vue` —
  fee/rent display and preflight checks.
- `apps/web/app/assets/payvand.idl.json` — client IDL.

## State Inherited From `0001`

This plan starts from the code `0001` leaves behind:

- The payment account is created, settled, and closed in one transaction; the
  receipt PDA is the permanent record and the replay guard.
- The receipt rent is advanced by the payer (or session key) and **not**
  refunded, so a direct SOL invoice costs the payer `amount + receipt_rent`.
  Phase 1 here replaces that with the treasury refund.
- There is no `Config`, no treasury, and no protocol fee.
- `pending` / `settled` / `missing` are defined against the receipt.
- Budget settlements make the session key whole for `payment_rent +
  receipt_rent`, and the pot pays both rents on top of the invoice amount.
- One `PaymentReceipt` type covers SOL and token invoices (301 bytes,
  `["payment-receipt", invoice]`, with `Pubkey::default()` for a SOL mint), and
  every settle path writes it before closing the invoice to the seller.
- The minimum payable invoice is the payment rent (`InvoiceTooSmall`), which
  this plan raises once the fee is added.
- Deployed state: the program is upgraded on devnet under the original ID, the
  API and web app are live against it, and the devnet invoices left over from
  the old program are already settled and therefore inert.

## Objective

Make the economics explicit and correct: the payer pays exactly the price shown
on the QR, the seller bears a percentage fee deducted from its proceeds, the
protocol treasury pays the permanent receipt rent for both SOL and token
invoices, the temporary invoice rent returns automatically, and the monthly
budget records what it actually paid. The treasury is a program PDA, so no
caller can redirect the fee.

## Scope

In scope:

- A `Config` PDA (fee policy, session float) and a program-owned treasury PDA.
- Seller-pays fee split for SOL and token invoices; the fee goes to the
  treasury and nowhere else.
- Treasury-funded receipt rent for SOL **and** token invoices, refunding the
  payer (or session key) that advanced it.
- Explicit underflow guards and a minimum payable invoice amount.
- Budget accounting: the pot pays exactly the invoice amount, `spent` matches
  it, same-month top-ups preserve `spent`.
- Token fee splitting, treasury token ATA, and session-key float sizing with a
  top-up path.
- `close_token_budget` mint check and canonical vault constraint.
- Session-key sweep on close/rotate, corrected documentation, and program
  events.
- Token-2022 decision, `Config` layout test, dead-code cleanup.
- Devnet rollout of the fee model.

Out of scope:

- Anything in `0001` (close removal, receipt creation, status model).
- Payer-side gasless/relayer support: the payer still needs SOL for the network
  fee and the temporary invoice rent on token invoices until a sponsor exists.
- Token-2022 migration (documented as unsupported, tracked separately).
- Mainnet deployment, per-shop fee overrides, discounts, a full event indexer.

## Dependencies

Product decisions (adopted; the numbers below back them):

- **Fee: `fee_bps = 200` (2%), `fixed_fee_lamports = 0`.** The seller bears it.
  The payer pays the displayed price.
- **Treasury funds the receipt rent for both paths.** No `fee >= receipt_rent`
  rule, so a small invoice is payable instead of rejected after its QR was
  shown. The trade-off is a per-invoice subsidy below break-even; see "Fee And
  Rent Numbers".
- **The treasury is a program PDA, not a wallet.** `[TREASURY_SEED]`, holding
  SOL, receiving all SOL fees and paying all receipt rent. The settle contexts
  derive it from `Config.treasury_bump`, so the payer — who builds the
  transaction — has no account to substitute. Admin surplus leaves via
  `withdraw_treasury`.
- **Token fees go to the treasury's ATA for that mint**, created and funded by
  the treasury itself on first use, never by the payer.
- **For token invoices the payment account closes to whoever advanced its
  rent** — the payer for a direct payment, the session key for a budget
  payment. Revision 1 of the draft closed it to the seller, which made the
  payer's SOL a rebate to the seller; with the treasury covering receipt rent,
  the seller has no SOL exposure and needs no rebate.
- **A same-month top-up never resets `spent`.** Only a calendar-month change
  does.
- **`fee_mode` stays in `Config`** (`SellerPays` default, `BuyerPays` for future
  products) but the UI only ever sends `SellerPays`.
- **Only the program's upgrade authority can initialize `Config`**, and the
  `admin` it records is that signer. The check reads this program's `ProgramData`
  upgrade authority through the loader's own PDA, which closes the "first caller
  on a fresh devnet installs themselves" hole.

Technical decisions:

- Fee math is integer and floors in the seller's favour:
  `fee = min(amount * fee_bps / 10_000 + fixed_fee_lamports, amount - 1)` for
  SOL, `fee = min(amount * fee_bps / 10_000, amount - 1)` for tokens.
- Settle requires `amount > payment_rent + fee` (`InvoiceTooSmall`) so no
  subtraction can underflow. This replaces the removed `FeeTooLow` rule.
- The treasury must hold its own rent-exempt minimum plus the receipt rent, or
  settle fails with `TreasuryUnderfunded` before any transfer.
- `Config` is created once per deployment; the client caches it with a TTL and
  refuses to estimate costs while it is unreadable.

Open questions (do not block Phase 1, but decide before Phase 3/5):

- Who funds the treasury initially on devnet (admin wallet transfer, expected
  size 0.05–0.1 SOL), and what is the monitoring threshold before it runs dry?
- Is 2% the launch rate, or a placeholder for a smaller pilot?
- Do we ever need per-shop fee overrides, or is one global policy enough?
- Should the fee recipient stay a PDA permanently, or move to a multisig once
  the protocol has one?

## Fee And Rent Numbers

Rent uses Solana's rent-exempt formula, `(bytes + 128) × lamports_per_byte`.
These values are what the program and the client must agree on; tests assert
them via `Rent::minimum_balance`, and the web client already reads rent over RPC
(`useBudget.ts` → `rentMinimum`), so nothing hardcodes a stale number except
these planning figures.

**The per-byte rate drifted (2026-11-15).** The tables below were written at
6,960 lamports per byte; devnet now charges **5,080** — `Budget` 1,143,000 for
97 bytes, the 301-byte receipt 2,179,320, a token account 1,488,440. Read the
figures here as ratios: the break-even rule (`fee_bps × amount ≥ receipt rent`)
and the treasury floor (`receipt rent + the treasury's own rent`) are the durable
parts, and `README.md`'s "Fees, rent and the treasury" states them with the
live receipt rent.

| Account | Bytes | Lamports | SOL |
| --- | ---: | ---: | ---: |
| `Config` | 62 | 1,322,400 | 0.0013224 |
| `Budget` | 97 | 1,566,000 | 0.0015660 |
| `TokenBudget` | 129 | 1,788,720 | 0.0017887 |
| `Payment` | 230 | 2,491,680 | 0.0024917 |
| `TokenPayment` | 262 | 2,714,400 | 0.0027144 |
| `PaymentReceipt` (one type, both currencies) | 301 | 2,985,840 | 0.0029858 |
| Token account (ATA) | 165 | 2,039,280 | 0.0020393 |
| Treasury (0 data) | 0 | 890,880 | 0.0008909 |

`Config` = `8 + 32 admin + 2 fee_bps + 8 fixed_fee_lamports + 1 fee_mode +
8 session_fee_float + 1 bump + 1 treasury_bump + 1 version`.

### SOL invoice at 2%, treasury-funded receipt rent

Payer pays `amount` in total (the receipt rent is advanced at `init` and
refunded by the treasury in the same transaction). Seller receives
`amount - fee`. Treasury delta is `fee - R_r`, where `R_r = 2,985,840` is the
301-byte receipt.

| Invoice (SOL) | Lamports | Fee | Seller receives | Treasury delta |
| --- | ---: | ---: | ---: | ---: |
| 0.005 | 5,000,000 | 100,000 | 4,900,000 | −2,885,840 |
| 0.01 | 10,000,000 | 200,000 | 9,800,000 | −2,785,840 |
| 0.05 | 50,000,000 | 1,000,000 | 49,000,000 | −1,985,840 |
| 0.149292 | 149,292,000 | 2,985,840 | 146,306,160 | 0 |
| 0.5 | 500,000,000 | 10,000,000 | 490,000,000 | +7,014,160 |
| 1 | 1,000,000,000 | 20,000,000 | 980,000,000 | +17,014,160 |
| 10 | 10,000,000,000 | 200,000,000 | 9,800,000,000 | +197,014,160 |

Break-even invoice (fee = receipt rent), where the treasury stops subsidising:

| `fee_bps` | Break-even | In SOL |
| ---: | ---: | ---: |
| 100 (1%) | 298,584,000 | 0.298584 |
| 150 (1.5%) | 199,056,000 | 0.199056 |
| 200 (2%) | 149,292,000 | 0.149292 |

Consequences to state in the docs and the admin view:

- Invoices below 0.149292 SOL at 2% are subsidised by the treasury at
  `2,985,840 − floor(amount × 0.02)` lamports each, roughly
  `0.00299 − 0.02 × amount` SOL. That is the price of accepting small
  invoices; the alternative (rejecting them) is what revision 1 did.
- Raising `fixed_fee_lamports` to ≈3.0M (2,985,840) would make every invoice
  self-funding, but a flat 0.003 SOL fee is 30% of a 0.01 SOL payment. Keep the
  default at 0 and let `fee_bps` carry the revenue.
- The treasury must hold at least `R_r + 890,880` = **3,876,720 lamports
  (0.0039 SOL)** before a settle. One receipt type serves both currencies, so the
  SOL and token settlements have the same floor (revision 1 needed 3,431,280 for
  SOL and 3,654,000 for tokens).

### Minimum payable invoice (underflow guard)

`amount > payment_rent + fee`, with `fee = floor(amount × 0.02)`:

- Solve `amount > 2,491,680 + floor(amount × 0.02)` → the smallest passing
  amount is **2,542,531 lamports (0.0025425 SOL)**; at 2,542,530 the guard
  rejects it (`fee` 50,850 plus rent 2,491,680 equals the amount).

Revision 1's `fee >= receipt_rent` rule implied a ≈0.254 SOL floor at 1% and a
≈0.127 SOL floor at 2%. With treasury funding the hard floor drops to 0.00254
SOL, and everything between that and break-even is payable but subsidised.

### Per-flow ledger (1 SOL invoice at 2%)

Direct payment:

| Account | Lamports | Note |
| --- | ---: | --- |
| Payer | −1,000,000,000 | net after the treasury refunds `R_r` |
| Seller | +980,000,000 | `amount − fee` |
| Treasury | +17,014,160 | `fee − R_r` |
| `PaymentReceipt` | 2,985,840 locked | permanent record |

Budget payment (session key advanced `R_p + R_r`, pot reimburses both):

| Account | Lamports | Note |
| --- | ---: | --- |
| Pot | −1,000,000,000 | exactly `amount`; `spent += amount` |
| Seller | +980,000,000 | includes the `R_p` returned by close |
| Treasury | +17,014,160 | `fee − R_r` |
| Session key | 0 | rents advanced are reimbursed |
| `PaymentReceipt` | 2,985,840 locked | permanent record |

`BuyerPays` variant (Task 1.3, direct settle only — the budget path rejects it):

| Account | Lamports | Note |
| --- | ---: | --- |
| Payer | −1,020,000,000 | `amount + fee`, after the treasury refunds `R_r` and close returns `R_p` |
| Seller | +1,000,000,000 | exactly `amount` |
| Treasury | +17,014,160 | `fee − R_r` |
| `PaymentReceipt` | 2,985,840 locked | permanent record |

### Token invoice (1,000,000,000 base units at 2%)

| Account | Tokens | SOL |
| --- | ---: | ---: |
| Payer | −1,000,000,000 | network fee only, plus new seller ATA rent if it had none |
| Seller | +980,000,000 | `amount − fee_t` |
| Treasury ATA | +20,000,000 | `fee_t` |
| Treasury (SOL) | — | −2,985,840 refunding the receipt rent |

The seller needs no SOL. The payer's SOL exposure is the network fee and, on a
fresh seller, the seller's ATA rent (2,039,280) — both disclosed at checkout.

### Session-key float

Per worst-case token payment to a fresh seller:

`R_p + R_r + seller ATA = 2,491,680 + 2,985,840 + 2,039,280 = 7,516,800`
lamports (0.0075168 SOL), of which only the ATA rent (and the network fee) is
consumed. `session_fee_float = 30,000,000` (0.03 SOL) covers three such payments
(22,550,400 peak) with headroom. For SOL budget payments the peak is
`R_p + R_r = 5,477,520` and is fully reimbursed, so the same float is ample. The
program's `MIN_SESSION_FEE_FLOAT` is that same 5,477,520, so a config can never
be given a float that cannot front one payment.

## Issues Addressed

Findings 1, 2, 10, 11, 13, 14, 18, 20, 21, and 23 are closed by `0001`.

| ID | Issue | Severity | Phase |
| --- | --- | --- | --- |
| 3 | SOL budget `spent` excludes invoice rent | High | 2 |
| 4 | Refills reset the monthly counter | High | 2 |
| 5 | Token session float consumed by ATA rent | High | 3 |
| 6 | Session-key float never returned | High | 4 |
| 7 | `close_token_budget` accepts the wrong mint | Medium | 3 |
| 8 | Hidden rent/fee costs; no fee transparency | Medium | 1 and 4 |
| 9 | No events; the record is invisible to indexers | Medium | 4 |
| 12 | Vault not constrained to the canonical ATA | Medium | 3 |
| 15 | SPL Token only; no Token-2022 | Low | 4 |
| 16 | Manual account spaces | Low | 4 |
| 17 | Old session-key float stranded on rotation | Low | 4 |
| 19 | `fee >= receipt_rent` made small invoices unpayable | High | 1 |
| 20 | Fee form of the `amount − rent − fee` underflow | High | 1 |
| 22 | Fee recipient was caller-supplied (fee redirectable) | Critical | 1 |
| 24 | `Config` absent during the upgrade window fails every settle | Medium | 5 |

## Phases

### Phase 1: Config, treasury, and the SOL fee split

**Built so far (2026-11-13):** Tasks 1.1 and 1.2 are implemented on the program
and covered by the Rust harness (20 tests green, `cargo clippy --all-targets --
-D warnings` clean). The deployed devnet program is still the `0001` build, and
the web client and the copied IDL still target the `0001` account lists — they
gain the `config` and `treasury` accounts in Task 1.4, so nothing is deployable
until then.

Goal: replace the interim payer-funded receipt rent from `0001` with a
treasury-funded model, add the fee policy, and make the fee destination
impossible to redirect.

#### Task 1.1: Add the `Config` and treasury accounts

Description: Introduce `Config` (singleton PDA) and the program treasury PDA.

`Config`:

- `admin: Pubkey` — may update the fee policy and withdraw surplus. It is
  recorded at initialization as the upgrade authority that signed, never as an
  argument the caller chooses.
- `fee_bps: u16`, `fixed_fee_lamports: u64`, `fee_mode: FeeMode`.
- `session_fee_float: u64` — replaces the hardcoded `SESSION_FEE_FLOAT`.
- `treasury_bump: u8`, `bump: u8`, `version: u8`.

Instructions:

- `initialize_config(fee_bps, fixed_fee_lamports, fee_mode, session_fee_float)`
  — creates `Config` once per deployment. Only the program's upgrade authority
  may call it, and the `admin` it records is that same signer; no caller can
  install an admin of its choosing.

  Accounts: `admin: Signer<'info>` (also the payer of `Config`'s rent),
  `program_data: UncheckedAccount<'info>` with the constraint
  `seeds = [crate::ID.as_ref()], bump, seeds::program =
  bpf_loader_upgradeable::id(), owner = bpf_loader_upgradeable::id()` — which
  pins it to this program's `ProgramData` PDA and makes a decoy account
  impossible; `config` (`init`, `[CONFIG_SEED]`), `treasury: SystemAccount`
  (`[TREASURY_SEED]`), `system_program`. No `program` account is needed:
  `crate::ID` is the program address.

  Authority check, in the handler (`anchor_lang::solana_program::bpf_loader_upgradeable`):

  1. `UpgradeableLoaderState::try_from_slice(&program_data.data.borrow())`;
     anything but `ProgramData` → `ProgramNotUpgradeable`.
  2. `upgrade_authority_address == Some(admin.key())` → proceed; `None` →
     `ProgramNotUpgradeable` (a finalized program has no authority to
     bootstrap from, and a random signer must never become admin); `Some(other)`
     → `InvalidConfigAdmin`. `Signer` already proves that key signed, so no
     separate signature check is needed.

  `ProgramData` is not wrapped in `Account<ProgramData>`: the upgradeable loader
  type is not an Anchor account, so the state is deserialized explicitly as
  above.

  `admin` pays `Config`'s rent (1,322,400 lamports), so the rollout preflight
  funds it. `update_config` changes policy only and never rewrites `admin`;
  rotating the admin or recovering a lost key is an upgrade under the same
  authority, and `DEV.md` must say so. Because the check needs `ProgramData`, a
  program deployed without an upgrade authority can never initialize `Config` —
  document that the program stays upgradeable until `Config` exists.
- `update_config(...)` — admin only; validates `fee_bps <= 10_000`,
  `fee_mode` is a valid variant, and `session_fee_float` is above the configured
  floor. The `ConfigUpdated` event is added in Task 4.3.
- `withdraw_treasury(amount)` — admin only; moves surplus SOL out of the
  treasury while leaving its rent-exempt minimum. There is no user-facing path
  that touches the treasury.

Treasury: `[TREASURY_SEED]` PDA, system-owned, no data. It is referenced in
settle contexts as
`#[account(mut, seeds = [TREASURY_SEED], bump = config.treasury_bump)]`, which
is the fee-recipient constraint: the payer cannot name a different account, and
`Config` stores no replaceable recipient address.

Acceptance criteria:

- [x] `Config` exists with the fields above and `CONFIG_SPACE = 62`.
- [x] `initialize_config` succeeds only when signed by the upgrade authority,
      and records that signer as `admin`.
- [x] A funded non-authority signer fails with `InvalidConfigAdmin`.
- [x] An account that is not this program's `ProgramData` PDA fails the seeds
      and owner constraint.
- [x] A finalized program (`upgrade_authority_address == None`) fails with
      `ProgramNotUpgradeable`.
- [x] `initialize_config` can run once; a second call fails because the config
      PDA already exists.
- [x] `update_config` rejects `fee_bps > 10_000` (`InvalidFeeBps`), an
      invalid mode (`InstructionDidNotDeserialize`, because Borsh rejects the
      unknown enum byte), and a zero float (`InvalidSessionFloat`); a test
      covers each.
- [x] `withdraw_treasury` is admin-only and cannot drain the treasury below its
      rent-exempt minimum.
- [x] The treasury address is derived from seeds in the program and in the
      client, never passed as a plain account. **Both halves done:**
      `["treasury"]` in the program, referenced only through
      `config.treasury_bump`; `deriveTreasuryPda` in `usePaymentProgram.ts`,
      which every caller uses to place the account.
- [x] Token-2022 and other programs are unaffected by `Config` creation.

Verification:

- [x] Anchor test: initialize signed by the program's upgrade authority
      succeeds, and `config.admin` equals that signer.
- [x] Anchor test: initialize from a second, funded keypair fails with
      `InvalidConfigAdmin`.
- [x] Anchor test: initialize with another program's `ProgramData` fails the
      seeds/owner constraint.
- [x] Anchor test: initialize once, second initialize fails.
- [x] Anchor test: non-admin `update_config` and `withdraw_treasury` fail; admin
      update succeeds and `Config` reads back with the new policy.
- [x] Anchor test: `withdraw_treasury` leaves the rent-exempt minimum.

Files likely touched:

- `apps/contracts/programs/payvand/src/state.rs`
- `apps/contracts/programs/payvand/src/constants.rs`
- `apps/contracts/programs/payvand/src/errors.rs`
- `apps/contracts/programs/payvand/src/instructions/config.rs` (new)
- `apps/contracts/programs/payvand/src/lib.rs`
- `apps/contracts/programs/payvand/tests/settle.rs` (the Rust harness that
  replaced the old Anchor TS suite)

Dependencies: `0001` deployed.

Estimated scope: Medium.

#### Task 1.2: Seller-pays SOL settlement with treasury-funded receipt rent

Description: Implement `SellerPays` — the only mode the product uses and the
one the ledger in "Fee and Rent Numbers" describes — for both SOL settle paths.
The payer pays exactly `amount`, the seller receives `amount - fee`, the fee
goes to the treasury, and the treasury refunds the receipt rent the payer
advanced at `init`. `BuyerPays` is a separate task (1.3) so this math can land
and be verified on its own; events are Phase 4 (Task 4.3), with `msg!` logs
only here.

`settle_payment` and `pay_from_budget` changes:

- Read `Config` (PDA `[CONFIG_SEED]`); absent → `ConfigMissing`. Require
  `config.fee_mode == FeeMode::SellerPays`, else `FeeModeUnsupported` (Task 1.3
  adds the other branch).
- `fee = min(amount * fee_bps / 10_000 + fixed_fee_lamports, amount - 1)`.
- `require!(amount > payment_rent + fee, InvoiceTooSmall)` — the underflow guard
  (finding 20). Minimum at 2%: 2,542,531 lamports.
- Direct path: transfer `amount - payment_rent - fee` to the seller, `fee` to
  the treasury, close the payment account to the seller (rent back), then
  refund `receipt_rent` from the treasury to the payer.
- Budget path: the pot pays exactly `amount` — `amount - payment_rent - fee` to
  the seller, `fee` to the treasury, and `payment_rent + receipt_rent` to the
  session key as reimbursement; close the payment account to the seller.
- `require!(treasury.lamports() >= receipt_rent + treasury_rent_min,
  TreasuryUnderfunded)` before the refund.
- `Config` is read-only here; `initialize_config` is the only writer besides
  `update_config`.
- Keep `msg!` logs carrying the split so devnet settlements are readable before
  Task 4.3 adds `emit!`.

Acceptance criteria:

- [x] Direct SOL: payer balance delta is exactly `amount` plus the network fee;
      seller delta is `amount - fee`; treasury delta is `fee - R_r`.
- [x] Budget SOL: pot delta is exactly `amount`; `spent += amount`; session key
      delta is zero; seller delta is `amount - fee`.
- [x] `fee = 0` is accepted (fee-less pilot) but only via `Config`.
- [x] `amount == payment_rent + fee` fails with `InvoiceTooSmall`; one lamport
      more succeeds.
- [x] An empty treasury fails with `TreasuryUnderfunded` before any transfer.
- [x] A `Config` with `fee_mode == BuyerPays` fails with `FeeModeUnsupported`
      on both paths (until Task 1.3).
- [x] No path sends lamports to a caller-supplied fee account.
- [x] No `emit!` call is added in this task.

Verification:

- [x] Anchor test: each balance delta above at `fee_bps = 200`, plus a seller
      delta with a 1,000,000 fixed fee (the fixed fee is not repeated in every
      delta assertion).
- [x] Anchor test: `InvoiceTooSmall` boundary; `TreasuryUnderfunded` with an
      empty treasury.
- [x] Anchor test: passing an arbitrary wallet where the treasury belongs fails
      the seeds constraint.
- [x] Anchor test: `BuyerPays` config is rejected with `FeeModeUnsupported`.

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/sol.rs`
- `apps/contracts/programs/payvand/src/state.rs`
- `apps/contracts/programs/payvand/src/errors.rs`
- `apps/contracts/programs/payvand/tests/settle.rs` (the Rust harness that
  replaced the old Anchor TS suite)

Dependencies: Task 1.1.

Estimated scope: Large.

#### Task 1.3: Add the `BuyerPays` fee mode

**Status (2026-11-17): deferred by decision.** Every settle path requires
`SellerPays` and refuses `BuyerPays` with `FeeModeUnsupported` — `sol.rs` for
both the direct and the budget path, `token.rs` for the token path — which is the
safe half of this task: the mode cannot be selected and cannot charge anyone. The
charging half below was never built. The criteria stay unticked because nothing
implements them; a SKU that needs `BuyerPays` starts here.

Description: Add the config-only branch that charges the fee to the payer. The
UI never sends it and no product uses it yet; it exists so a future SKU can
charge the buyer without a program upgrade.

Rules:

- Seller always receives exactly `amount`.
- Payer sends `amount + fee`; `fee` goes to the treasury.
- The payment account closes to the payer (the account that advanced its rent),
  and the treasury refunds the receipt rent to the payer. Payer net is
  `amount + fee`.
- No `amount - payment_rent` subtraction is performed, so the underflow guard
  here is `require!(amount > 0)`, not `InvoiceTooSmall`.
- `pay_from_budget` rejects `BuyerPays` with `FeeModeUnsupported`: a budget that
  silently charged a fee the UI never showed would break the `spent == amount`
  invariant. Direct settle only, documented.
- The treasury-funding guard (`TreasuryUnderfunded`) applies unchanged.

Acceptance criteria:

- [ ] Direct SOL with `BuyerPays`: payer delta `amount + fee`, seller delta
      `amount`, treasury delta `fee - R_r`.
- [ ] Budget SOL with `BuyerPays` fails with `FeeModeUnsupported`.
- [ ] No UI path can select `BuyerPays`; `estimateInvoiceCosts` (Task 1.4)
      handles the mode for completeness and shows a payer fee line only when it
      is active.
- [ ] `SellerPays` behavior is unchanged by this task.

Verification:

- [ ] Anchor test: `BuyerPays` direct ledger deltas.
- [ ] Anchor test: `BuyerPays` budget payment fails with
      `FeeModeUnsupported`.
- [ ] Anchor test: the `SellerPays` suite still passes untouched.

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/sol.rs`
- `apps/contracts/programs/payvand/src/errors.rs`
- `apps/contracts/tests/payvand.ts`
- `apps/web/app/composables/useFeeConfig.ts`

Dependencies: Task 1.2.

Estimated scope: Medium.

#### Task 1.4: Client `Config` cache and a single cost estimator

Description: Add a client module that loads `Config` (cached with a TTL, e.g.
60 s) and exposes `estimateInvoiceCosts(amount, mint?)` returning the fee,
seller proceeds, payer total, temporary rent, receipt rent, treasury refund, and
network-fee estimate. Every page and preflight check reads this helper, so the
displayed numbers and the transaction math have one source.

**Built so far (2026-11-14):** the account wiring and the `Config` read are
done. `usePaymentProgram` derives `["config"]` and `["treasury"]`,
`settle_payment`, `pay_from_budget` and `open_budget` pass them, and the copied
IDL is regenerated from the program (it now also carries `Config`,
`initialize_config`, `update_config` and `withdraw_treasury`).
`useFeeConfig.ts` reads and decodes `Config` (60 s cache, refuses to quote if
the account is missing or mistagged), and the SOL budget quote takes its session
float from there, so `useBudget.ts` no longer keeps a copy of that number. The
token flows still read the program's fixed float until Phase 3 moves them to
`Config`.

`estimateInvoiceCosts` now lives beside that cache, in `useInvoiceCosts()`. It
reads `Config` and the cluster's rent (never a constant, so it follows the
program's `Rent::get()` — devnet's rent is about 73% of the mainnet table above,
and both sides agree because both ask the cluster), and reports the fee, the
seller's proceeds, the payer's total, both rents, the network-fee estimate and
the minimum payable amount. The minimum is solved from `amount > rent + fee`
rather than probed, and matches the program's floor exactly: 2,542,531 lamports
at 2% on mainnet rent, 2,769,796 for a token invoice. `usePaymentCheckout`
quotes it on load, on a currency switch and after a discount, shows the payer's
extras under the price, and refuses to submit below the floor or when `Config`
cannot be read. The budget obstacle now asks for exactly `amount`, since the pot
no longer advances either rent.

`BuyerPays` is quoted for completeness — the payer's total carries the fee and a
"protocol fee" line appears only when the policy is that mode — but no UI path
selects it yet.

The arithmetic itself lives in `app/utils/invoiceCost.ts`, away from the chain
and the Nuxt context: `feeOn`, `minimumPayable` and `invoiceCosts`, each a pure
function of a policy and rents. That is what `invoiceCost.test.ts` holds against
the program — the fee table including the fixed-fee case the contract suite
settles, the `InvoiceTooSmall` floor for 0%, 2%, 10% and 99.99% policies on both
mainnet and devnet rent, the 10,000 bps policy that admits nothing, and a large
invoice. The fee now multiplies in BigInt, like the program's `u128`, so a large
amount cannot lose a lamport to a float product. The suite runs with
`pnpm test:web` (vitest, added for this) and is part of `task verify`.

Still open: `shops/[id].vue` and `budget.vue` from Task 1.5.

**Deploy ordering is now load-bearing:** the client sends the new account lists,
so deploying the web app before the program is upgraded (and `initialize_config`
has run) makes every SOL settlement fail with a missing-account error. Task 5.1
must upgrade the program, initialize `Config`, and fund the treasury first; the
program also tolerates the reverse window (new program, old client) only after
the client deploy, so do the program first.

Acceptance criteria:

- [x] The helper reads rent over RPC (no hardcoded rent) and `Config` from
      chain, and matches the program's floor rounding.
- [x] If `Config` cannot be read, the checkout refuses to quote a total instead
      of assuming zero fee.
- [x] The helper reports the minimum payable invoice amount and blocks
      submission below it.
- [x] A unit test compares the helper's output with the program's math for
      several amounts, including the minimum and a large invoice.

Verification:

- [x] `pnpm typecheck`
- [x] Unit test for the helper's thresholds and rounding (`pnpm test:web`).
- [ ] Manual localnet: change `Config` and watch estimates refresh after the
      TTL.

Files likely touched:

- `apps/web/app/composables/useFeeConfig.ts` (new)
- `apps/web/app/composables/usePaymentProgram.ts`
- `apps/web/app/composables/useBudget.ts`

Dependencies: Task 1.1.

Estimated scope: Medium.

#### Task 1.5: Show the fee and rent split where money is decided

Description: Use `estimateInvoiceCosts` to disclose costs on the three pages
that matter:

- `sell.vue` and `shops/[id].vue`: "You receive X after the Y% fee. The buyer
  pays exactly P (plus the Solana network fee). The permanent payment record is
  paid by the protocol; the temporary invoice rent returns to you automatically."
- `pay.vue`: the price, the network fee, and — for token invoices — the seller
  ATA rent the payer covers on a fresh seller. No fee line for the payer on SOL
  invoices.
- `budget.vue`: the open/refill breakdown reflecting `session_fee_float`.

Acceptance criteria:

- [x] The three pages show the estimate from the shared helper.
- [x] No page claims "every lamport comes back" or shows a fee the payer is not
      paying.
- [x] The token checkout states the payer's SOL costs explicitly.
- [x] Amounts and labels are localized in all five locales (en, fa, fr, de, es).

Verification:

- [x] `pnpm typecheck` and `pnpm lint`
- [ ] Localnet: create an invoice and confirm the note matches the settle
      balances.

**Built so far (2026-11-14):** the buyer checkout lists the Solana network fee
and, on a fresh seller, the token-account rent a token payment would create —
with no fee line on a SOL invoice, because the seller pays it.

The seller side is one component, `InvoiceSplitNote`: "You receive X after the
Y% fee", and that the code's rent comes back on payment while the protocol pays
for the record. It sits on `sell.vue` beside the price and on a shop's page
(`ShopPayment`, over the cart total), so both quote the same policy; the shop's
note is priced in SOL only, because a cart is.

`budget.vue` shows the signature's breakdown under the limit field:
`BudgetOpenCost` lists the pot top-up, the session-key float, the account rent
(only when it has to be made) and the network fee, then the total. The rows come
from `openBudgetCost` / `openTokenBudgetCost`, which read the float from
`Config` for SOL and still from the program's constant for tokens (Phase 3 moves
them). A limit the chain could not price says so rather than showing nothing,
and the quote is debounced while the field is being typed in.

Every page reads `Config` directly; where it is missing they show nothing rather
than a guess.

Files likely touched:

- `apps/web/app/pages/sell.vue`
- `apps/web/app/pages/shops/[id].vue`
- `apps/web/app/pages/pay.vue`
- `apps/web/app/pages/budget.vue`
- `apps/web/i18n/locales/*.json`

Dependencies: Task 1.4.

Estimated scope: Medium.

#### Task 1.6: Fee and treasury tests

**Built so far:** the balance-delta, treasury and fee-mode cases landed with
Tasks 1.1 and 1.2, and the float floor is now pinned as a boundary test
(`MIN_SESSION_FEE_FLOAT` accepted, one lamport less rejected, and the constant
asserted equal to `invoice_rent + receipt_rent`). The API's program scan is
covered too: a 62-byte `Config` account is classified as "nothing to keep"
instead of taking the payment list down, while receipts and budgets still
classify.

Acceptance criteria:

- [x] Direct SOL: payer delta `amount`, seller delta `amount - fee`, treasury
      delta `fee - R_r`, receipt holds `R_r`.
- [x] Budget SOL: pot delta `amount`, `spent += amount`, session key unchanged.
- [x] Fee rounding floors: `amount = 99` at 2% gives `fee = 1`, seller gets 98.
      `fee_floors_rather_than_rounds_at_ninety_nine_lamports` calls `fee_for`
      where it lives, because a 99-lamport price can never settle: the invoice
      rent alone is millions. It also pins 199 → 3 and 100 → 2, so the floor is
      a floor and not a lucky case. Breaking the division to round up fails it
      with `left: 2, right: 1` (checked).
- [x] `fee_bps = 10_000` behaves as documented (`amount - 1` cap; `fee_bps = 0`
      is covered). At the cap the underflow guard reduces to
      `1 > invoice_rent`, so `a_ten_thousand_bps_policy_admits_no_sol_invoice`
      gets `InvoiceTooSmall` on a 0.1 SOL invoice and checks the seller, the
      treasury, the invoice account and the receipt were all left untouched.
      `10_001` is refused by `update_config` (the admin test).
- [x] `withdraw_treasury` cannot empty the treasury; a settle after a withdrawal
      that drops it below `R_r + 890,880` fails with `TreasuryUnderfunded`.
      `withdrawing_the_treasury_to_its_rent_underfunds_the_next_settle` withdraws
      the whole surplus, pins the treasury at exactly its rent, then gets
      `TreasuryUnderfunded` on the next settle — and asserts the invoice and
      receipt the failed transaction had already created are rolled back.
- [x] Passing a wallet instead of the treasury PDA is rejected.

Verification:

- [x] `cargo test --test settle` (what `Anchor.toml` runs for `anchor test`) —
      39 tests, the three above among them (2026-11-18).

Files likely touched:

- `apps/contracts/tests/payvand.ts`

Dependencies: Tasks 1.1–1.3.

Estimated scope: Small.

---

### Phase 2: Budget accounting

Goal: make `spent` mean what left the pot for the invoice, and make refills
behave the way the UI says they do.

#### Task 2.1: Pot pays exactly the invoice amount

Description: With the Phase 1 ledger, the pot pays exactly `amount` and the
session key is made whole for both rents it advanced. Replace the old
`pot >= amount + invoice_rent` check with `pot >= amount` plus the reimbursement
transfers, and drop the old single-rent refund.

Acceptance criteria:

- [x] `pay_from_budget` debits the pot by exactly `amount`.
- [x] The session key is reimbursed exactly `payment_rent + receipt_rent`.
- [x] `budget.spent` increases by exactly `amount`.
- [x] The monthly-limit check and the balance check fail with distinct errors
      (`BudgetExhausted` 6005 is the month, `InsufficientBudget` 6006 the pot).
- [x] The UI's `remainingThisMonth` and `availablePot` agree on what a payment
      costs (`silentPaymentObstacle` asks the limit first, then the pot, and
      names the missing one).

Verification:

- [x] Harness test (the suite this repo runs, not the Anchor one):
      `pay_from_budget_settles_the_invoice_and_records_the_spend` checks the
      pot, seller, treasury and session-key deltas against the 2% policy, and
      `spent == amount`.
- [ ] A payment at the limit succeeds, one lamport over fails with
      `BudgetExhausted` — moved to Task 2.4, which writes it.

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/sol.rs`
- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/web/app/composables/useBudget.ts`
- `apps/contracts/tests/payvand.ts`

Dependencies: Phase 1.

Estimated scope: Medium.

#### Task 2.2: Preserve `spent` on same-month top-ups

Description: Reset `spent` only when `budget.month_index != current_month_index()`;
a same-month top-up raises the pot and limit and keeps the counter.

Acceptance criteria:

- [x] Same-month `open_budget` and `open_token_budget` preserve `spent` and
      update `monthly_limit`.
- [x] A calendar-month change resets `spent` and updates `month_index`.
- [x] `budget.vue` copy no longer says a refill restarts the counter.
- [x] The rotate flow preserves `spent` unless the month changed.

Verification:

- [x] Harness test (not the Anchor suite, which this repo does not run):
      `open_budget_keeps_spent_until_the_month_changes` spends, refills, asserts
      `spent` and `month_index` stand, warps the clock 40 days, and asserts the
      counter and month both start over. Deleting the month comparison turns it
      red.
- [x] Web copy review: `budget.success.refilled` and `budget.success.rotated`
      said the counter restarts; both now say the month's spending keeps
      counting, in all five locales.

**Fixed 2026-11-15:** both handlers set `spent = 0` unconditionally, so a
mid-month refill (the same `init_if_needed` instruction on the same account)
handed back the whole limit — a buyer could refill past it. `open_budget` and
`open_token_budget` now reset the counter only when
`budget.month_index != current_month_index()`, which is also what
`remainingThisMonth` in `useBudget.ts` already assumed. Both paths are covered
by harness tests that run in `cargo test`:
`open_budget_keeps_spent_until_the_month_changes` (spend, refill, then a clock
warp into the next month) and
`open_token_budget_keeps_spent_until_the_month_changes` (the
same rule on the token account; the harness has no SPL runtime, so the counter
is written into the account between opens to stand in for a payment).

The program was upgraded in place on devnet for it (`4k8QbJ4n…`, slot
522521670, same ID and authority). The account needed a 10,240-byte
`solana program extend` first, because the loader only grows data in that step
and the fix added 16 bytes; it now holds 525,312 bytes. The rule was then
checked on chain: budget `fQadYNyE…` opened at 0.05 SOL (`66vW41yu…`), a
`pay_from_budget` of 0.01 SOL recorded `spent` 10,000,000 (`5QMkCEpB…`), and a
refill to 0.08 SOL (`2fS72BEQ…`) left `spent` at 10,000,000, the limit at
80,000,000, the month index unchanged at 24323 and 0.07 SOL spendable. The
budget is still open at `fQadYNyE…` (session key `8nnbn9Wu…`) for inspection.

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/sol.rs`
- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/web/app/pages/budget.vue`
- `apps/web/app/composables/useBudget.ts`
- `apps/contracts/tests/payvand.ts`
- `apps/contracts/programs/payvand/tests/settle.rs`

Dependencies: Task 2.1.

Estimated scope: Small.

#### Task 2.3: Align the UI preflight with the real costs

Description: Make the obstacle checks match the program: `silentPaymentObstacle`
compares `availablePot >= amount` and the session key's SOL against
`payment_rent + receipt_rent + network fee` (both reimbursed, so only the
*peak* matters); `tokenBudgetObstacle` compares it against
`payment_rent + token receipt rent + seller ATA rent + network fee`, using the
sized float. Errors name the missing resource: pot, monthly limit, or session
key SOL.

Acceptance criteria:

- [x] The UI never offers a budget payment the session key cannot fund. The
      client asks the pot against the price, which is exactly what the settle
      spends now that `pay_from_budget` credits the treasury's receipt-rent
      refund before checking it (Task 2.4). That build is on devnet as of
      2026-11-15 and the spend-to-zero check below passed, so the page can no
      longer offer a payment the pot refuses.
- [x] "Remaining" equals what the user can actually spend: `remainingThisMonth`
      subtracts `spent` only in the month it belongs to, and the obstacle asks
      it before the pot.
- [x] Open/refill breakdown reflects `Config::session_fee_float`
      (`openBudgetCost` reads it from the cached policy).
- [x] Error copy distinguishes pot (`nearlyEmpty`), limit (`limitUsedUp`) and
      float (`noSessionKey`).

Verification:

- [x] `pnpm typecheck` and `pnpm lint`
- [ ] Localnet: drain the float and confirm the UI warns before submission. Not
      run: the preview browser cannot sign, and the devnet budget smoke only
      covered a funded float.

Files likely touched:

- `apps/web/app/composables/useBudget.ts`
- `apps/web/app/pages/budget.vue`

Dependencies: Tasks 2.1–2.2.

Estimated scope: Medium.

#### Task 2.4: Budget regression tests

Acceptance criteria:

- [x] Payment at the limit succeeds; one lamport over fails.
      `pay_from_budget_stops_at_the_monthly_limit`: two 0.05 SOL invoices reach
      a 0.10 SOL limit and settle leaving the pot at the budget's own rent, the
      third is refused with `BudgetExhausted` (6005), and the counter stays at
      the limit. The invoices are whole amounts because the smallest payable
      invoice is far above one lamport. Removing the month check turns the test
      red: the payment then fails with `InsufficientBudget` (6006).
- [x] Same-month refill preserves `spent`; a month change resets it.
      `open_budget_keeps_spent_until_the_month_changes` (Task 2.2).
- [x] Pot and vault balances match the documented deltas — the pot in
      `pay_from_budget_settles_the_invoice_and_records_the_spend`
      (`pot_before - pot_after == amount`) and in the devnet smoke test. **The
      vault is still untested**: the token path needs an SPL runtime the harness
      does not carry (Task 3.4).
- [x] A compromised session key cannot exceed the pot balance:
      `pay_from_budget_cannot_spend_more_than_the_pot` leaves the pot holding
      half a price inside a four-invoice limit and gets `InsufficientBudget`
      (6006), with the counter untouched.
- [x] Seller proceeds and treasury fee match Phase 1 for budget payments (the
      same test asserts both deltas).

Verification:

- [x] `cargo test --manifest-path apps/contracts/Cargo.toml` — 25 tests. The
      Anchor suite (`anchor test`) still needs a Devnet cluster and is not run
      in this repo.

**Follow-up found while writing these, and fixed the same day (2026-11-15):**
the pot check ran before `pay_from_treasury` refunds the receipt rent, so a
payment transiently needed `amount + receipt_rent` in the pot although it only
spends `amount` — the last receipt rent of every limit was unspendable, and the
client's preflight (which compares the pot against the price) would wave it
through only to have it refused. The program now credits that refund in the
check (`pot + receipt_rent >= outflow`), which is what the transfer order has
always done; the client needed no change. The token path never had the problem:
it refunds the rents out of its own float, capped at what is there.

`pay_from_budget_stops_at_the_monthly_limit` now runs with the pot holding
exactly the limit and no slack: the invoice that reaches the limit settles and
leaves the pot at the budget's own rent, i.e. the limit is spendable to its last
lamport. Restoring the old check turns that test red with
`InsufficientBudget` (6006).

The program was upgraded in place for it the same day (`29G84AqdfAT…`, slot
522529255, same ID and authority, 525,312 bytes — the earlier extend left
enough room, so no second `extend`), and the rule was checked on devnet with a
budget holding exactly its limit: buyer `FvFg6DTo…`, session `FBUG9i9Q…`, budget
`CZJD9Wz3…` opened at 0.02 SOL (`Z1z5z1bL…`, pot 20,000,000), one 0.01 SOL
payment (`5riCMoX3…`, pot 10,000,000, `spent` 10,000,000), a second
(`5GHSgYo2…`) that left **pot 0** — the account at exactly its 1,143,000-lamport
rent, `spent` 20,000,000 — and a third refused with `BudgetExhausted` (6005).
Under the old check that second payment fails `InsufficientBudget`, which is
what the local teeth check showed.

Files likely touched:

- `apps/contracts/tests/payvand.ts`
- `apps/contracts/programs/payvand/tests/settle.rs`

Dependencies: Tasks 2.1–2.3.

Estimated scope: Small.

---

### Phase 3: Token payments and float safety

Goal: pay token invoices without the seller holding SOL, survive fresh seller
ATAs, and stop wrong-mint or decoy-vault closes from stranding funds.

#### Task 3.1: Token fee split and treasury ATA

Description: Split the token amount `amount - fee_t` to the seller's ATA and
`fee_t` to the treasury's ATA for that mint, creating both when absent. The
treasury funds its own ATA rent via `invoke_signed`
(`create_associated_token_account` signed by the treasury seeds) so the payer
does not pay for protocol infrastructure. The treasury refunds the token receipt
rent, and the payment account closes to whoever advanced its rent.

Acceptance criteria:

- [x] Seller receives `amount - fee_t`; treasury ATA receives `fee_t` — on the
      payer path, executing: `settle_token_payment_splits_the_price_between_seller_and_treasury`
      pays 700,000 and checks 686,000 into the seller's account and 14,000 into
      the treasury's account for the mint. `pay_from_token_budget` carries no
      treasury account in its interface and moves the whole amount, so this
      holds for one path, not both (revision 6). Revision 12 makes it hold on
      both, by giving `pay_from_token_budget` the treasury accounts and the same
      split; Task 3.5.
- [x] `fee_t = min(amount * fee_bps / 10_000, amount - 1)`;
      `require!(amount > fee_t)` so a seller never receives zero. Exercised at
      the cap (2026-11-18):
      `a_ten_thousand_bps_policy_leaves_the_token_seller_one_base_unit` sets
      `fee_bps = 10_000` through `update_config`, pays a 700,000-unit invoice,
      and checks one base unit reaches the seller while 699,999 reach the
      treasury's account. The guard's failing side is unreachable by design —
      the cap keeps `fee_t <= amount - 1` — so that is as close as the program
      can be driven to it.
- [ ] A fresh seller ATA is created and paid for by the payer; a fresh treasury
      ATA is created and paid for by the treasury. Neither holds as written: the
      payer path requires the seller's account to exist already (the seller
      opens it through `/sell`), the budget path creates a fresh one and the
      session float pays its rent, and the treasury's account is opened by the
      admin through `open_treasury_token_account` (2026-11-15 note above). The
      behaviour that does exist is covered under Task 3.4.
- [ ] Zero-value transfers are skipped rather than sent: the handler skips a
      zero fee, but nothing pins it.
- [x] The treasury refunds `R_tr` to the payer or session key: the executed
      settle ends with the pool exactly one receipt rent down — the refund out,
      the invoice rent back in.
- [x] The seller needs no SOL before or after the payment: both token tests pay
      a seller that holds no lamports and assert it stays at zero until the
      auto-close hands it the invoice rent. Revision 12 moves that close to the
      pool, so the seller stays at zero for good; not built.

Verification:

- [x] Rust harness, not Anchor: a seller with no token account and no SOL
      receives its tokens on the budget path, its account created and paid for
      by the float
      (`the_float_carries_two_payments_to_fresh_sellers_then_runs_out`).
- [ ] Treasury ATA created on first use of a mint; second payment reuses it.
      Proved on devnet (2026-11-15) and by `open_treasury_token_account` being
      idempotent; the harness opens it once, so reuse is not tested.
- [ ] The payer's SOL delta is the seller ATA rent (when new) plus the network
      fee only. Not asserted: the payer path never creates the seller's account,
      and the transaction's fee payer in the harness is the bank's own key.

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/contracts/programs/payvand/src/constants.rs`
- `apps/contracts/tests/payvand.ts`

Dependencies: Phase 1.

Estimated scope: Large.

#### Task 3.2: Size and manage the session-key float

Description: Size the token floats so the float itself carries three consecutive
payments to sellers that hold no SOL — not three with a top-up in the middle —
and keep the explicit top-up path for what lies beyond that. The task originally
named the means as `Config::session_fee_float` instead of the hardcoded
`SESSION_FEE_FLOAT` and the removal of `TOKEN_BUDGET_FEE_FLOAT`; revision 8
records why the sizing was done instead, and those two stay deferred.

Acceptance criteria:

- [x] `open_budget` tops the session key up to `Config::session_fee_float` and
      never takes more than the gap. `open_token_budget` does the same against
      the constants below: the gap between `rent + TOKEN_BUDGET_FEE_FLOAT` and
      what the budget holds, and between `SESSION_FEE_FLOAT` and what the session
      key holds. Reading either from `Config` is deferred (revision 8).
- [x] The token floats are sized together for three payments rather than merged
      into one `Config` float: `SESSION_FEE_FLOAT` is 12,000,000 and
      `TOKEN_BUDGET_FEE_FLOAT` 18,000,000, which is the 0.03 SOL this plan
      budgets for a session float. Neither call takes more than the gap.
- [x] The buyer can top up the float independently. The floats are lamports the
      program reads live, so a refill is a plain system transfer from the buyer —
      no limit change, no token movement — and the budget returns what is left on
      close. `tokenFloatTopUpInstructions` in `useBudget.ts` sends only the
      difference before a silent token payment that the float could not fund.
- [x] The client's open/refill cost estimate matches the program.
      `openTokenBudgetCost` reports a refill's refund float as the gap up to
      `rent + TOKEN_BUDGET_FEE_FLOAT` (`Math.max(0, …)` over the account's
      lamports) rather than 0, which is what `open_token_budget` transfers.
- [x] Three consecutive token payments to fresh sellers do not exhaust the float.
      Measured (2026-11-16, revision 8): at the harness's 6,960 lamports a byte
      each payment advances 7,744,520 and is refunded 5,700,240, so the third
      starts with 11,833,080 to front against the 12,000,000 session float, and
      the budget's 5,700,240 refund for it sits inside the 6,599,520 its float
      has left. The three complete, leaving 5,867,160 on the session key and
      899,280 of the budget float; a fourth is refused for want of lamports
      (`Custom(1)`) with the counter and the vault unchanged.

Verification:

- [x] Rust harness: `the_float_carries_three_payments_to_fresh_sellers` pays three
      sellers that hold neither SOL nor a token, checks each received the price
      and the invoice rent, pins the float arithmetic exactly — the session key
      down 3 × (account + fee), the budget down 3 × (invoice + receipt) — then
      refuses a fourth with `Custom(1)` and asserts the counter and the vault did
      not move. At the old 10,000,000/10,000,000 sizing the same test fails on the
      third payment (checked by restoring it), so it pins the numbers rather than
      describing them. **Revision 12 changes what it measures**, not that it
      measures it: with the pool funding the records and taking the closed
      invoice, the seller receives the price alone and the budget pays the price
      alone, so the invoice rent disappears from both sides of this test and from
      the 18,000,000 it sized. Re-measure in Task 3.5.
- [x] Rust harness: `a_top_up_lets_the_float_carry_more_fresh_sellers` starts at
      that boundary: three payments fit, the fourth is refused, one top-up pays
      it, and a second top-up pays a fifth, so the refill is a rule and not a
      one-off. The top-up amounts are computed from live balances, the way the
      client does.
- [x] Rust harness fidelity: allowance payments are sent with the session key as
      fee payer (`send_with_fee_payer`), as the client sends them. Before that the
      harness's default payer covered the 5,000-a-signature fee, so the float's
      measured cost was short by 5,000 a payment.
- [x] The harness's refill rule is checked against the client's.
      `top_up_token_floats` models what `tokenFloatNeed` + `tokenFloatTopUp` in
      `apps/web/app/composables/useBudget.ts` do, and a harness cannot call an app,
      so the copy is pinned rather than trusted: both suites read
      `tests/fixtures/token-float-top-up.txt` — `the_float_top_up_rule_matches_the_shared_vectors`
      in `tests/settle.rs`, and the same rows in `useBudget.test.ts` — so editing
      the rule on either side, or a row itself, fails on both.
- [x] Anchor test: float top-up without a limit change.
      `a_float_top_up_needs_no_limit_change` spends the float to the boundary the
      two tests above measure (three payments, a fourth refused), refills it with
      the buyer's plain transfers alone, and asserts the limit, `spent`,
      `month_index`, the vault and the buyer's token balance are exactly what
      they were while each float holds one more payment's worth — then pays the
      refused seller and checks the counter follows. It fails when the refill's
      session amount leaves out the payment fee (checked, 7,739,520 against
      7,744,520), so it pins the amounts rather than only the success.
      `Anchor.toml` runs `cargo test` for `anchor test`, so this is the same
      suite as the rest of the file.
- [x] `nuxt typecheck` clean (`./node_modules/.bin/nuxt typecheck`; the global
      `pnpm` shim on this machine is broken).

What revision 8 decided, and what it left: the token flows still read the
constants instead of `Config::session_fee_float`, and `TOKEN_BUDGET_FEE_FLOAT`
still exists. Both were the task's suggested means, not its criterion; the
criterion is that the float carries three payments, and the constants now say so
at mainnet rent. Merging them into one `Config` float is left as a follow-up
because it changes what `session_fee_float` means for every existing SOL budget
and needs a devnet migration and a client re-quote, not because it is cheaper —
revision 8 has the arithmetic.

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/contracts/programs/payvand/src/constants.rs`
- `apps/contracts/programs/payvand/src/instructions/config.rs`
- `apps/web/app/composables/useBudget.ts`
- `apps/web/app/pages/budget.vue`
- `apps/contracts/tests/payvand.ts`

Dependencies: Tasks 3.1, 1.1.

Estimated scope: Medium.

#### Task 3.3: Mint and vault constraints

Description: Add `require_keys_eq!(mint.key(), budget.mint, MintMismatch)` to the
token close/pay paths, and constrain the vault to the canonical ATA
(`associated_token::mint = mint`, `associated_token::authority = budget`) in
`pay_from_token_budget` and `close_token_budget`, matching
`open_token_budget`.

Acceptance criteria:

- [x] A wrong-mint close fails with `MintMismatch` and strands nothing:
      `closing_a_token_budget_against_another_mint_fails_with_mint_mismatch`
      gets `Custom(6008)` and the stand-in vault keeps its 400,000 tokens.
- [x] A decoy token account for the same mint and authority is rejected:
      `a_decoy_vault_with_the_same_mint_and_authority_is_rejected` gets Anchor's
      canonical-ATA check (`ConstraintRaw`, `Custom(2009)`) and neither the decoy
      nor the real vault moves.
- [x] The canonical vault is the only accepted vault; its rent returns to the
      buyer on close:
      `closing_a_token_budget_drains_the_canonical_vault_and_returns_its_rent`
      checks the residual 300,000 tokens are in the buyer's account, the vault
      and budget accounts are gone, and the buyer got the vault's rent plus
      everything the budget still held.

Verification:

- [x] Rust harness: wrong-mint close fails; decoy vault rejected (2026-11-16).
- [x] Rust harness: canonical vault drained and closed, rents back to the buyer.

Built (2026-11-16): the program half of this task landed with the tests. Both
paths now use `associated_token::mint`/`authority` for the vault, and
`close_token_budget` also requires the mint argument to be the budget's own.

Run on devnet (2026-11-17): the harness proves the built artifact, not the
deployed one, so `apps/web/scripts/devnet-token-guard-smoke.mjs`
(`pnpm smoke:guards`) repeats both refusals against program
`4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J`. It opened a token budget on a
throwaway mint (500,000 limit, `3FMX5rV3…`), closed it with another mint's
canonical vault — every ATA constraint satisfied, so only the mint check can
refuse — got `Custom(6008)` (`2J5T2gqf…`), then built a token account with the
vault's mint and authority at an address that is not the vault (`JD6T8b2e…`)
and had both `pay_from_token_budget` (`458yh73z…`) and `close_token_budget`
(`21ek1M93…`) refused with `Custom(2009)`, Anchor's canonical-ATA check. The
real vault still held 500,000, the decoy and the seller held 0 and the budget
was still open, so nothing was stranded; the run then closed it (`fSrvNdKA…`).

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/contracts/tests/payvand.ts`

Dependencies: Task 3.2.

Estimated scope: Small.

#### Task 3.4: Token regression tests

Acceptance criteria:

- [x] Wrong-mint close fails; decoy vault rejected (2026-11-16): `Custom(6008)`
      for the mint, `Custom(2009)` for the decoy, with no token moved.
- [x] Fresh seller: ATA created, the float pays its rent, the seller receives
      its tokens. On the budget path, which carries no fee, the seller gets the
      whole `amount`; the payer path's `amount - fee_t` is Task 3.1's test.
- [x] Treasury ATA receives `fee_t` and its rent is not charged to the payer:
      the payer's token account moves by the price alone (1,000,000 → 300,000)
      while the treasury's account gains 14,000 and the admin paid its rent.
- [ ] Payment account closed to the rent payer, not the seller. Decided
      2026-11-18 (revision 12): the criterion stands and the handler changes.
      The invoice closes to the pool that funds it, the seller keeps
      `amount - fee_t` alone, and the pot pays exactly `amount`. Not built: the
      work is Task 3.5.
      Today `pay_from_token_budget` refunds both rents to the session key and
      closes the invoice to the seller, so a fresh seller ends up holding the
      invoice rent (2,714,400 at the bank's rent).
- [x] A settled token invoice cannot be replayed (2026-11-16):
      `settle_token_payment_replay_is_rejected_as_already_settled` settles an
      invoice, then sends the same one again with a fresh blockhash and gets
      `AlreadySettled` (6002). The seller's, the treasury's and the buyer's token
      balances are unchanged, no second invoice account is left behind, and the
      receipt still carries the first settle's `settled_at`, `payer` and `amount`.
      Removing the receipt's guard lets the replay through, so the test pins the
      guard rather than the shape of the accounts.
- [x] Token monthly limit enforced and preserved across a same-month top-up:
      `open_token_budget_keeps_spent_until_the_month_changes` (2026-11-15), which
      writes the counter into the budget account between opens. Since
      2026-11-16 the settle side runs too: two paid invoices from one budget are
      counted in
      `the_float_carries_two_payments_to_fresh_sellers_then_runs_out`.

Verification:

- [x] `cargo test --manifest-path apps/contracts/Cargo.toml` — 33 tests, the
      token ones executing against the deployed SPL programs; `cargo fmt --all
      --check` and `cargo clippy -p payvand --all-targets -- -D warnings` clean.

Files likely touched:

- `apps/contracts/tests/payvand.ts`

Dependencies: Tasks 3.1–3.3.

Estimated scope: Small.

#### Task 3.5: The budget path's fee, its pool-funded rents, and the close to the treasury

Description: This is the program change revision 12 decided when it settled Task
3.4's criterion. Today `pay_from_token_budget` has no treasury in its interface,
moves the whole amount to the seller, refunds both rents out of the buyer's pot
and closes the invoice to the seller, so a fresh seller leaves with `amount`
plus the invoice rent (2,714,400 at the bank's rent) of the buyer's lamports. It
becomes the shape `settle_token_payment` already has: the price pays the fee, the
pool funds the records and takes the closed invoice, and the seller receives the
price less the fee. Nothing about the payer path or `settle_token_payment`
changes.

Accounts, in this order — the client builder, the harness helper and the IDL all
follow it:

1. `session_key` — signer, mutable: fee payer and rent payer, refunded in the
   same transaction.
2. `budget` — mutable, `seeds = [TOKEN_BUDGET_SEED, budget.buyer, budget.mint]`,
   `bump = budget.bump`.
3. `vault` — mutable, the canonical ATA for the budget and mint
   (`associated_token::mint`/`authority`), as `open_token_budget` already checks.
4. `mint` — read-only.
5. `seller` — mutable, unchecked, and no longer the close destination.
6. `seller_token_account` — `init_if_needed`, paid by the session key, the
   seller's ATA for the mint.
7. `payment` — `init`, paid by the session key, same seeds as today.
8. `receipt` — `init_if_needed`, paid by the session key.
9. `reference` — unchecked.
10. `config` — read-only, `seeds = [CONFIG_SEED]`, `bump = config.bump`.
11. `treasury` — mutable, `seeds = [TREASURY_SEED]`, `bump = config.treasury_bump`.
12. `treasury_token_account` — mutable, `token::mint = mint`,
    `token::authority = treasury`; it must already exist (the admin opens it with
    `open_treasury_token_account`), so no mint becomes unpurchasable by a buyer
    signature.
13. `token_program`.
14. `associated_token_program`.
15. `system_program`.

Handler, in this order:

1. The guards that exist today: `amount > 0`, memos, a real seller, the session
   key matching the budget, the mint matching the budget, the receipt unsettled,
   the monthly cap, and `vault.amount >= amount`.
2. `fee_t = fee_for(&config, amount)`; `require!(amount > fee_t)` holds by the
   `amount - 1` cap.
3. `invoice_rent` and `receipt_rent` from `Rent::get()`, then
   `ensure_treasury_funded(&treasury, invoice_rent + receipt_rent)` — before any
   transfer, so a pool that cannot refund cannot start a settlement.
4. Vault → seller `amount - fee_t`, signed by the budget seeds.
5. Vault → `treasury_token_account` `fee_t`, skipped when it is zero.
6. `pay_from_treasury(treasury → session_key, invoice_rent + receipt_rent)` — a
   CPI, so it runs before any direct lamport move in this frame.
7. Write `payment` and `receipt`, bump `budget.spent` by `amount`.
8. `payment.close(treasury)` — the invoice rent returns to the pool that funded
   it, and the seller keeps exactly `amount - fee_t`.
9. `msg!` with the split, the session key and the mint, as the other paths do.

What falls out of it, and has to move with it:

- **The budget no longer holds a refund float.** The pool refunds the session
  key, so `open_token_budget`'s `rent + TOKEN_BUDGET_FEE_FLOAT` top-up has nothing
  to do: `TOKEN_BUDGET_FEE_FLOAT` goes, and the budget is funded to its own rent
  only. `openTokenBudgetCost`, `tokenFloatNeed`, `tokenFloatTopUp` and the
  vectors both suites read (`tests/fixtures/token-float-top-up.txt`, with
  `useBudget.test.ts` on the client side) follow. `SESSION_FEE_FLOAT` still has
  to cover the rents the session key fronts inside one transaction plus a fresh
  seller's ATA and the network fee; re-measure three fresh sellers and set it
  from the printed numbers rather than from today's 12,000,000.
- **The IDL and the client builder.** `anchor idl build` (or `anchor build`)
  regenerates `target/idl/payvand.json`; copy it to
  `apps/web/app/assets/payvand.idl.json`. `sendSilentTokenPayment` in
  `useBudget.ts` passes the three new accounts, derived the way the SOL path
  already derives `config` and `treasury` and the way the payer path derives the
  treasury's ATA. The program still has to be built with `cargo build-sbf` for
  the harness, which runs the SBF artifact — not `anchor build`'s eBPF target.
- **`lib.rs`'s entry-point docs** describe the old refund and the budget's float
  and need the new shape.
- **The devnet guard smoke test** (`apps/web/scripts/devnet-token-guard-smoke.mjs`)
  builds `pay_from_token_budget` by hand, so it passes the new accounts too, or
  the decoy-vault refusal it repeats stops being the refusal the program makes.

Acceptance criteria:

- [ ] `pay_from_token_budget` takes the fifteen accounts above in that order, and
      the copied IDL names them.
- [ ] Seller receives `amount - fee_t` tokens and nothing else; the treasury's
      ATA for the mint receives `fee_t`; the buyer's vault falls by `amount`.
- [ ] The pot pays exactly `amount`: the budget's lamport balance falls by exactly
      `amount`, `spent` rises by `amount`, and the session key ends the payment
      where it started (fronts the rents, is refunded, pays only the ATA rent and
      the network fee).
- [ ] The invoice closes to the treasury: the seller holds no lamports before or
      after, and the pool ends one receipt rent down per payment.
- [ ] A treasury holding one rent but not both refuses with
      `TreasuryUnderfunded` before any token moves.
- [ ] A substituted `config` or `treasury` is refused by its seeds constraint.
- [ ] `fee_bps = 10_000` on this path leaves the seller one base unit, matching
      the payer path's cap test.
- [ ] `useBudget.ts`'s silent payment sends the same accounts and settles on
      devnet from a real budget.
- [ ] The floats and the shared vectors are re-measured and updated on both
      sides, and `TOKEN_BUDGET_FEE_FLOAT` is gone from the program, the client and
      the harness.
- [ ] Task 3.4's criterion is ticked, with the invoice rent no longer reaching the
      seller.

Verification:

- [ ] `cargo build-sbf --manifest-path programs/payvand/Cargo.toml`, then
      `cargo test --manifest-path apps/contracts/programs/payvand/Cargo.toml
      --test settle` — the token tests execute the rebuilt artifact, and the
      five that build this instruction
      (`the_float_carries_three_payments_to_fresh_sellers`,
      `a_top_up_lets_the_float_carry_more_fresh_sellers`,
      `the_float_top_up_rule_matches_the_shared_vectors`,
      `a_decoy_vault_with_the_same_mint_and_authority_is_rejected`,
      `closing_a_token_budget_against_another_mint_fails_with_mint_mismatch`)
      pass on the new accounts. The assertion at `settle.rs:2734` that leaves a
      seller holding `token_invoice_rent()` is inverted: the seller's lamports
      stay at zero.
- [ ] `cargo fmt --all --check` and
      `cargo clippy -p payvand --all-targets -- -D warnings`.
- [ ] `vitest run`, `nuxt typecheck`, `eslint .` in `apps/web`.
- [ ] Devnet: `pnpm smoke:guards` (with the new accounts) and one real silent
      token payment through a budget, with the treasury's ATA gaining `fee_t`
      and the seller's wallet holding no lamports.
- [ ] A revision note carries the measured float numbers and the new per-payment
      pool position (`fee_t - 2,985,840`).

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/contracts/programs/payvand/src/constants.rs`
- `apps/contracts/programs/payvand/src/lib.rs`
- `apps/contracts/programs/payvand/tests/settle.rs`
- `apps/contracts/programs/payvand/tests/fixtures/token-float-top-up.txt`
- `apps/web/app/assets/payvand.idl.json`
- `apps/web/app/composables/useBudget.ts`, `useBudget.test.ts`
- `apps/web/scripts/devnet-token-guard-smoke.mjs`

Dependencies: Tasks 3.1–3.4, and the devnet program upgrade in Task 5.1.

Estimated scope: Large.

---

### Phase 4: Session-key lifecycle, events, and cleanup

Goal: stop losing the float, make the program observable, and close the
remaining low-severity findings.

#### Task 4.1: Sweep the session key before forgetting or rotating

Description: The program cannot move lamports out of the session key (it is not
a PDA), so the client signs `system_program::transfer` from the session key to
the buyer for `balance - fee` before `forgetSessionSigner` and before rotating.
If the sweep fails, do not forget the key; show the balance and a retry.

Acceptance criteria:

- [x] Closing the last budget sweeps the float back before the key is forgotten.
      `handBackFloat` in `budget.vue` sweeps, and only then forgets; a failed
      sweep keeps the key, since a forgotten key cannot sign the transfer that
      recovers it.
- [x] Rotating leaves no float of ours behind. As built, rotation always
      installs *this* device's key (`ensureSessionSigner`), so the key it
      replaces belongs to another device and is not ours to sign for; that
      device gets the recover action below. Reworded from "rotating sweeps the
      old key when this device holds it", which no code path can reach.
      Checked rather than assumed (2026-11-15): rotation should **not** mint a
      fresh key. A fresh key would strand the float this device already holds
      and make `open_budget` buy a second one, and the key it replaces lives on
      the other device either way, so nothing is made sweepable by it.
      `useSessionKey.test.ts` pins that this device answers with the same key
      every time and that only forgetting drops it; generating one per call
      turns that test red, which is the prompt to add the sweep then.
- [x] A rotated-out device detects it is no longer the budget's `session_key`
      and offers to sweep: `refreshRecoverableFloat` compares this device's key
      with every live budget's `session_key` and surfaces the balance — only
      after a read that succeeded (`budgetsRead`), since a failed read leaves
      the budgets null and is no evidence that nothing needs the key. A sweep
      whose confirmation poll times out re-reads the key before failing: the
      balance decides, so a float that already moved is not reported as stuck.
- [x] The success message only claims "everything came back" when the sweep
      succeeded: `budget.success.closed` when it did, `closedUnswept` (which
      names the leftover and points at the recover action) when it did not.
- [x] A non-zero balance shows a visible recover action: the `budget.recover`
      alert with the amount and a "Recover … SOL" button, shown whether or not
      an allowance is left on the page.

Verification:

- [x] Devnet, with the real program: `open_budget` 0.02 SOL left the session key
      `WxGBYkcQ…` holding 30,000,000 lamports, `close_budget` returned the pot
      and left that float untouched (the loss this task is about), and the sweep
      the client builds — a system transfer for `balance - 5,000`, signed by the
      session key as its own fee payer — moved 29,995,000 to the buyer
      (`4WVsHfKK…`) and left the key at **0**, with nothing left to sweep a
      second time. The buyer paid nothing for it, so the wallet only had to
      approve the close.
- [x] Rotation, as above: nothing of ours is replaced, so there is no old
      balance to sweep from this device; the rotated-out device's balance is
      what the recover action covers.
- Not checked: the browser path itself (button → sweep → forgotten key) needs a
      wallet and an extension, so the wiring is type-checked and the two pure
      pieces are unit-tested (`useBudget.test.ts`: the amount and the transfer's
      instruction index and little-endian lamports), not clicked through.

Files touched:

- `apps/web/app/composables/useBudget.ts`: `sweepableLamports`,
  `transferLamportsInstruction`, `sweepableFloat`, `sweepSessionFloat`.
- `apps/web/app/pages/budget.vue`: `handBackFloat`, `recoverFloat`,
  `refreshRecoverableFloat` and the recover alert.
- `apps/web/i18n/locales/*.json`: `budget.recover.*`,
  `budget.success.closedUnswept` / `.floatRecovered`, and
  `budget.errors.floatNotRecovered`, in all five locales.
- `apps/web/app/composables/useBudget.test.ts` (new) and
  `useSessionKey.test.ts` (new, pins the rotation invariant above).

Files likely touched:

- `apps/web/app/composables/useSessionKey.ts`
- `apps/web/app/composables/useBudget.ts`
- `apps/web/app/pages/budget.vue`

Dependencies: Phase 2.

Estimated scope: Medium.

#### Task 4.2: Correct the fee and rent documentation

Description: Fix claims that the new model makes false:
`lib.rs` "the buyer gets every lamport back", `lib.rs` "every token and lamport
comes back", `budget.vue` "everything left went back to your wallet", and the
fee/rent note on `sell.vue` / `shops/[id].vue` / `pay.vue`. Document: payer pays
the price; seller bears `fee_bps`; the treasury pays the permanent receipt rent;
temporary rent returns automatically; token fees are deducted in the token, so
sellers need no SOL.

Acceptance criteria:

- [x] Doc comments match behavior (2026-11-15). `lib.rs` no longer promises that
      `close_budget` returns "every lamport" or that `close_token_budget`
      returns "every token and lamport": they return the pot, or the vault with
      both rents and the float the budget kept, and the session key's float is
      swept by the client because a PDA cannot move lamports off a keypair. The
      close comment in `budget.vue` says which of the two it is doing.
- [x] UI success messages are accurate. `budget.success.closed` says "everything
      left *in it*", since a session key that still pays another currency's
      allowance keeps its float on purpose (and a failed sweep already speaks
      through `closedUnswept`/`floatUnsweptBody`, Task 4.1). `invoiceSplit.
      rentNote` was true only for SOL — a token invoice refunds the payer and
      closes to the treasury, so the seller gets no rent back — and now has a
      `rentNoteToken` sibling that `InvoiceSplitNote` picks by currency. The
      terms page stopped claiming Payvand charges nothing and never receives
      funds: the seller pays `fee_bps`, the protocol funds the permanent record,
      and that fee is the only money that passes through.
- [x] `README.md` documents the fee model, the treasury and the break-even
      economics: a "Fees, rent and the treasury" section with who pays and who
      receives, the SOL ledger step by step, the token variant (sellers need no
      SOL, the treasury needs its account per mint), the budget variant,
      break-even at `fee_bps × amount ≥ receipt rent` (≈0.109 SOL at 2% and
      today's rent), the subsidy below it, `TreasuryUnderfunded`, and the
      `fixed_fee_lamports` trade-off. The old closing note that had
      `settle_payment` moving lamports "straight from the payer to the seller"
      now describes the fee split and the treasury-funded receipt.
- [x] `DEV.md` documents `initialize_config`, `update_config`,
      `withdraw_treasury` and the treasury funding step: the live `Config`
      values with their addresses, the seeds and who may sign, the limits
      (`fee_bps ≤ 10_000`, `session_fee_float ≥ 5_477_520`), the plain transfer
      that funds the treasury, `pnpm treasury:open --mint` for a new token, and
      the account layout for reading `Config` by hand.

Verification:

- [x] Doc review (2026-11-15): every claim was checked against the handlers —
      `settle_payment` splits the transfer, closes the invoice to the seller and
      refunds the payer out of the treasury; `close_token_budget` drains the
      vault, closes it to the buyer and refunds the budget's float;
      `validate_fee_policy` and `withdraw_treasury` enforce the limits now
      documented. `cargo fmt --check`, `cargo clippy -D warnings`,
      `pnpm typecheck`, `eslint` and the 40 web tests all pass, and the five
      locale files carry the same keys.
- [ ] Manual close/rotate and invoice flows in a browser: not run — there is no
      wallet or extension in this environment. The ledgers these docs describe
      are the ones exercised on devnet under Tasks 2.2, 2.3, 4.1 and 5.2.

Files likely touched:

- `apps/contracts/programs/payvand/src/lib.rs`
- `apps/web/app/pages/budget.vue`, `sell.vue`, `shops/[id].vue`, `pay.vue`
- `README.md`
- `DEV.md`

Dependencies: Task 4.1.

Estimated scope: Small.

#### Task 4.3: Program events

**Status (2026-11-17): not started.** The program logs with `msg!` only — no
`#[event]`, no `emit!`, and the generated IDL has no `events` section.

Description: Emit events for every state change, with the fee/rent split so an
indexer can reconstruct proceeds without re-deriving `Config`. This task owns
the settle events that Tasks 1.2 and 1.3 deliberately left as `msg!` logs, so
the fee math could land and be verified before any event surface was added:

- `PaymentSettled` / `TokenPaymentSettled` — amount, fee, seller proceeds,
  receipt rent, rent returned, payer, receipt, mint (token), timestamp.
- `ConfigUpdated`, `BudgetOpened`, `BudgetClosed`, `TokenBudgetOpened`,
  `TokenBudgetClosed`, `TreasuryWithdrawn`.

Acceptance criteria:

- [ ] Events exist for each state change and are in the IDL.
- [ ] Settle events carry the split.
- [ ] No event describes a close path; none exists.
- [ ] `msg!` logs remain for human-readable explorers.
- [ ] The settle events replace the interim logs from Tasks 1.2 and 1.3, with
      no duplicate emission.

Verification:

- [ ] Anchor test asserts event payloads, including the `SellerPays` and
      `BuyerPays` splits.
- [ ] `anchor build` regenerates the IDL with the events.

Files likely touched:

- `apps/contracts/programs/payvand/src/events.rs` (new)
- `apps/contracts/programs/payvand/src/instructions/*.rs`
- `apps/contracts/programs/payvand/src/lib.rs`
- `apps/web/app/assets/payvand.idl.json`
- `apps/contracts/tests/payvand.ts`

Dependencies: Tasks 1.2, 1.3, and Phase 3.

Estimated scope: Medium.

#### Task 4.4: Token-2022 decision and `Config` layout test

Description: Document that the program uses `anchor_spl::token` and rejects
Token-2022, link a follow-up plan for `token_interface` migration, and make sure
the UI/API do not offer Token-2022 mints. Add an `InitSpace` (or size-assert)
test for `Config` and the budget accounts; the receipt and payment layouts keep
their `0001` tests.

Acceptance criteria:

- [ ] Product docs state supported token programs and link the follow-up.
- [ ] A Token-2022 mint fails with a clear error.
- [ ] `Config`, `Budget`, and `TokenBudget` sizes are asserted by a test.

Verification:

- [ ] Doc review.
- [ ] `cargo test -p payvand`

Files likely touched:

- `README.md`
- `DEV.md`
- `apps/contracts/programs/payvand/src/state.rs`
- `apps/contracts/programs/payvand/src/constants.rs`

Dependencies: Phase 1.

Estimated scope: Small.

#### Task 4.5: Remove dead code

Acceptance criteria:

- [x] No unused error variants (`FeeTooLow` is gone with the rule that used it).
- [x] No stale comments describing the old payer-funded receipt rent, the
      seller-closed token rent, or a reclaim flow (searched for all three;
      nothing left).
- [x] `cargo clippy -p payvand -- -D warnings` is clean.

Verification:

- [x] `cargo clippy --all-targets -- -D warnings` (2026-11-15, clean apart from
      a future-incompat note in a third-party proc-macro crate).
- [x] `cargo check -p payvand`

Files likely touched:

- `apps/contracts/programs/payvand/src/errors.rs`
- `apps/contracts/programs/payvand/src/instructions/*.rs`
- `apps/contracts/programs/payvand/src/lib.rs`

Dependencies: Phases 1–4.

Estimated scope: Small.

---

### Phase 5: Verification, rollout, and archival

#### Task 5.1: Full verification

Acceptance criteria:

- [ ] `task verify` passes, including `anchor test`.
- [ ] `cargo clippy -p payvand -- -D warnings` passes.
- [ ] The IDL is regenerated and copied to
      `apps/web/app/assets/payvand.idl.json`.
- [ ] API tests cover the receipt rent and fee fields.
- [ ] `estimateInvoiceCosts` is tested against the program's math, including the
      minimum amount and the break-even point.
- [ ] Localnet smoke test of all four settlement paths.

Verification:

- [ ] `task verify`
- [ ] `cd apps/contracts && anchor test`

Files likely touched:

- `apps/web/app/assets/payvand.idl.json`
- Files changed in Phases 1–4.

Dependencies: Phases 1–4.

Estimated scope: Medium.

#### Task 5.2: Devnet rollout

Description: The `Config` account does not exist until after the program is
upgraded, so any settle between those two steps fails with `ConfigMissing`.
Run the upgrade and `initialize_config` in one script with a preflight check, and
keep the window under a minute.

Rollout order:

1. Preflight: confirm the upgrade authority, back up the program keypair, fund
   that authority's wallet for the `Config` rent (1,322,400 lamports) and the
   treasury seed balance (≥ 3,431,280 lamports). The authority wallet becomes
   `Config.admin`.
2. `anchor build`, regenerate and copy the IDL, then `anchor upgrade` with the
   unchanged program ID.
3. Immediately run `initialize_config(200, 0, SellerPays, 30_000_000)` signed by
   the upgrade authority, and assert `Config` reads back with
   `admin == authority`. A signature from any other key fails with
   `InvalidConfigAdmin`; if initialization fails, `anchor upgrade` back to the
   previous build.
4. Deploy the API.
5. Deploy the web app.
6. Smoke-test.

**Built so far (2026-11-14):** steps 1–3 ran against devnet, in three commands
rather than one script, so the upgrade→`initialize_config` window was a few
minutes instead of the planned one; no payment traffic was possible in it, and
nothing was paying at the time.

- Program `4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J` upgraded in place to
  the 0002 build (491,000 bytes, same ID, same authority `Evmo7zn…`, ProgramData
  `8GQRqbdm…`, slot 522160416). Upgrade signature `2GVaAF15…`.
- `Config 8FdnWvwzEKaWHgB3nY7AZwmiKtwKDkHabDKg8x617Msy` created by the upgrade
  authority (`4PJrZ2VN…`) and then set to the rollout policy
  `(200, 0, SellerPays, 30_000_000)` (`2avp93Dj…`); it reads back exactly that.
- `Treasury 9tyu74VybRdMEZ2zXszcsqS2mhsdr1iExaxoVkXVvQc4` funded with
  50,000,000 lamports. It held 67,820,680 after the settle below, so the
  threshold to keep is one receipt rent: top up below ~0.01 SOL, which is
  2,179,320 lamports of devnet rent plus margin.
- Settle smoke (1 SOL, throwaway buyer, memo `devnet smoke`, signature
  `5gHeXsFE…`): seller +980,000,000 (`amount - fee`), treasury +17,820,680
  (`fee - receipt_rent`; devnet's receipt rent is 2,179,320, not the mainnet
  2,540,400 this table assumed), buyer −1,000,005,000 (`amount` + the 5,000
  lamport network fee, receipt rent refunded). The 301-byte receipt exists and
  the invoice account is closed.
- The app's own helper, reading that `Config` and devnet rent, quoted
  `fee 20,000,000`, `sellerReceives 980,000,000`, `minPayable 1,855,756` — the
  same numbers the settle produced. A shop page then rendered "You receive
  0.01176 SOL after the 2% fee" and the 0.00181864 SOL rent for a 0.012 SOL
  cart.

**Program upgraded again for the refill fix (2026-11-15):** in place, same ID
and authority, signature `4k8QbJ4n…`, slot 522521670, 525,312 bytes after a
10,240-byte `solana program extend` (the loader will not grow data by 16 bytes
alone). The on-chain check and its signatures are recorded under Task 2.2; the
IDL did not change, so `apps/web/app/assets/payvand.idl.json` needed no
regeneration and the deployed web app keeps working unchanged.

**Program upgraded once more for the receipt-rent credit (2026-11-15):**
`29G84AqdfAT…`, slot 522529255, same ID, authority and 525,312 bytes — no extend
needed this time. The spend-to-zero check and its signatures are under Task 2.4.
Again the IDL is unchanged, so only the program moved.

**Program upgraded for the mint/vault constraints and the raised floats
(2026-11-17):** in place, same ID, authority and 525,312 bytes, slot 522910696,
upgrade `46z2UtBU…`.
ProgramData `8GQRqbdm…` starts with exactly what a fresh `cargo build-sbf` of the
current source produces (516,992 bytes, sha256 `18599442…`, 8,320 trailing zero
bytes), so the deployed artifact is the checked-in program and rebuilding the
same source is a no-op. The guards that make that build different were then run
against it on devnet (Task 3.3), and the budget that run opened reported the new
floats — 12,000,000 lamports on the session key, 18,000,000 on the budget — so
the amount the program moved is the deployed build's own constant.

**Web redeploy (2026-11-14):** the production build that quotes from `Config`
and sends the treasury account is live at `https://payvand-app.vercel.app`
(deployment `payvand-ahj2ckg40-…`, built from the two-file upload tree the
README describes). The deployed `/budget` chunk contains the `budget.cost.*`
keys of the new breakdown, so the fee UI shipped with it.

**Web redeploy after the token change (2026-11-14):** deployment
`payvand-63lqaqp9x-…` is aliased to `https://payvand-app.vercel.app` and its
checkout chunk carries `treasury_token_account`, so production now sends the
program the same account list the `2VDZzs4j…` settle used. The live
`/p/…` token invoice renders the price, "≈ 0.00001 SOL" for the network fee,
and nothing else — no protocol fee and no rent row. It was checked in the
browser, not paid from it: the preview Chromium has no signing wallet, so the
settle itself is the script one above.

**Web redeploy after the seller-account change (2026-11-15):** deployment
`payvand-2igo44ob4-…` is aliased to `https://payvand-app.vercel.app`. Checked on
the live site with a wallet-standard stub that hands its bytes to a throwaway
devnet signer: a fresh seller (`6dQg…KjPz`, 0.03 SOL, no token account) picked
USDC, priced 1.5, and pressed Create. The page asked for one signature, the
account `4dLexG6t…` appeared for mint `4zMM…` (1,488,440 lamports, owned by the
token program) in `4qXd94JX…`, and the seller's balance fell by exactly that
rent plus one 5,000 lamport fee. The invoice (`3mnEP2W8…`, "Seller ATA check",
1.5 USDC) was created afterwards; its live `/p/…` page shows the price and
"≈ 0.00001 SOL" for the network fee and no seller-account notice.

**The live token invoice paid (2026-11-15):** deployment `payvand-op9ze5szb-…`
(aliased like the others) paid `3mnEP2W8…` from a funded devnet buyer through
the page itself, signature `CRXEr1Ns…`. From the transaction: buyer −1.5 USDC
and −5,000 lamports, no rent and no fee; seller +1.47 USDC; treasury token
account +0.03 USDC; the 2,179,320-lamport receipt written, with the pool down
exactly that and the 1,981,200-lamport invoice paid back into it on close. Two
things had to be mended first, both found by attempting the payment:

- `minPayable` measured every invoice against the lamport invoice rent, so the
  checkout refused 1.5 USDC as "below the smallest amount the program can
  settle". Only a SOL settle takes its rent out of the price; a token settle
  refunds the rents from the pool and asks only `amount > fee`, so the rent is
  in that floor for SOL alone now.
- The treasury had no Circle-USDC or EURC token account, so the settle failed
  `AccountNotInitialized` on `treasury_token_account`. Both were opened paid by
  the deployer (`7G5ziyiB…` USDC, `ERJ39bL1…` EURC, owner the treasury PDA);
  the test mint's `3zhVS5bm…` was made the same way.

**Provisioning a mint is now a program step (2026-11-15):** `open_treasury_token_account`
(`OpenTreasuryTokenAccount` in `instructions/token.rs`) opens the treasury's
associated account for a mint, paid by the admin, idempotent, and gated on
`config.admin`. It exists because the treasury is a PDA: only the program can
sign for it, so a deployer could not open that account with a wallet call. The
program was upgraded in place for it (`276L62U4…`, 515,072 bytes, same ID and
authority) and the instruction was proved on devnet with a mint created for the
purpose: `5WkCtigV…` had no treasury account, one call (`4crPU2iH…`) created
`HdpMqUjB…` (1,488,440 lamports, authority the treasury `9tyu74Vy…`), and a
second call (`5n9dJ4BN…`) changed nothing.

Adding a currency is therefore two steps, and the second one is not optional:
put the mint in `CURRENCY_MINTS`, then provision it with one command —
`pnpm treasury:open --mint <mint> --cluster devnet`
(`apps/web/scripts/open-treasury-token-account.mjs`: reads the checked-in IDL
for the instruction's discriminator and account list, derives the config and
treasury PDAs plus the associated account, signs with `~/.config/solana/id.json`
unless `--keypair`/`--rpc` say otherwise, and reports "already open" instead of
sending a second transaction). Proved on devnet with mint `AJBMc6Ju…`: the
script created `4adC9BGM…` (authority the treasury `9tyu74Vy…`) in `2tqHv1Rn…`
and the second run sent nothing.

The checkout now reads the treasury's account for the invoice's mint and, when
it is missing, shows what is missing and holds the pay button instead of letting
the settle fail `AccountNotInitialized` after the buyer signs (that error was
the first thing the live payment run produced). `pay()` itself refuses on the
same quote — the button is one door, not the only one — and throws
`errors.treasuryTokenAccountMissing` before any wallet request; checked on the
local dev build with the treasury read forced empty: the failure was that
sentence and the stubbed wallet was never asked to sign. The deployed bundle
carries the same branch ahead of `settle_token_payment`, and
`apps/web/app/composables/usePaymentCheckout.test.ts` now pins it: it calls
`pay()` with a quote whose treasury account is missing and fails if anything is
built or the wallet is asked to sign (deleting the guard was checked to turn it
red), with a second case sending the same invoice once the account is there.
The provisioning script now takes each account's writable/signer flag from the
IDL instead of naming two of them, so a flag change there follows through.


Smoke tests:

- [x] SOL invoice at 0.01 SOL: payer pays 0.01 exactly (plus the 5,000 lamport
      network fee); seller receives 0.0098; treasury loses 1,979,320 lamports
      (fee 200,000 − receipt rent 2,179,320, devnet rent); the 301-byte receipt
      holds 2,179,320. Signature `5dq5uKBU…`.
- [x] SOL invoice at 1 SOL: seller receives 0.98; treasury gains the fee less the
      receipt rent (17,820,680 on devnet rent, not the 17,459,600 the mainnet
      table predicted).
- [x] Budget payment: pot decreases by exactly the price (10,000,000), `spent`
      becomes 10,000,000, the session key pays only the 5,000 network fee (the
      pot reimburses it both rents), seller +9,800,000, treasury +200,000 −
      receipt rent. `open_budget` moved exactly `Config.session_fee_float`
      (30,000,000) to the session key and the limit into a 97-byte account.
      Signatures `3Q7M8y4b…`, `2T48sVvp…`.
- [x] Token invoice pays the SOL way: seller receives `amount - fee_t`, the
      treasury's token account receives `fee_t`, and the payer spends only the
      network fee. The program was upgraded in place to slot 522185487
      (501,240 bytes, same ID and authority) and mint `29owUqEY…` got a treasury
      token account (`3zhVS5bm…`). Settle `2VDZzs4j…`, 0.70 tokens to a seller
      who had opened their own account: buyer −700,000 tokens and −5,000
      lamports (network only), seller +686,000 (`fee_t` 14,000), treasury +14,000
      tokens and −2,179,320 lamports (the permanent receipt). The pool comes out
      ahead in tokens and down one receipt rent, which is what the fee has to
      cover; the treasury token balance is the pool the plan converts later.
- [x] The seller's account is opened by the seller, not the buyer: `/sell`
      creates it with one `CreateIdempotent` signature before the invoice
      exists (paid by the seller, skipped when it is already there), and the
      checkout keeps the notice as the fallback for a code made elsewhere. The
      instruction is pinned by a unit test and was run on devnet for a fresh
      owner (`3DaYgcKv…`: account created, 1,488,440 lamports, owned by the
      token program).
- [x] Wrong-mint close fails; decoy vault rejected. Executed in the Rust harness
      (2026-11-16): the wrong mint gives `MintMismatch`, the decoy vault Anchor's
      canonical-ATA check, and neither moved a token. Repeated on devnet
      (2026-11-17) against the deployed program: `Custom(6008)` on the
      wrong-mint close, `Custom(2009)` on both decoy-vault calls, balances
      unmoved (`pnpm smoke:guards`, signatures under Task 3.3).
- [x] `close_budget` returns the pot and the account rent to the buyer and the
      account is gone (`42FiDA7V…`; the stand-in budget `5Jirbyfw…` likewise
      closed a 38,963,680 lamport account back to the wallet).
- [ ] Close all budgets: the float is swept and the vault drained. The SOL half
      is checked on devnet (2026-11-15, Task 4.1): a 0.02 SOL budget put
      30,000,000 lamports on session key `WxGBYkcQ…`, `close_budget` returned the
      pot and left that float untouched — which is why the sweep exists — and
      the client's sweep then moved 29,995,000 lamports to the buyer and left
      the key at **0** (`4WVsHfKK…`), with nothing left for a second sweep.
      Still open: `close_token_budget`'s vault drain needs a token budget and a
      token settle on devnet (Phase 3), and the close→sweep path was run as the
      instructions themselves, not clicked through `/budget` with a wallet — so
      the browser flow that calls them is still unexercised.
- [x] Shop page estimates match the settled balances: the 0.012 SOL cart showed
      "You receive 0.01176 SOL after the 2% fee" and `0.00181864 SOL` rent.
- [x] Budget page estimates match the settled balances: `/budget` rendered
      `0.03 SOL` on the session key (the `Config` float), `0.001143 SOL` account
      rent and `0.00001 SOL` network fee, total `0.131153 SOL` for a 0.1 SOL
      limit; for a live budget it showed the settled account's limit
      `0.02 SOL`, `0.02 SOL` left, pot `0.03782068 SOL` and session key
      `Gcnh…NQpe`, matching `open_budget` `66R7jHdF…`. The Chromium preview has
      no wallet extension, so this screen used a registered wallet-standard stub
      (read-only, no signing); `close_budget` `5Jirbyfw…` removed the stand-in
      budget.
- [x] Checkout page shows the same money on both currencies: the price, and
      the network fee as the only extra line. A token invoice whose seller has
      opened their account renders "Solana network fee ≈ 0.00001 SOL" and
      nothing else, matching settle `2VDZzs4j…` where the buyer lost exactly the
      5,000 lamport fee; a SOL invoice (`HBGn1Hoi…`) matches its settle
      `5dq5uKBU…` the same way. A token invoice to a seller with no account
      shows the setup notice instead of a rent the buyer would never owe.
- [x] `withdraw_treasury` moves surplus and leaves the rent-exempt minimum:
      withdrawing everything above it left 650,240 lamports (devnet's
      `minimum_balance(0)`, not the 890,880 this plan assumed for mainnet); the
      treasury was then refunded to 50,650,240. Signature `3y9KL8DJ…`.

Acceptance criteria:

- [x] The program ID is unchanged.
- [x] `Config` exists with the launch policy and is readable by the client.
- [x] The IDL in the web app matches the deployed program.
- [ ] Smoke tests pass.
- [x] The treasury balance is recorded after rollout, with a documented top-up
      threshold (67,820,680 lamports; top up below ~0.01 SOL).

Verification:

- [ ] `solana program show <PROGRAM_ID>`
- [ ] `solana account <CONFIG_PDA>` and `<TREASURY_PDA>`
- [ ] The checklist above.

Files likely touched:

- `apps/web/app/assets/payvand.idl.json`
- `apps/api` deployment config (if any)
- `README.md` / `DEV.md`

Dependencies: Task 5.1.

Estimated scope: Medium.

#### Task 5.3: Update docs and archive

Acceptance criteria:

- [ ] `README.md` documents the fee model, the treasury, and the numbers above.
- [ ] `plans/README.md` lists this plan as completed and archived.
- [ ] The plan file moves to `plans/archive/`.
- [ ] Follow-ups (Token-2022, relayer/sponsor, per-shop fees) are captured as
      issues or new plans.

Verification:

- [ ] Doc review.
- [ ] `git status` shows only intended changes.

Files likely touched:

- `README.md`
- `plans/README.md`
- `plans/archive/0002-fee-model-and-treasury.md`

Dependencies: Task 5.2.

Estimated scope: Small.

---

## Checkpoints

- [ ] **Checkpoint 1:** `Config` and the treasury exist and only the upgrade
      authority could create `Config`; SOL fees land in the treasury; the
      receipt rent is refused unless the treasury can pay it; `SellerPays` pays
      the payer exactly the price and lands before `BuyerPays` is added; small
      invoices fail with `InvoiceTooSmall` instead of underflowing; a
      caller-supplied fee account is rejected.
- [x] **Checkpoint 2 (2026-11-15):** The pot pays exactly the invoice amount
      (devnet spend-to-zero: pot 0 alongside `spent == limit`); `spent` matches
      it; same-month refills preserve the counter (`fQadYNyE…`: 0.01 spent, kept
      across a refill); preflight checks match the program, which now credits the
      receipt rent before checking the pot.
- [x] **Checkpoint 3 (2026-11-16):** Token sellers need no SOL; token fees reach
      the treasury ATA; the float covers three fresh-seller payments; wrong-mint
      and decoy vaults fail. All four execute in the harness: three sellers with
      no SOL and no account are paid out of the float, the fourth needs a top-up,
      and the buyer's plain transfers are the whole refill (Task 3.2). The two
      refusals were then repeated on devnet against the deployed program
      (2026-11-17, `pnpm smoke:guards`): `Custom(6008)` for the wrong mint,
      `Custom(2009)` twice for the decoy vault, nothing stranded.
- [ ] **Checkpoint 4:** Closing or rotating sweeps the float; events carry the
      split; Token-2022 is documented; `Config` layout is tested; no dead code.
- [ ] **Checkpoint 5:** `task verify`, `anchor test`, and the devnet smoke tests
      pass; the treasury has a recorded balance and threshold; the plan is
      archived.

## Risks And Mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Treasury runs dry, blocking settles | High | `TreasuryUnderfunded` fails before transfers; expose the balance and a top-up threshold; `withdraw_treasury` leaves the rent reserve. |
| Below-break-even invoices drain the treasury faster than fees refill it | High | Break-even table documented; admin visibility; `fee_bps` is the lever; the minimum viable invoice is enforced on chain. |
| `Config` missing during the upgrade window | Medium | Upgrade and `initialize_config` run as one script with a preflight check and a rollback step. |
| Fee redirect through a substituted account | Critical | The fee destination is a PDA derived from `TREASURY_SEED`; no `Config` field names a replaceable wallet; a negative test passes an arbitrary wallet. |
| Fee/cap misconfiguration (10 000 bps, huge fixed fee) | High | `update_config` validates `fee_bps <= 10_000`; settle caps at `amount - 1`; admin-only writes; tests for both bounds. |
| A stranger initializes `Config` first and installs themselves as admin | Critical | `initialize_config` requires the upgrade authority's signature and verifies it against this program's `ProgramData`; the account is pinned to the loader's PDA by seeds and owner, and a non-authority signer gets `InvalidConfigAdmin`. |
| `BuyerPays` is enabled by mistake and the UI never disclosed the fee | Medium | The UI cannot select the mode; the budget path rejects it outright; `estimateInvoiceCosts` renders a payer fee line whenever the mode is active. |
| Token receipt rent refund is skipped when the treasury ATA is created | Medium | Treasury funds its own ATA; refund and ATA creation are separate transfers, both asserted in tests. |
| Float top-up turns into another hidden charge | Medium | `session_fee_float` is shown in the open/refill breakdown and is refunded on close/rotate. |
| UI estimate drifts from program math | Medium | One helper, floor rounding documented, unit test comparing it to program outputs. |
| Admin key compromise drains the treasury | High | Document the authority; move to a multisig when available; `withdraw_treasury` is the only outflow path and leaves the reserve. |

## Definition Of Done

- [ ] All listed findings are checked.
- [ ] `task verify`, `anchor test`, and
      `cargo clippy -p payvand -- -D warnings` pass.
- [ ] The payer pays exactly the displayed price for SOL invoices; the seller
      receives the price minus `fee_bps`; the treasury pays the receipt rent.
- [ ] No path can send the fee anywhere but the treasury PDA.
- [ ] Token sellers need no SOL; token fees reach the treasury ATA; the float is
      sized, topped up on demand, and swept on close/rotate.
- [ ] `spent` equals what left the pot; same-month refills preserve it.
- [ ] The three pages show the estimate the program will produce.
- [ ] Devnet smoke tests pass on an unchanged program ID with `Config`
      initialized.
- [ ] Docs and comments match deployed behavior.
- [ ] The plan is archived and listed in `plans/README.md`.

## Open Questions

- Is 2% the launch rate, or should the devnet config use a smaller pilot value?
- Initial treasury funding amount and the alert threshold before a top-up is
  required?
- Per-shop fee overrides now, later, or never?
- Should `Config.admin` (and therefore `update_config` and
  `withdraw_treasury`) move to a multisig before mainnet? The treasury itself
  stays a PDA; only the authority over it would change.
- Should token invoices ever close rent to the seller again (a deliberate SOL
  onboarding rebate), or is the payer/session-key close final? Decided
  2026-11-18 (revision 12): the close goes to the pool that funded it, so a
  seller's proceeds are `amount - fee_t` and no rebate is planned.
- Payer-side gasless support remains a separate plan; confirm it is deferred.
