# Plan: Fee Model, Treasury, and Budget Accounting

## Status

Completed — archived 2026-11-22. The fee model, the treasury, the budgets and
the events shipped (see *Current State*); the verification still owed is in
[`plans/REMAINING.md`](../REMAINING.md), so the plan does not have to be
reopened for it. New (2026-11-12), split from the 2026-11-09 contract security
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

## Current State (2026-11-22)

This plan's model shipped and then moved on. The single rate and the 301-byte
receipt described below are the revision-22 state; two later commits, and the
uncommitted work that follows them, replaced both. Read the tables and task text
as history, and this section as what the program does now:

- **The receipt is a 48-byte marker** (`760053b`, 2026-11-20):
  `PaymentReceipt { invoice, settled_at }` at 1,224,960 lamports of rent
  (mainnet, 6,960 a byte), down from 301 bytes and 2,985,840. Everything a
  receipt used to copy — the split, payer, memo, reference — is in the
  `PaymentSettled` event, which the API now reads from the settle transaction's
  own logs.
- **Its rent is the fee's floor, charged in the currency** (`760053b`): a SOL
  seller is charged `max(rate, R_r)` (`fee_for` returns `max(0, proportional −
  receipt_rent)`), a token seller `max(proportional, flat)` where the flat is
  that rent in the token. The treasury refunds the marker's rent to the payer on
  the SOL path and both rents — invoice and marker — on the token paths, where
  the invoice closes back to the treasury pool.
- **The policy is tiered** (`1c0600d`, 2026-11-21): `Config` v2 carries a
  `FeePolicy` — `fee_bps_below` / `fee_bps_above`, a SOL and a token boundary,
  and per-mint flats for USDC and EURC — in place of `fee_bps` +
  `fixed_fee_lamports`. The default is 100 bps up to 0.1 SOL (10 token units)
  and 50 bps above it, so "2%" appears nowhere in the program any more.
- **The flats are live and refreshed in public** (uncommitted at the time of
  writing): `Config` v3 adds `flats_updated_at`, a separate `FeeAnchor` PDA holds
  the admin's blessed SOL price, and `refresh_fee_price` recomputes both flats
  within ±1,000 bps of that anchor at most once a minute. `migrate_config`
  upgrades v1 and v2 in place. `withdraw_treasury` keeps
  `TREASURY_RESERVE_SETTLEMENTS` (one token settle's refund) back, plus the next
  rotation's rent once `0006`'s flag exists.
- **The seller's token account is the seller's cost**: no settle or budget path
  opens it any more, and `SESSION_FEE_FLOAT` (12,000,000) is sized from one
  settlement's two records (3,939,360) instead of three fresh sellers.
  `MIN_SESSION_FEE_FLOAT` is 3,716,640.
- **Shops are sold by subscription** (€1.70 / 30 days, paid as an ordinary
  invoice from the platform seller, `apps/api/src/subscription.rs`). It grew out
  of this work but is outside this plan's scope and has no plan of its own.
- **What is still owed:** Task 5.2's `fQadYNyE…` budget and the browser
  click-through of `/budget` (revision 20), and `anchor test`'s full form, which
  would upgrade devnet to prove what the harness already proves (revision 22).
  Those are checks, not building, so they live in
  [`plans/REMAINING.md`](../REMAINING.md) — the same place this plan's other
  unrun checks are — and they do not reopen it. Task 5.3's other complaint is
  settled: the follow-ups it named are written (`0003`, `0004`, `0005`, plus
  `0006` for compressed receipts), and the plan is archived.
- The numbered tables under **Fee And Rent Numbers**, and every `fee_bps` /
  `fixed_fee_lamports` / 2% reference in the task text, describe the pre-tier
  model. Price a reopened task from `helpers::fee_for` and `Config::policy`, not
  from those tables.

## Remaining Work

Audited 2026-11-17 against `apps/contracts/programs/payvand` and `apps/web`. Each
line says where it stands, so a later pass does not have to re-derive it.

### Code

- **Task 4.3 — closed (2026-11-18).** `src/events.rs` declares
  `PaymentSettled`, `ConfigUpdated` and `BudgetOpened`, emitted beside the `msg!`
  lines on all four settle paths, both budget opens and `update_config`. The
  IDL's `events` section was regenerated and copied to
  `apps/web/app/assets/payvand.idl.json`, and six harness tests decode the
  emitted payloads. Close-path and treasury events are deliberately not built,
  and no API or web consumer is planned: the task's note records why (the splits
  in the app are quotes, and a log-fed history costs more than the receipt scan).
- **Task 4.4 — closed (2026-11-18).** Every token context refuses a mint
  the SPL Token program does not own with `PaymentError::Token2022Unsupported`
  (`Custom(6017)`), and the token contexts use `anchor-spl`'s interface account
  types so that check is reached instead of Anchor's generic
  `AccountOwnedByWrongProgram`. No instruction's account list or order changed.
  `BUDGET_SPACE`, `TOKEN_PAYMENT_SPACE` and `TOKEN_BUDGET_SPACE` are pinned in the
  harness beside `Config`'s and the payment layouts'. The docs half closed too
  (2026-11-18): `README.md`'s *Token programs* section and `DEV.md`'s token block
  name SPL Token as the only supported program, quote the `Custom(6017)` refusal
  and link `plans/deferred/0003-token-interface-migration.md`, which carries the
  `token_interface` migration.
- **Task 1.6 — closed (2026-11-18).** All three boundary cases now execute:
  `fee_floors_rather_than_rounds_at_ninety_nine_lamports`,
  `a_ten_thousand_bps_policy_admits_no_sol_invoice`, and
  `withdrawing_the_treasury_to_its_rent_underfunds_the_next_settle`.
- **Task 3.1 — closed (2026-11-18).** The token cap is exercised by
  `a_ten_thousand_bps_policy_leaves_the_token_seller_one_base_unit`, and the
  zero-fee skip is executed rather than described by
  `a_zero_fee_settle_token_payment_sends_no_fee_transfer` and
  `a_zero_fee_budget_payment_sends_no_fee_transfer`, which freeze the treasury's
  account so a sent zero transfer would fail, and on the SOL wallet path by
  `a_zero_fee_settle_payment_sends_no_fee_transfer`, which compares what ran: a
  0% settlement makes one system-program invocation fewer than the same
  settlement at 2%, and that one is the fee transfer. The handler's
  `require!(amount > fee_t)` cannot fire — `fee_for` caps the fee at
  `amount - 1` — so the cap test is as close to that floor as the program can be
  driven.
- **Task 3.5 — closed (2026-11-18).** The revision-12 change is built: the
  fifteen-account interface, the fee split, the pool-funded rents, the close to
  the treasury, the rebuilt `.so` and IDL, the client's builder, and the harness
  and client float rules. Revision 13 records the measured numbers; the devnet
  checks it could not run are listed there and in Task 3.5's verification.

### Verification this repo does not run

- `anchor test` appears in 1.6, 5.1 and 5.3. The suite this repo runs is
  `cargo test --manifest-path apps/contracts/programs/payvand/Cargo.toml --test
  settle` against the committed SBF artifact and the SPL ELF fixtures.
- Manual localnet/browser checks: 1.4 (config TTL refresh), 1.5 (fee note on a
  settled invoice), 2.3 (float drained, UI warns), 4.2 (close/rotate and invoice
  flows in a browser).

### Rollout record

- **Task 5.2 — closed (2026-11-19).** The rollout itself is recorded in the
  revision notes (deployed, `Config` set, budgets opened and paid from); the
  three checkboxes that stayed open for want of a recorded read were filled the
  same way: `solana program show`, `solana account` on `Config` and the treasury,
  and the close-all-budgets sweep. Revision 17 records what each answered. One
  SOL budget (`fQadYNyE…`) is still open — see that box — and the browser
  click-through of `/budget` is still unexercised: it was attempted on
  2026-11-19 and blocked by the missing wallet and the missing session key, both
  recorded in that box.

### Deferred by decision

- **Task 1.3 — `BuyerPays`.** Every settle path requires `SellerPays` and refuses
  `BuyerPays` with `FeeModeUnsupported` (`sol.rs`, `token.rs`), so the mode exists
  in `Config` and charges nothing. The charging half was never built and no
  product asks for it.
- Token-2022 migration (`token_interface`) — written out in
  [0003-token-interface-migration.md](../deferred/0003-token-interface-migration.md) — and
  merging the two float constants onto `Config::session_fee_float` (revision 8).
- Payer-side gasless/relayer support —
  [0004-relayer-sponsored-fees.md](../0004-relayer-sponsored-fees.md) — and
  per-shop fee overrides — [0005-per-shop-fees.md](0005-per-shop-fees.md). Both
  are drafts, and both wait on this plan's global policy being deployed.

### Archival

- **Task 5.3 — closed (2026-11-22).** The follow-ups this plan named are written
  out (`0003`, `0004`, `0005`, plus `0006`), and the plan moved to
  `plans/archive/0002-fee-model-and-treasury.md`. `plans/README.md` lists it
  under **Archived** with a `Completed` status, and the checks it still owes —
  the two open items in **Current State**, `task verify`, `cargo clippy`, the
  manual localnet pass and the Definition of Done's stale bullets — are tracked
  in [`plans/REMAINING.md`](../REMAINING.md) instead of keeping a shipped plan
  open. Every link into this file was rewritten for its new directory.

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
- Built in Task 3.5 (2026-11-18): the program, the IDL and the client's builder
  all carry the shape above, the harness executes the rebuilt artifact, and
  revision 13 records the measured numbers.

Revision 13 records Task 3.5's build (2026-11-18):

- **The account list is live.** `pay_from_token_budget` takes the fifteen
  accounts revision 12 listed, in that order, and
  `apps/web/app/assets/payvand.idl.json` is the IDL `anchor idl build` produced
  from them. The handler runs the nine steps: guards (including the fee mode) →
  `fee_for` → `ensure_treasury_funded(invoice + receipt)` → vault → seller
  `amount - fee_t` → vault → treasury ATA `fee_t` when it is non-zero →
  `pay_from_treasury(session_key, invoice + receipt)` → writes and `spent` →
  `payment.close(treasury)`.
- **Measured, not guessed.** At the bank's 6,960 lamports a byte one payment
  fronts 7,744,520 — invoice 2,714,400, receipt 2,985,840, the seller's account
  2,039,280, and the 5,000 signature — and the pool refunds 5,700,240 of it in
  the same transaction, so a settled payment costs the session key 2,044,280.
  The key needs the full 7,744,520 available at the start of each payment, so
  three consecutive sellers need 11,833,080: `SESSION_FEE_FLOAT` stays
  12,000,000, and the harness pins the boundary at three and a refusal at four.
- **The budget's float is gone.** `TOKEN_BUDGET_FEE_FLOAT` is deleted from the
  program, the client and the harness; `open_token_budget` funds the account to
  its own rent (`wanted = budget_rent`), `tokenOpenCost` quotes the rent shortfall
  only, `tokenFloatNeed` returns `{ budget: 0, session: peak }` and the shared
  vectors in `tests/fixtures/token-float-top-up.txt` read the same way on both
  sides.
- **The pool's position.** A payment leaves the pool one receipt rent down
  (2,985,840 SOL) and `fee_t` up in the mint — the break-even the plan already
  documents — and the invoice's own rent comes back through the close. A seller
  ends a payment holding no lamports at all, and the harness asserts it twice.
- **One build lesson.** With fifteen accounts the SBF linker refused the struct
  ("overwrites values in the frame") until the `Account` wrappers were boxed;
  `PayFromTokenBudget` boxes them now, and the warning is gone.
- **Verified:** `cargo build-sbf`, 43 harness tests, `cargo fmt --check`,
  `cargo clippy --all-targets -D warnings`, and `vitest run` (123),
  `nuxt typecheck`, `eslint .` in `apps/web`. **Not verified:** the devnet smoke
  run and a real silent payment against a deployed build of this revision —
  both need the upgrade in Task 5.1.

Revision 14 records Task 4.4's enforcement (2026-11-18):

- **Token-2022 is refused by name, not inside a CPI.**
  `PaymentError::Token2022Unsupported` sits after `FeeOverflow` (`Custom(6017)`),
  so every existing code keeps its number, and every token context's `mint` is an
  `UncheckedAccount` constrained to the SPL Token program and the 82-byte SPL
  mint layout. The error a caller sees is
  `AnchorError caused by account: mint … Error Number: 6017`.
- **The account types moved to the interface; the policy did not.** Anchor parses
  typed accounts before constraints, so a first attempt with the mint checked by
  a constraint but the accounts left as `Account<TokenAccount>` still reported
  `AccountOwnedByWrongProgram` (3007) on `seller_token_account` — measured, not
  assumed. `InterfaceAccount<TokenAccount>` and `Interface<TokenInterface>` parse
  both programs, which lets the mint's owner check be the rejection. No
  instruction's account list or order changed, so the deployed interface, the
  IDL and the clients match revision 13; `transfer_checked` and the extension
  rules stay deferred.
- **Layouts pinned.** `TokenBudget` joins the serialisation test, and
  `BUDGET_SPACE = 97`, `TOKEN_PAYMENT_SPACE = 262` and
  `TOKEN_BUDGET_SPACE = 129` are asserted.
- **On devnet.** The rebuilt artifact was deployed in place at slot 523617551 —
  529,320 bytes fit the existing 545,792-byte `ProgramData`, so no
  `solana program extend` — and the deployed bytes hash to `b8b0fc7a…`, matching
  `target/deploy/payvand.so`. `pnpm smoke:silent-payment` and
  `pnpm smoke:guards` both pass against it (the split, the wrong-mint and decoy
  guards, the close path), the interface account types included; the first
  attempts failed only on the public RPC's 429s.
- **Verified:** `cargo build-sbf`, 53 harness tests, `cargo fmt --check`,
  `cargo clippy --all-targets`, `anchor idl build` (IDL copied to
  `apps/web`), and `vitest run` (124), `nuxt typecheck`, `eslint .` in
  `apps/web`. **Not verified at that revision:** the docs half of the task, which
  Revision 16 closes.

Revision 15 pins Task 3.1's zero-fee rule on the SOL wallet path (2026-11-18):

- **Pinned by what ran, not by balances.** `settle_payment`'s `if fee > 0` guards
  a `system_program::transfer` to the treasury, and a zero lamport transfer would
  succeed, so no balance tells a skip from a send.
  `a_zero_fee_settle_payment_sends_no_fee_transfer` settles the same
  100,000,000-lamport invoice at 0% and at 2% and asserts the 2% transaction made
  exactly one more system-program invocation — the fee transfer. A new
  `send_with_logs` helper returns `process_transaction_with_metadata`'s log,
  because `process_transaction` returns none.
- **The test pins the guard.** With `if fee > 0` replaced by `if true` and the
  artifact rebuilt, the test fails (5 invocations at 0%, the same as at 2%) while
  the rest of the suite still passes; the guard is restored and the artifact
  rebuilt before the suite run that counts.
- **The SOL budget path has nothing observable to pin.** Its `if fee > 0` guards
  a direct `add_lamports` to the treasury, a no-op at zero even when executed; the
  token paths' frozen-account tests are what pin the CPI shape there.
- **Verified:** `cargo build-sbf`, 53 harness tests, `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`.

Revision 16 closes Task 4.4's documentation half (2026-11-18):

- **`README.md` says which token programs it settles.** A *Token programs*
  section states SPL Token mints only, the owner-and-82-byte-layout check, the
  named refusal `PaymentError::Token2022Unsupported` (`Custom(6017)`) with its
  message, why the interface account types are there (otherwise
  `AccountOwnedByWrongProgram` answers first), that the transfers themselves are
  still SPL Token's, and that `CURRENCY_MINTS` offers only SPL Token mints.
- **`DEV.md` says the same where the token treasury step lives** — the exact
  error a caller sees, that the refusal happens before any CPI so nothing moves,
  and that a Token-2022 mint added today would only buy a `6017`.
- **A follow-up to link, not a placeholder.**
  `plans/deferred/0003-token-interface-migration.md` (Deferred) records what exists and the
  work the migration actually needs: a `Config` mint/extension policy, interface
  CPIs with a truthful `seller_proceeds` under transfer fees, per-program ATA
  derivation, a treasury account per mint and program, the mint list, and the
  test that flips `a_token_2022_mint_is_rejected_by_name` into the positive
  case. `plans/README.md` lists it, and `0002`'s out-of-scope line and
  deferred-work bullet point at it.
- **Verified:** doc review against the source — the
  `mint.owner == anchor_spl::token::ID && mint.data_len() == SPL_MINT_LEN`
  constraint in `instructions/token.rs`, `Token2022Unsupported` last in
  `PaymentError`, and `CURRENCY_MINTS` in `apps/web` — with no code changed.

Revision 17 records Task 5.2's closing evidence (2026-11-19):

- **The two read-backs, run rather than described.** `solana program show
  4grKZpw6…` answers Program Id `4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J`,
  owner `BPFLoaderUpgradeab1e…`, ProgramData
  `8GQRqbdm86rwhmqk45Fkwt2Y4pajZCc9dcvNg6LuPMq1`, authority
  `Evmo7znLU9hSZr9t2VrGVLaqV7bifxG7RKPSGvM3Tvqw`, last deployed in slot
  523617551, 545,792 bytes, 2.7735022 SOL. `solana account` finds `Config`
  (`8FdnWvwz…`) at 62 bytes and 965,200 lamports owned by the program, and the
  treasury (`9tyu74Vy…`) at 19,360,440 lamports — above the top-up threshold the
  rollout record names.
- **The vault drain, exercised.** `pnpm smoke:guards` opened token budget
  `84f9QsETyZ9eiVuFyHvvh92pFPM6tSGUjRB14w1sdzdv` (`AaJ2EYNg…`, session float
  12,000,000, budget float 0) with its whole 500,000-unit limit in the vault, and
  `close_token_budget` (`3BSApWcb…`) drained it back to the buyer; the account is
  gone afterwards. Five token budgets that earlier partial runs left open
  (`3USURxyr…`, `3qimBEYF…`, `9ZtC6TER…`, `D68hNw9Z…`, `G3qxZkC2…`, all limit
  500,000, spent 0) were closed the same way — `3xumxyXx…`, `5iA1pNEJ…`,
  `577jkyv2…`, `2KALuBBR…`, `VkPsVAYk…` — each draining 500,000 units to the
  buyer, and a `getProgramAccounts` scan for 129-byte budgets on that buyer now
  returns none.
- **One SOL budget stays open, on purpose.** `fQadYNyE…` (Checkpoint 2's refill
  record) holds 0.081143 SOL. Its float sits on an app-generated session key, so
  closing it without that key would leave the float uncollected — the failure
  mode the sweep exists for. It is left for a run that has the key.

Revision 18 writes out the two follow-ups Task 5.3 named but the repo had no
plan for (2026-11-19):

- **`0004` — relayer and sponsored fees.** Records why the payer still needs
  SOL (it is its own fee payer and rent payer on every settle path), the
  lamport arithmetic that makes a naive sponsor a faucet — 2,179,320 of receipt
  rent coming back against 1,818,640 of payment rent going to the seller, so a
  sponsored token settle would net the payer +355,680 lamports — and the
  decisions to make before code: in-house sponsor or Kora, whether the sponsor
  funds only the fee or also the rents, and whether the program routes the
  refund to the sponsor.
- **`0005` — per-shop fee overrides.** Records that a shop is an API row while
  the fee is a program-wide `Config`, that a seller may keep several shops so a
  seller-keyed policy is not per shop, and that the shop therefore has to
  arrive as an optional account with `Config`'s rate as the fallback, because
  invoice seeds cannot grow without invalidating printed QRs.
- **Both are `Draft`, listed in `plans/README.md`, and linked from this plan's
  out-of-scope lines, deferred-work bullet and Linked Docs.** Task 5.3's
  follow-up box is annotated but still unticked: archiving this plan is its own
  step.

Revision 19 covers Task 5.1's two untested criteria (2026-11-19):

- **The API reports the record it promises, and no split.**
  `a_settled_record_claims_no_split` serialises a settled record and fails if a
  `fee`, `fee_lamports`, `seller_proceeds` or `receipt_rent_lamports` field
  appears, while pinning what the payload does promise — the amount, the status
  and `settled_by` — beside the decoder tests that already cover both currencies
  and the receipt-wins-over-an-invoice rule. The absent split is revision 15's
  decision, not an oversight: the API reads no events and today's `Config` is
  not the policy a past settlement used. A rent field was tried and removed the
  same day: nothing in `apps/api` or `apps/web` reads a receipt's rent, so it
  would have been a payload field with no consumer, added only to satisfy a
  checklist line. The test now forbids it coming back without one, and the API
  suite is 69 tests: the one that pinned the field left with it.
- **The quote is held to the program's arithmetic.** `useInvoiceCosts.test.ts`
  now exercises `estimateInvoiceCosts` against `fee_for` and the
  `InvoiceTooSmall` guard at a rent the stub can state (1,000 lamports a byte),
  and `invoiceCost.test.ts` gains a break-even table that searches for the
  smallest amount whose fee covers the receipt rent and checks both sides of it.
  The per-invoice subsidy this plan documents shows up as the gap between the
  floor (2,542,531 lamports on mainnet) and break-even (149,292,000).
- **Task 5.1's verification at revision 19:** the localnet pass over all four
  settlement paths is revision 21; `task verify` and `anchor test` are
  revision 22.

Revision 20 attempts Task 5.2's last caveat in the browser and records why it
cannot be met (2026-11-19). `/budget` loads on devnet at
`https://localhost:3001` and reports "No wallet detected": the preview
Chromium has no wallet-standard wallet, and `close_budget` needs the buyer's
signature. More decisively, the budget's float — 30,000,000 lamports on session
key `8nnbn9Wu…` — cannot be swept from anywhere: the app's `payvand.session`
IndexedDB store at that origin is empty, no keypair file answers to that
address, and the 2026-11-15 run passed an ephemeral key that its script deleted.
Closing the budget would strand 0.03 SOL, which is the failure the sweep exists
to prevent, so `fQadYNyE…` stays open and the orphaned float is written down as
the cost of script-run budget demos. The browser click-through is therefore
still open — it has now been attempted, and the plan says what blocks it.

Revision 21 runs Task 5.1's localnet pass and ticks it (2026-11-19). A
`solana-test-validator` 3.1.10 validator with a temporary ledger, the
revision-14 build (529,320 bytes) deployed to the same program ID, a 200-bps
config and a treasury seeded 100,000,000 lamports: all four settlement paths
settled, and every balance delta matched the fee model. Direct SOL and direct
USDC paid the seller the price less the 2% fee while the treasury lost the
receipt rent on SOL and gained the fee on USDC; both budget paths moved the pot
by exactly the price and left the session key at its float less the network
fee, with its rents reimbursed. The run also showed the treasury must be seeded
before the first SOL settle — `initialize_config` leaves it at its own rent and
`settle_payment` fails `TreasuryUnderfunded` until the rollout's funding step
runs. The per-path signatures and deltas are under Task 5.1. Afterwards the
validator was stopped and its ledger and driver deleted, so no local chain
state is left on this machine. `task verify` and `anchor test` were still open
then; revision 22 runs them.

Revision 22 runs Task 5.1's two remaining checks and records what each one is
(2026-11-19). `task verify` exits 0 — lint, typecheck, 148 web tests, 69 API
tests, and a fresh SBF build with the 53 settle-harness tests — so the box is
ticked; its only blemish is the `v-html` warning on `PaymentQr.vue`, which has
never been an error. `anchor test` is a different kind of open: `Anchor.toml`'s
`[scripts] test = "cargo test"`, so its test phase is the suite this repo
already runs, and `anchor test --skip-build --skip-deploy` passes all 53 through
the Anchor CLI without touching a cluster. The full command also builds and
deploys to the provider cluster — Devnet — which would upgrade the live program
to prove what the in-process harness proves against the same artifact, so that
box stays unticked with the reason written beside it. Nothing in Task 5.1 now
needs a validator, a browser or a deploy; the only open item is a check whose
full form costs a devnet upgrade and buys no new evidence.

## Linked Docs

- `plans/README.md` — plan conventions and lifecycle.
- `plans/deferred/0003-token-interface-migration.md` — the deferred Token-2022 migration
  this plan's Task 4.4 hands over.
- `plans/0004-relayer-sponsored-fees.md` — the sponsor the payer's SOL gap needs.
- `plans/archive/0005-per-shop-fees.md` — the per-shop override this plan's global
  policy leaves out.
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
- `apps/api/src/payments.rs` — on-chain decoders.
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
  fee and the temporary invoice rent on token invoices until a sponsor exists —
  designed in
  [0004-relayer-sponsored-fees.md](../0004-relayer-sponsored-fees.md).
- Token-2022 migration (documented as unsupported, tracked in
  [0003-token-interface-migration.md](../deferred/0003-token-interface-migration.md)).
- Mainnet deployment, discounts, a full event indexer.
- Per-shop fee overrides — designed in
  [0005-per-shop-fees.md](0005-per-shop-fees.md). One global policy ships here.

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

> **Historical (revision 22 and earlier).** These tables price a single
> `fee_bps` and a 301-byte receipt; the program now charges `Config::policy`'s
> two marginal rates with the 48-byte marker's rent as the floor. See
> **Current State** for the live model.

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
  floor. It emits `ConfigUpdated` beside its `msg!`.
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
- `apps/contracts/programs/payvand/tests/settle.rs`
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

- [x] `cargo test --manifest-path apps/contracts/programs/payvand/Cargo.toml
      --test settle` (the suite `Anchor.toml` runs for `anchor test`) — 53 tests,
      the three above among them (2026-11-18).

Files likely touched:

- `apps/contracts/programs/payvand/tests/settle.rs`

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
- [x] A payment at the limit succeeds, one lamport over fails with
      `BudgetExhausted` — moved to Task 2.4, which wrote it:
      `pay_from_budget_stops_at_the_monthly_limit`.

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/sol.rs`
- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/web/app/composables/useBudget.ts`
- `apps/contracts/programs/payvand/tests/settle.rs`

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
same rule on the token account; the harness now runs the token flows against the
committed SPL fixtures, but the counter is written into the account between
opens to keep the test about the refill rule rather than a payment).

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
      (`pot_before - pot_after == amount`) and in the devnet smoke test, and the
      token vault in
      `pay_from_token_budget_splits_the_price_and_closes_to_the_treasury`, which
      runs against the committed SPL fixtures (revision 6) and pins the vault
      down by exactly the invoice amount.
- [x] A compromised session key cannot exceed the pot balance:
      `pay_from_budget_cannot_spend_more_than_the_pot` leaves the pot holding
      half a price inside a four-invoice limit and gets `InsufficientBudget`
      (6006), with the counter untouched.
- [x] Seller proceeds and treasury fee match Phase 1 for budget payments (the
      same test asserts both deltas).

Verification:

- [x] `cargo test --manifest-path apps/contracts/Cargo.toml` — 53 tests (the
      `settle` harness; `Anchor.toml` runs the same suite for `anchor test`).

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
      holds for one path, not both (revision 6). Since Task 3.5 it holds on both:
      `pay_from_token_budget` passes the treasury accounts and splits the same
      way.
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
- [x] Zero-value transfers are skipped rather than sent. Pinned on both token
      paths (2026-11-18): `a_zero_fee_settle_token_payment_sends_no_fee_transfer`
      and `a_zero_fee_budget_payment_sends_no_fee_transfer` freeze the treasury's
      account for the mint and settle a 0% invoice. A zero `transfer` is still a
      CPI and the token program refuses one into a frozen account, so a
      settlement that succeeds is one that skipped the fee CPI; each test then
      switches the policy back to 2% and checks the same frozen account refuses
      the payment with `Custom(17)` (`AccountFrozen`), which is what makes the
      freeze a real refusal rather than a no-op. With the two `if fee > 0`
      guards replaced by `if true`, both tests fail and the rest of the suite
      still passes — the skip is what they pin.
      The SOL wallet path is pinned the same day by what actually ran, because a
      zero lamport system transfer succeeds and leaves no balance to read:
      `a_zero_fee_settle_payment_sends_no_fee_transfer` settles the same
      100,000,000-lamport invoice at 0% and at 2% and asserts the 2% transaction
      made exactly one more system-program invocation than the 0% one — the fee
      `system_program::transfer` — while the 0% treasury's delta is the receipt
      refund alone and the seller receives the whole price. The SOL budget path
      has nothing observable to pin: its `if fee > 0` guards a direct
      `add_lamports`, which at zero is a no-op even when executed.
- [x] The treasury refunds `R_tr` to the payer or session key: the executed
      settle ends with the pool exactly one receipt rent down — the refund out,
      the invoice rent back in.
- [x] The seller needs no SOL before or after the payment: both token tests pay
      a seller that holds no lamports and assert it stays at zero, and since Task
      3.5 closes the invoice to the pool the seller stays at zero for good.

Verification:

- [x] Rust harness, not Anchor: a seller with no token account and no SOL
      receives its tokens on the budget path, its account created and paid for
      by the float (`the_float_carries_three_payments_to_fresh_sellers`, which
      also refuses a fourth fresh seller for want of lamports).
- [x] Treasury ATA created on first use of a mint; second payment reuses it.
      Proved on devnet (2026-11-15) and by `open_treasury_token_account` being
      idempotent; the harness opens it once, so reuse is not tested. The account
      is opened by the admin, not lazily by a settle — the devnet proof is the
      creation and the reuse across payments, not the ordering the sentence
      implies.
- [ ] The payer's SOL delta is the seller ATA rent (when new) plus the network
      fee only. Not asserted: the payer path never creates the seller's account,
      and the transaction's fee payer in the harness is the bank's own key.

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/contracts/programs/payvand/src/constants.rs`
- `apps/contracts/programs/payvand/tests/settle.rs`

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
      invoice, the seller receives `price - fee_t` and no lamports, and the
      budget's lamports do not move at all. Re-measured in Task 3.5 (revision 13):
      the session key still fronts 7,744,520 a payment and is still 2,044,280
      lighter after one, so `SESSION_FEE_FLOAT` stays 12,000,000 and the
      18,000,000 budget float is gone.
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
- `apps/contracts/programs/payvand/tests/settle.rs`

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
- `apps/contracts/programs/payvand/tests/settle.rs`

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
- [x] Payment account closed to the rent payer, not the seller. Decided
      2026-11-18 (revision 12) and built in Task 3.5: the invoice closes to the
      pool that funds it, the seller keeps `amount - fee_t` alone, and the pot
      pays exactly `amount` in tokens. Before the change a fresh seller ended up
      holding the invoice rent (2,714,400 at the bank's rent); the harness now
      asserts the seller's lamports stay at zero.
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
      2026-11-16 the settle side runs too: three paid invoices from one budget
      are counted in `the_float_carries_three_payments_to_fresh_sellers`, and a
      fourth is refused for lamports rather than the limit.

Verification:

- [x] `cargo test --manifest-path apps/contracts/programs/payvand/Cargo.toml
      --test settle` — 53 tests, the token ones executing against the committed
      SPL fixtures; `cargo fmt --all --check` and
      `cargo clippy -p payvand --all-targets -- -D warnings` clean.

Files likely touched:

- `apps/contracts/programs/payvand/tests/settle.rs`

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

- [x] `pay_from_token_budget` takes the fifteen accounts above in that order, and
      the copied IDL names them.
- [x] Seller receives `amount - fee_t` tokens and nothing else; the treasury's
      ATA for the mint receives `fee_t`; the buyer's vault falls by `amount`.
      `pay_from_token_budget_splits_the_price_and_closes_to_the_treasury` pays
      700,000 and checks 686,000 into the seller's account, 14,000 into the
      treasury's, and the vault down by exactly 700,000.
- [x] The pot pays exactly `amount`: the vault falls by `amount`, `spent` rises by
      `amount`, and the budget's lamports do not move — it pays in tokens, and
      the pool, not the budget, funds the records. The session key ends the
      payment the seller's account rent and its signature lighter, the two
      records refunded.
- [x] The invoice closes to the treasury: the seller holds no lamports before or
      after, and the pool ends one receipt rent down per payment.
- [x] A treasury holding one rent but not both refuses with
      `TreasuryUnderfunded` before any token moves
      (`pay_from_token_budget_rejects_a_treasury_that_cannot_fund_the_records`).
- [x] A substituted `treasury` is refused by its seeds constraint
      (`pay_from_token_budget_rejects_a_substituted_treasury`, Custom(2006)); the
      config's seeds constraint is the same declaration the other two paths use.
- [x] `fee_bps = 10_000` on this path leaves the seller one base unit
      (`a_ten_thousand_bps_policy_leaves_the_budget_seller_one_base_unit`).
- [x] `useBudget.ts`'s silent payment sends the same accounts, with `config`,
      `treasury` and the treasury's ATA derived as the payer path derives them.
- [x] The floats and the shared vectors are re-measured and updated on both
      sides, and `TOKEN_BUDGET_FEE_FLOAT` is gone from the program, the client and
      the harness.
- [x] Task 3.4's criterion is ticked, with the invoice rent no longer reaching the
      seller.

Verification:

- [x] `cargo build-sbf --manifest-path programs/payvand/Cargo.toml`, then
      `cargo test --manifest-path apps/contracts/programs/payvand/Cargo.toml
      --test settle` — 53 pass (43 at the Task 3.5 build; the zero-fee,
      events, Token-2022 and SOL zero-fee work added ten more), the token tests executing
      the rebuilt artifact, and the five that build this instruction
      (`the_float_carries_three_payments_to_fresh_sellers`,
      `a_top_up_lets_the_float_carry_more_fresh_sellers`,
      `the_float_top_up_rule_matches_the_shared_vectors`,
      `a_decoy_vault_with_the_same_mint_and_authority_is_rejected`,
      `closing_a_token_budget_against_another_mint_fails_with_mint_mismatch`)
      pass on the new accounts. The assertion that left a seller holding
      `token_invoice_rent()` is inverted: the seller's lamports stay at zero.
- [x] `cargo fmt --all --check` and
      `cargo clippy -p payvand --all-targets -- -D warnings`.
- [x] `vitest run` (134 tests as of 2026-11-18; 123 at the Task 3.5 build),
      `nuxt typecheck`, `eslint .` in `apps/web`.
- [x] Devnet: `pnpm smoke:guards` (with the new accounts) and one real silent
      token payment through a budget, with the treasury's ATA gaining `fee_t`
      and the seller's wallet holding no lamports. Both ran on 2026-11-18
      against the upgraded program; the numbers and signatures are under
      "Devnet verification" below and in Task 5.1.
- [x] A revision note carries the measured float numbers and the new per-payment
      pool position (`fee_t - 2,985,840`): revision 13.

**Devnet verification (2026-11-18).** The rebuild was deployed in place — same ID
`4grKZpw6…`, same authority `Evmo7zn…`, slot 523581463, upgrade `2G3oygMr…`.
ProgramData `8GQRqbdm…` starts with exactly the 522,496-byte artifact the harness
tests, zero-padded to 525,312. Then, against that build:

- `pnpm smoke:guards` passes: throwaway mint A `6PUKkiiQ…`, budget `5Yj4R7No…`,
  `open_token_budget` `3UiogMcn…`. The floats it reports are revision 13's —
  session 12,000,000 lamports, budget 0 — so the deployed program is the build
  that no longer parks a records float. Refusals: `Custom(6008)` for the close
  against the wrong mint (`4X7WwLr4…`), `Custom(2009)` for the decoy payout from
  the budget (`5MS6XKkc…`) and for the decoy close (`4PDMt9hx…`); the vault kept
  its 500,000, the decoy and seller stayed at 0, and the budget closed
  (`Y8cdPWTx…`).
  The script needed one fix to get there: `pay_from_token_budget` now requires
  the treasury's account for the mint, so the smoke opens it with
  `open_treasury_token_account` (`5AJpN7wq…`) the way a deployment does. Without
  it the run stopped at `Custom(3012)` on `treasury_token_account` — the program
  reports the missing fee destination before it reaches the vault's ATA check —
  which is the new account being required, not the guard changing.
- A real silent payment through a budget (`pnpm smoke:silent-payment`): policy
  read from `Config` as 200 bps + 0, seller-paid. `open_treasury_token_account` `3aoN9Tyz…`,
  `open_token_budget` `QkFNHpZD…`, `pay_from_token_budget` `4WRnj5eN…`. A
  100,000-unit invoice paid by a session key that held only its float gave the
  seller `FcNQoBUo…` 98,000 units and **0 lamports**, before and after; put
  exactly 2,000 into the treasury's account `H5PxVHdJ…`; and raised `spent` to
  100,000. The session key went 12,000,000 → 10,506,560, down 1,493,440: devnet's
  1,488,440 seller-account rent plus the 5,000 signature, with both records
  refunded by the pool in the same transaction. The budget closed on the way out
  (`2rdxyKfdx…`). A second run produced the same numbers (`zS8z2TL…`), so the
  check is repeatable rather than a lucky pass.
- Three budgets the two failed runs above left open were closed afterwards
  (`4Jb3g378…`, `4XAF64Zv…`, `4XPSaq2H…`), so the cluster holds none of this
  run's.

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

**Status (2026-11-18): done** (`src/events.rs`). Three event types, emitted
beside the existing `msg!` lines, in the IDL's new `events` section:

- `PaymentSettled` — `amount`, `fee`, `seller_proceeds` (`amount - fee`),
  `payer`, `seller`, `mint`, `reference`. **One type for all four settle paths**
  (`settle_payment`, `pay_from_budget`, `settle_token_payment`,
  `pay_from_token_budget`): like `PaymentReceipt`, the mint carries the currency
  (`Pubkey::default()` for SOL) and `payer` is whoever signed — wallet or
  session key. That replaces the plan's `PaymentSettled` / `TokenPaymentSettled`
  pair, so an indexer subscribes to one event instead of guessing currency.
  The rents stay out of it: they are the pool's business, and the split an
  indexer needs is `fee` and `seller_proceeds`.
- `ConfigUpdated` — `admin`, `fee_bps`, `fixed_fee_lamports`, `fee_mode`,
  `session_fee_float`; emitted by `update_config`, so a settlement can be priced
  with the policy in force at its block.
- `BudgetOpened` — `buyer`, `session_key`, `mint`, `monthly_limit`; emitted by
  both `open_budget` and `open_token_budget` (again one type, empty mint for
  SOL). It covers refills too, since that is the same instruction.

Not built, deliberately:

- `BudgetClosed` / `TokenBudgetClosed` — the close handlers change no split and
  the accounts going away is itself the record; an event would only restate
  them. The plan's own "no event describes a close path" criterion says as much.
- `TreasuryWithdrawn` — an admin operation, visible as a treasury lamport move;
  no indexer of proceeds needs it. Add it when a treasury ledger exists.
- `initialize_config` emits nothing: it is a one-time bootstrap by the upgrade
  authority, not a policy change an indexer has to catch up with.

**No event consumer, deliberately (2026-11-18).** Nothing in `apps/api` or
`apps/web` re-derives a *settled* split that an event could replace. The API's
feed (`src/payments.rs`) reads `PaymentReceipt`/`Payment` accounts and reports
the invoice's price, with no fee field and no `Config` reader in production code
— the only `Config` decoder there is a test fixture. The web's split arithmetic
(`feeOn`/`invoiceCosts` in `utils/invoiceCost.ts`) is quote-only, rendered by
`InvoiceSplitNote` on the sell form and the shop page *before a payment exists*:
an event cannot answer that question, and the fee quoted has to be the policy in
force when the buyer signs, not one a past block recorded. The settled views
(the checkout receipt, `ProfileTransactionsCard`, `AdminPaymentsCard`) show the
receipt's price and never claim a split, so there is no Config-derived number to
correct. Taking the feed from events would also cost more than it saves:
`getSignaturesForAddress` plus one `getTransaction` per payment to read its
`Program data:` line, against today's single program scan, and only as far back
as the RPC keeps transactions. The narrow version worth adding when a seller's
settled screen has to show what they kept is one event read for one invoice (the
receipt's creation signature, then that transaction) — not a feed — and it
should say "price − fee" with the rent noted beside it, the way
`InvoiceSplitNote` already frames a quote.

Acceptance criteria:

- [x] Events exist for each state change and are in the IDL.
- [x] Settle events carry the split.
- [x] No event describes a close path; none exists.
- [x] `msg!` logs remain for human-readable explorers (they run first, then the
      event beside them); each settle emits exactly one `PaymentSettled`.
- [x] No duplicate emission: the event is the structured form next to the log,
      not a second copy of it.

Verification:

- [x] Harness tests decode the emitted payloads and assert the fields:
      `settle_payment_emits_the_split_it_paid`,
      `settle_token_payment_emits_the_mint_beside_the_split`,
      `pay_from_token_budget_emits_the_session_key_as_the_payer`,
      `update_config_emits_the_policy_it_installed`,
      `open_budget_emits_the_limit_and_the_session_key`,
      `open_token_budget_emits_the_mint_and_the_limit`. They simulate the
      instruction (`process_transaction` returns no logs), base64-decode each
      `Program data:` line, match the type's 8-byte event discriminator and
      borsh-decode the body. `cargo test --manifest-path
      apps/contracts/programs/payvand/Cargo.toml --test settle`: 53 passed.
- [x] `anchor idl build -o target/idl/payvand.json` regenerated the IDL with
      `events: [BudgetOpened, ConfigUpdated, PaymentSettled]` and their types;
      copied to `apps/web/app/assets/payvand.idl.json`. (`anchor build` is not
      the tool here — Anchor 1.2.1 emits eBPF arch v1, which the harness cannot
      execute; `cargo build-sbf` builds the artifact, `anchor idl build` the
      IDL.)
- `BuyerPays` is not asserted on a settle because no settle path accepts it
  (`FeeModeUnsupported`); the mode is pinned by the `ConfigUpdated` test instead.

Devnet (2026-11-18): the program was upgraded in place at slot 523592277 under
the same ID and `ProgramData` (`8GQRqbdm…`). The binary grew past its allocated
space, so `solana program extend <id> 20480` ran first — the loader only extends
in 10,240-byte steps, and a plain deploy of the 3,152-bytes-larger ELF fails with
`ExtendProgram requires a minimum of 10240 additional bytes`. `ProgramData` now
holds 545,792 bytes (0x85400); the deployed bytes match the artifact.
`pnpm smoke:silent-payment` passed against the new program, and its
`pay_from_token_budget` transaction (`4wKtjFru…`) carries one
`Program data: 9eb6984c6917e887…` line — the IDL's `PaymentSettled`
discriminator — decoding to amount 100,000, fee 2,000, seller proceeds 98,000,
session key as `payer`, and the invoice's mint and reference, beside the `msg!`
line in the same log.

Files likely touched:

- `apps/contracts/programs/payvand/src/events.rs` (new)
- `apps/contracts/programs/payvand/src/instructions/*.rs`
- `apps/contracts/programs/payvand/src/lib.rs`
- `apps/web/app/assets/payvand.idl.json`
- `apps/contracts/programs/payvand/tests/settle.rs`

Dependencies: Tasks 1.2, 1.3, and Phase 3.

Estimated scope: Medium.

#### Task 4.4: Token-2022 decision and account layout tests

Description: Document that the program uses `anchor_spl::token` and rejects
Token-2022, link a follow-up plan for `token_interface` migration, and make sure
the UI/API do not offer Token-2022 mints. Add an `InitSpace` (or size-assert)
test for `Config` and the budget accounts; the receipt and payment layouts keep
their `0001` tests.

The code half was built on 2026-11-18 (revision 14):

- **The rejection is named.** `PaymentError::Token2022Unsupported` ("Payvand
  settles only SPL Token mints; Token-2022 is not supported") is appended after
  `FeeOverflow`, so it is `Custom(6017)` and no deployed code moves. Every token
  context's `mint` is an `UncheckedAccount` constrained to
  `mint.owner == anchor_spl::token::ID && mint.data_len() == SPL_MINT_LEN`
  (`SPL_MINT_LEN = 82`, the SPL mint layout) with that error. A Token-2022
  settle now fails with `AnchorError caused by account: mint … Error Number:
  6017`.
- **Why the interface types are involved.** Anchor parses typed accounts before
  it evaluates constraints, so with `Account<TokenAccount>` the first
  Token-2022-owned account failed on its own owner check with the generic
  `AccountOwnedByWrongProgram` (3007) and the named check was never reached —
  the test reported exactly that before the change. `instructions::token` now
  types its token accounts as `InterfaceAccount<TokenAccount>` and its token
  program as `Interface<TokenInterface>`, which parse either program and leave
  the mint's owner as the rejection. The policy did not move: one program, named
  by the mint check, and the `token_interface` CPI migration remains deferred.
- **One size assertion per account.**
  `serialized_accounts_fit_their_space_constants` now serialises `TokenBudget`
  too (it already covered `Payment`, `TokenPayment`, `PaymentReceipt`, `Budget`
  and `Config`), and `account_spaces_match_the_sizes_used_by_the_program` pins
  `BUDGET_SPACE = 97`, `TOKEN_PAYMENT_SPACE = 262` and `TOKEN_BUDGET_SPACE = 129`
  beside `PAYMENT_SPACE = 230`, `PAYMENT_RECEIPT_SPACE = 301` and
  `CONFIG_SPACE = 62`.
- **The harness test.** `a_token_2022_mint_is_rejected_by_name` builds a
  Token-2022 mint, the buyer's, seller's and treasury's Token-2022 accounts and
  the Token-2022 program, then asserts the settle reports `Custom(6017)`, leaves
  no invoice and no receipt, and does not move the buyer's tokens.

The documentation half landed on 2026-11-18: `README.md` has a *Token programs*
section and `DEV.md` a token block, both naming SPL Token as the only supported
program, quoting the `Custom(6017)` refusal and its message, and linking
`plans/deferred/0003-token-interface-migration.md` for the migration. The UI/API mint
allowlist is unchanged, and the migration's own work (`transfer_checked`, the
extension rules) is not started — `0003` carries it.

Acceptance criteria:

- [x] Product docs state supported token programs and link the follow-up
      (2026-11-18: README's *Token programs* section and `DEV.md` name SPL Token
      as the only supported program, quote `Custom(6017)` and link
      `plans/deferred/0003-token-interface-migration.md`).
- [x] A Token-2022 mint fails with a clear error.
- [x] `Config`, `Budget`, and `TokenBudget` sizes are asserted by a test.

Verification:

- [x] Doc review (2026-11-18): every claim in the new README section and the
      `DEV.md` block was checked against the source — the
      `mint.owner == anchor_spl::token::ID && data_len() == SPL_MINT_LEN`
      constraint and `InterfaceAccount`/`Interface` types in
      `instructions/token.rs`, `Token2022Unsupported` as the last error
      (`Custom(6017)`), and `CURRENCY_MINTS` in `apps/web` listing only SPL
      Token mints on both clusters.
- [x] `cargo test --manifest-path apps/contracts/Cargo.toml --test settle`
      (2026-11-18: 53 tests, after `cargo build-sbf` rebuilt the artifact).
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` (clean
      apart from the third-party future-incompat note), `anchor idl build` with
      the IDL copied to `apps/web/app/assets/payvand.idl.json`, and in
      `apps/web`: `vitest run` (134 as of 2026-11-18; 124 at the Task 4.4
      build), `nuxt typecheck`, `eslint .` (only the pre-existing `v-html`
      warning).

Files likely touched:

- `README.md`
- `DEV.md`
- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/contracts/programs/payvand/src/errors.rs`
- `apps/contracts/programs/payvand/src/constants.rs`
- `apps/contracts/programs/payvand/tests/settle.rs`
- `apps/web/app/assets/payvand.idl.json`

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

- [x] `task verify` is green and `anchor test`'s configured test phase passes
      (2026-11-19). The `task verify` claim is its five steps and nothing else:
      `pnpm lint` (one warning, no errors — the `v-html` notice on
      `PaymentQr.vue:81`, which the QR drawing has always carried), `pnpm
      typecheck` clean, vitest 148 tests in 20 files, `cargo test` for the API
      69 tests, and `test:contracts` — a fresh `cargo build-sbf` of the revision
      14 source plus 53 settle-harness tests against the built artifact. The
      task does not run `anchor test`, so the `anchor test` claim is only its
      test script: `Anchor.toml` sets `[scripts] test = "cargo test"`, and
      `anchor test --skip-build --skip-deploy` runs it to the same 53 passes
      without contacting a cluster. The full `anchor test` also builds and
      deploys to `Anchor.toml`'s provider cluster, which is `Devnet`; that half
      did not run, because it would upgrade the live program to re-prove what
      the in-process harness proves against the same artifact. It is the single
      open item under Verification below.
- [x] `cargo clippy -p payvand -- -D warnings` passes (2026-11-18: clean apart
      from a third-party future-incompat note, as Task 4.5's verification
      records).
- [x] The IDL is regenerated and copied to
      `apps/web/app/assets/payvand.idl.json` (regenerated for the revision-14
      build and copied, 2026-11-18).
- [x] API tests cover the settled record the API promises — its price, status
      and signer, and no split (2026-11-19). `a_settled_record_claims_no_split`
      serialises a settled record and fails if a `fee`, `fee_lamports`,
      `seller_proceeds` or `receipt_rent_lamports` field appears, while pinning
      the amount, the status and `settled_by`; the decoder tests beside it cover
      both currencies and the receipt-wins-over-a-leftover-invoice rule. Neither
      a fee nor a rent is reported, and both absences are deliberate: a settled
      split is revision 15's decision (the API reads no events, and today's
      `Config` is not the policy a past settlement used), and a receipt's rent
      has no reader in `apps/api` or `apps/web`, so a field for it would be
      payload with no consumer. 69 API tests pass (the rent test went with the
      field).
- [x] `estimateInvoiceCosts` is tested against the program's math, including the
      minimum amount and the break-even point (2026-11-19).
      `useInvoiceCosts.test.ts` quotes a 1.5 USDC and a 0.01 SOL invoice from a
      stub cluster and holds the answer to `fee_for` and the `InvoiceTooSmall`
      guard: 30,000 of fee on 1,500,000 base units, a 234,694-lamport floor at a
      230,000-lamport invoice rent (refused at 234,693), and the token floor at
      1 because a token settle refunds its rents. `invoiceCost.test.ts` adds the
      break-even table: 149,292,000 lamports at 2% against mainnet's 2,985,840
      receipt rent, 298,584,000 at 1%, 108,966,000 on devnet's 2,179,320, and
      99,292,000 with a 0.001 SOL fixed fee — each asserted from both sides (the
      fee covers the rent at that amount and falls short one lamport below) and
      against the `fee_bps × amount ≥ receipt rent` rule this plan records.
      42 tests across the two files.
- [x] Localnet smoke test of all four settlement paths (2026-11-19). A
      `solana-test-validator` 3.1.10 local validator, the revision-14 build from
      `apps/contracts/target/deploy/payvand.so` (529,320 bytes) deployed to the
      same program ID, a config at 200 bps with a 12,000,000-lamport session
      float, and a treasury seeded with 100,000,000. Localnet's rent is 3,480
      lamports a byte-year: a 230-byte payment holds 2,491,680, a 262-byte token
      payment 2,714,400 and a 301-byte receipt 2,985,840. Each path was driven
      through the checked-in IDL and its balance deltas checked:
  - **`settle_payment`, 100,000,000 lamports:** the seller received 98,000,000
    and the treasury fell 985,840 — the 2,000,000 fee less the receipt rent it
    now owns. `3XT1b1iU…a55JXK`.
  - **`pay_from_budget`, 100,000,000 lamports:** `open_budget` (`33gSPZru…`)`,
    then the session key settled — the seller received 98,000,000, the pot fell
    exactly 100,000,000, the session key ended at 11,995,000 (its 12,000,000
    float less the 5,000 network fee it paid; its rents were reimbursed), and the
    treasury fell 985,840 as above. `3yaTzxFq…`.
  - **`settle_token_payment`, 50.000000 USDC:** the seller's account gained
    49,000,000, the treasury's account gained the 1,000,000 fee, and the payer's
    account fell 50,000,000. `2S3pAZwi…`.
  - **`pay_from_token_budget`, 50.000000 USDC:** `open_token_budget`
    (`4KW9coma…`), then the session key settled — the seller gained 49,000,000,
    the vault fell 50,000,000, the treasury gained 1,000,000, and the session key
    ended at 11,995,000 for the same reason. `4QCeufWH…`.

      Afterwards the program held exactly four 301-byte receipts and one budget
      of each kind (97 and 129 bytes). One thing the run taught that the plan
      did not say: `initialize_config` leaves the treasury at its own rent, so
      the first `settle_payment` fails `TreasuryUnderfunded` (6014) until it is
      seeded — the funding step the rollout describes, now observed rather than
      assumed. The driver was a temporary script against the IDL, not the
      browser app; the validator, its ledger and the script were removed after
      the run.

Verification:

- [x] `task verify` — exits 0; the five steps and their counts are in the first
      criterion above.
- [x] `anchor test`'s configured test phase (2026-11-19) —
      `anchor test --skip-build --skip-deploy` runs `[scripts] test = "cargo
      test"` from `Anchor.toml` and passes 53 tests without contacting a
      cluster.
- [ ] The Devnet deploy half of `cd apps/contracts && anchor test` — the full
      command also builds and deploys to `Anchor.toml`'s provider cluster, which
      is Devnet, and has not run: it would upgrade the live program to re-prove
      what the in-process harness proves against the same `.so`, so it is left
      to a run that is actually about deploying. `task anchor:test` is the
      deployment-free equivalent (Anchor build, SBF rebuild, same suite). This
      is the only open item in Task 5.1.

**Devnet verification of the revision-12 build (2026-11-18).** The revision-12
program was upgraded in place (same ID, authority and ProgramData; slot
523581463, upgrade `2G3oygMr…`) and the deployed artifact matched a fresh
`cargo build-sbf` of that revision's source byte for byte (522,496 bytes,
zero-padded to the 525,312-byte ProgramData). Two checks ran against it:
`pnpm smoke:guards` (`Custom(6008)` for the wrong mint, `Custom(2009)` twice for
the decoy vault, session float 12,000,000 and budget float 0 as the new constants
say), and one real silent token payment through a budget where the seller
received 98,000 of a 100,000 invoice and no lamports, the treasury's account
gained the 2,000 fee, and `spent` rose by the price. Details and signatures are
under Task 3.5's "Devnet verification". The later Token-2022 build (revision 14)
was upgraded in place at slot 523617551 — 529,320 bytes in the existing
545,792-byte `ProgramData`, no `extend` — and both smoke scripts passed against
that build too. `task verify` and the localnet pass over all four settle paths
were open when this note was written; they are revisions 22 and 21, and the API
and quote tests this note once listed are revision 19. The one box above still
unticked is the full `anchor test` with its Devnet deploy.

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
- [x] Close all budgets: the float is swept and the vault drained. The SOL half
      is checked on devnet (2026-11-15, Task 4.1): a 0.02 SOL budget put
      30,000,000 lamports on session key `WxGBYkcQ…`, `close_budget` returned the
      pot and left that float untouched — which is why the sweep exists — and
      the client's sweep then moved 29,995,000 lamports to the buyer and left
      the key at **0** (`4WVsHfKK…`), with nothing left for a second sweep.
      The vault drain ran on devnet on 2026-11-19: `pnpm smoke:guards` opened a
      token budget (`84f9QsET…`, `AaJ2EYNg…`) whose vault held its whole
      500,000-unit limit, and `close_token_budget` (`3BSApWcb…`) drained it back,
      leaving no account at the budget's address. The five token budgets earlier
      partial runs had left open (`3USURxyr…`, `3qimBEYF…`, `9ZtC6TER…`,
      `D68hNw9Z…`, `G3qxZkC2…`, each limit 500,000 and spent 0) were closed the
      same way and drained 500,000 units each; a `getProgramAccounts` scan for
      129-byte budgets on that buyer now returns none. Two things this does not
      claim: the close→sweep path was run as the instructions themselves and is
      still not clicked through `/budget` with a wallet, so the browser flow that
      calls them is unexercised; and one SOL demo budget, `fQadYNyE…` (the
      refill record under Checkpoint 2), is still open at 0.081143 SOL.

      **The `/budget` attempt, 2026-11-19: attempted and blocked, twice over.**
      The app was run (`pnpm dev`, devnet) and `/budget` opened at
      `https://localhost:3001`, the origin `nuxt.config.ts` documents. The page
      renders and answers "No wallet detected. Install a Solana wallet extension
      and reload." — this Chromium profile has no wallet-standard wallet, and
      `close_budget` is the buyer's signature, so no close can be sent from
      here as `Evmo7zn…`. The sweep is blocked for a reason that no wallet would
      fix: the budget's float is 30,000,000 lamports on session key
      `8nnbn9Wu9dnYcyWVp8TEmhkRw1uDwpfotthLeojVA6QA`, and that key is not on this
      device. The `payvand.session` IndexedDB database at that origin holds its
      `keys` store and **no entries**; no keypair file in the workspace or
      `/tmp` answers to that address; and the 2026-11-15 run that opened the
      budget passed an ephemeral session key (`open_budget` `66vW41yu…`) whose
      temp keypair the script removed with its workdir. Closing `fQadYNyE…`
      here would take the pot and the rent home and leave the 0.03 SOL float
      unspendable — the outcome the sweep exists to prevent — so the budget stays
      open. The float is orphaned, and that is the real cost of running budget
      demos from scripts instead of from the app, where the key would still be
      held. Closing the loop needs a browser that can sign as the buyer (a
      wallet extension) and a budget re-opened from the app, so the key being
      swept is one the device actually holds.
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
- [x] Smoke tests pass (2026-11-18: `pnpm smoke:guards` and
      `pnpm smoke:silent-payment` both passed against the revision-14 build at
      slot 523617551).
- [x] The treasury balance is recorded after rollout, with a documented top-up
      threshold (67,820,680 lamports; top up below ~0.01 SOL).

Verification:

- [x] `solana program show <PROGRAM_ID>` (2026-11-19: Program Id
      `4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J`, owner
      `BPFLoaderUpgradeab1e…`, ProgramData
      `8GQRqbdm86rwhmqk45Fkwt2Y4pajZCc9dcvNg6LuPMq1`, authority
      `Evmo7znLU9hSZr9t2VrGVLaqV7bifxG7RKPSGvM3Tvqw`, last deployed in slot
      523617551, 545,792 bytes, 2.7735022 SOL).
- [x] `solana account <CONFIG_PDA>` and `<TREASURY_PDA>` (2026-11-19: `Config`
      `8FdnWvwzEKaWHgB3nY7AZwmiKtwKDkHabDKg8x617Msy` — 62 bytes, 965,200
      lamports, owned by the program; treasury
      `9tyu74VybRdMEZ2zXszcsqS2mhsdr1iExaxoVkXVvQc4` — 19,360,440 lamports,
      owned by the system program, which is above the ~0.01 SOL top-up
      threshold).
- [x] The checklist above.

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
      issues or new plans. All three are plans as of 2026-11-19:
      [0003](../deferred/0003-token-interface-migration.md),
      [0004](../0004-relayer-sponsored-fees.md) and
      [0005](0005-per-shop-fees.md), each listed in `plans/README.md`; only the
      `plans/README.md` status change and the move to `plans/archive/` remain in
      this criterion.

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
- [x] **Checkpoint 4 (2026-11-18):** Closing or rotating sweeps the float;
      events carry the split; Token-2022 is documented; `Config` layout is
      tested; no dead code. Evidenced by Tasks 4.1 (sweep, checked on devnet),
      4.3 (the three events on all four settle paths), 4.4 (the *Token programs*
      docs and the named `Custom(6017)` refusal), the account-size assertion
      test, and 4.5 (clippy clean, no stale comments).
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
      receives the price less `max(rate, receipt rent)`; the treasury takes only
      the fee. (This bullet's original wording — `fee_bps` and a
      treasury-funded receipt — is revision-22 history; see **Current State**.)
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

- ~~Is 2% the launch rate?~~ Decided (`1c0600d`): 100 bps up to 0.1 SOL / 10
  token units and 50 bps above, with the receipt rent as the floor.
- Initial treasury funding amount and the alert threshold before a top-up is
  required?
- Per-shop fee overrides now, later, or never? Deferred; designed in
  [0005-per-shop-fees.md](0005-per-shop-fees.md).
- Should `Config.admin` (and therefore `update_config` and
  `withdraw_treasury`) move to a multisig before mainnet? The treasury itself
  stays a PDA; only the authority over it would change.
- Should token invoices ever close rent to the seller again (a deliberate SOL
  onboarding rebate), or is the payer/session-key close final? Decided
  2026-11-18 (revision 12): the close goes to the pool that funded it, so a
  seller's proceeds are `amount - fee_t` and no rebate is planned.
- Payer-side gasless support remains a separate plan; confirm it is deferred.
