# 0003 — Token-2022 migration (`token_interface`)

Status: `Deferred` (moved from `On hold` on 2026-11-26). There is no current
intent to add Token-2022 support. Follow-up to `0002`'s Task 4.4, which decided
the program settles SPL Token mints only and made that refusal explicit rather
than leaving it to a failure inside a CPI. `README.md` links this file as the
place the migration is tracked.

Task C is done and stays done; Tasks B, D, E and F are parked, not dropped.

## When To Implement

Only revisit if the product actually wants a named Token-2022 mint in
`CURRENCY_MINTS`. There is no date or traffic trigger: as of 2026-11-26 both
stablecoins Payvand prices in are classic SPL Token mints, and the useful
Token-2022 stablecoins carry extensions the current policy refuses. Restarting
starts by re-deciding the extension policy, then by admitting a mint under that
policy; see Task A. Until then, the shipped `token_interface` CPIs keep the SPL
boundary explicit.

First increment (2026-11-22): Task A's policy is decided and recorded below, and
Task C's interface CPIs are in the program — `cargo build-sbf` plus all 57 suite
tests passed with the refusal test unchanged. The mint constraint still admits SPL
Token only, so this increment changes no behaviour. Verification of that
increment: `cargo build-sbf` clean, `cargo test --test settle` 57 passed, `cargo
fmt --check` clean, `anchor idl build` byte-identical to
`apps/web/app/assets/payvand.idl.json` at the time.

Progress re-audited 2026-11-26 against `92010cb`: **the deferral stands and
nothing has restarted.** `CURRENCY_MINTS` is still USDC/EURC, the harness still
registers only SPL Token and the ATA program, and the boundary test still refuses
a base-layout Token-2022 mint by name. The current suite is 74 tests (the Task C
run above recorded 57); after a fresh `cargo build-sbf`, `cargo test --test settle`
passes 74/74 and `cargo fmt --check` is clean. `cargo clippy --all-targets -- -D
warnings` now fails on **six** lints in `tests/settle.rs`, not three: the old
`proportional.max(TEST_FIXED_FEE)` and a `manual_clamp` at :1405, the constant
assertions at :3147–3150, plus `clone_on_copy` at :4457 and `redundant_locals` at
:4469 from later work. The on-chain owner check is also stricter than Task A's
"one of the two programs" wording: today a mint must be owned by the classic SPL
Token program. Open boxes below are decided-away or parked by the deferral, not
oversights; none were ticked.

## Why it is deferred (2026-11-22)

Payvand prices in SOL, USDC and EURC, and **both stablecoins are classic SPL
Token mints on both clusters** — USDC `EPjFWdd5…` mainnet / `4zMMC9sr…` devnet,
EURC `HzwqbKZw…` — which is what Circle's own address table says (token standard
`SPL`) and what `README.md` already documents. `CURRENCY_MINTS` is the product
switch, so with that list unchanged the migration changes nothing a user or a
seller can see.

The tokens Token-2022 would add are the newer institutional stablecoins — PYUSD
(`2b1kV6Dk…`), USDG (`2u1tszS…`) — but the ones worth adding **carry
extensions**: PYUSD ships metadata/confidential-transfer extensions, USDG lists
permanent delegate, confidential transfers, metadata, transfer hook and mint
close authority. Task A's rule (owner is one of the two programs and exactly
`SPL_MINT_LEN`) refuses every one of them with `Token2022Unsupported`. So
completing Tasks B, D, E and F would admit only hypothetical extension-free
Token-2022 mints, while paying for the ATA-program plumbing on both sides, a
Token-2022 program fixture in the harness, and a test suite — for no mint the
product would name.

What stays: Task C. It is behaviour-identical, proven by the suite passing
unchanged and the IDL being byte-identical, and it is the only reason the SPL
Token refusal is the *named* `Token2022Unsupported` rather than Anchor's generic
`AccountOwnedByWrongProgram` — the guarantee `0002` Task 4.4 promised. Reverting
it would remove that name and buy nothing.

**Revisit when** a Token-2022 mint is actually wanted in `CURRENCY_MINTS` — that
is, the first stablecoin Payvand intends to accept that is not USDC, EURC or
SOL. That day needs an extension policy first (Task A, again: which extensions a
settlement may see, and what each does to the split, the `seller_proceeds`
promise and the account assumptions), because the owner-plus-length rule admits
none of the real ones. Tasks B, D, E and F below are then the work, and are
already written down.

## What exists today (2026-11-22)

- Every token context types its accounts as `InterfaceAccount<TokenAccount>`
  and its token program as `Interface<TokenInterface>`, so either program
  parses, and its mint as an `UncheckedAccount` constrained to
  `mint.owner == anchor_spl::token::ID && mint.data_len() == SPL_MINT_LEN` (82)
  with `PaymentError::Token2022Unsupported` (`Custom(6017)`).
- The CPIs are `anchor_spl::token_interface`'s (`transfer_checked` and
  `close_account`, with decimals read from the unchecked mint at byte 44), the
  ATA constraints are still SPL Token's derive, and the treasury holds one token
  account per mint, opened by `open_treasury_token_account`.
- `a_token_2022_mint_is_rejected_by_name` asserts a Token-2022 settle fails with
  `Custom(6017)`, writes no invoice or receipt and moves no tokens. While this
  plan is deferred it is the permanent boundary test, not a task to flip; a
  restart turns it into the positive case (Task F).

## Why it is not a constraint change

Dropping the owner check lets a Token-2022 mint through every layer below, and
each one changes:

1. **The split.** `settle_token_payment` pays `amount − fee_t` to the seller and
   `fee_t` to the treasury with `transfer_checked`, where `fee_t` is already
   `max(rate, flat)` and the flat is the 48-byte marker's rent in that currency
   (`helpers::fee_for`, `760053b`/`1c0600d`). The invoice closes to the treasury
   pool, not the seller. A transfer-fee mint takes its own cut in flight, so the
   seller would receive `amount − fee_t − mint fee` and the event's
   `seller_proceeds` would be a promise the token does not keep — a refusal now
   also protects a fee that already funds the record.
2. **The currency rule.** "SPL Token program" stops being the allowlist. The
   policy has to name mints (or permitted extensions) on chain, which is `Config`
   growing a field, not a rename.
3. **Accounts.** `get_associated_token_address` derives under the wrong program
   for a Token-2022 mint, on both sides of the wire: the program's ATA
   constraints, `usePaymentProgram`'s derive, the seller's setup, and the
   treasury's account. Token-2022 account sizes also vary by extension, so the
   `data_len` assumption goes.
4. **Extensions with authority over the money.** Permanent delegate, transfer
   hook, confidential transfer and mint close authority each break an assumption
   the program's accounting makes. Refusing them is a decision per extension,
   and a refused one needs its own named error like `Token2022Unsupported`.

## Tasks

### Task A: Decide the policy before the code

**Decided 2026-11-22: no extension is admitted.** A mint settles only when its
account is the base layout — owned by one of the two token programs and exactly
`SPL_MINT_LEN` (82) bytes. Every Token-2022 extension appends TLV bytes, so that
single length check is the allowlist: metadata pointer, transfer fee, permanent
delegate, transfer hook, confidential transfer and mint close authority are all
refused with `Token2022Unsupported` (`Custom(6017)`), before any CPI. A
transfer-fee mint is refused rather than charged, so `seller_proceeds` stays
`amount − fee_t` and `transfer_checked_with_fee` is never used. Because the rule
is expressible as owner + length, Task B's `Config` field is deferred until an
extension is admitted — no new account data, version bump or migration yet.

- [x] Write down which extensions a settlement may see: none.
- [x] Decide what a transfer fee does to the split: refuse the mint.
- [x] Record the decision here before any program edit.

The decision stands as the program's behaviour, but as of 2026-11-22 it is not a
route to any real Token-2022 stablecoin — see *Why it is deferred*. Any restart
begins by re-deciding this task for a named mint and its extensions. Audited
2026-11-26: the deployed constraint is actually stricter than the sentence above
— `mint.owner == anchor_spl::token::ID` in `instructions/token/settle.rs` and
`instructions/token/budget/open.rs`, so even a base-layout Token-2022 mint is
refused. The interface account/CPI types remain; the "two programs" wording is
the intended policy *after* a restart, not today's on-chain rule.

### Task B: `Config` mint policy

*(Deferred with the plan — see the header.)*

- [ ] Add the mint policy to `Config` with a space bump, its layout assertion
      (`account_spaces_match_the_sizes_used_by_the_program`) and a settle test
      that a mint the policy admits still prices and splits exactly as now.
      *(Deferred by Task A: while no extension is admitted the policy is
      `owner ∈ {token, token-2022} && data_len == SPL_MINT_LEN`, which needs no
      `Config` field. Revisit this box when the first extension is allowed.)*
- [ ] Keep `initialize_config`/`update_config` backwards-compatible for the
      deployed `Config` (realloc + version bump, or a fresh account and a
      migration step recorded in `DEV.md`). `migrate_config` already upgrades
      v1 and v2 in place. *(Stale 2026-11-26: the tree now writes version 4 —
      taken by `0007`'s network-fee fields, not by a mint policy or a shop
      policy. A mint policy would be v5 if this task is ever restarted;
      `0006`, now in `plans/deferred/`, still avoids a `Config` byte with a
      separate `ReceiptMode` PDA.)*

### Task C: Interface CPIs and the split

*(Done and kept. The two open boxes below — a mint-side fee field on
`PaymentSettled`, and a named error per refused extension — are deferred with
the rest: no extension is admitted, so there is no mint-side fee to report and
one `Token2022Unsupported` covers every refusal.)*

- [x] Move the token CPIs to `anchor_spl::token_interface`, keeping the
      `SellerPays` split identical for an SPL Token mint — 2026-11-22: the four
      transfers and the one close in `settle_token_payment`,
      `open_token_budget`, `pay_from_token_budget` and `close_token_budget` are
      `transfer_checked` / `close_account` now, with `mint_decimals` reading the
      mint's decimals. `open_treasury_token_account` has no CPI of its own (the
      ATA program creates the account), and the 57 suite tests pass unchanged.
- [ ] Emit `PaymentSettled` with the same `amount`/`fee`/`seller_proceeds`
      semantics, or a field that names a mint-side fee, so `0002`'s event note
      stays true.
- [ ] Refuse a disallowed extension by name, beside
      `Token2022Unsupported`.

### Task D: Accounts on both sides

*(Deferred with the plan — see the header.)*

- [ ] Derive ATAs with the mint's owning program in the program's constraints
      and in `apps/web` (`usePaymentProgram`, the seller's setup, the treasury
      script `pnpm treasury:open`).
- [ ] Keep the seller's account a setup step: no settle or budget path opens it
      any more (the float stopped doing so with the 48-byte marker), so
      `ensureSellerTokenAccount` is the only place a seller's ATA is created and
      it needs the mint's owning program too.
- [ ] Let the treasury hold one account per (mint, program) pair; document the
      extra step for a Token-2022 mint in `DEV.md`.

### Task E: App and API

*(Deferred with the plan — see the header.)*

- [ ] Offer a Token-2022 mint in `CURRENCY_MINTS` only once the treasury account
      exists on that cluster — the checkout already refuses otherwise, so the
      mint list stays the product switch.
- [ ] Check `apps/api`'s token handling (decimals, mint reads, the receipts it
      decodes) against a Token-2022 mint's larger account.

### Task F: Tests and devnet

*(Deferred with the plan — see the header. The existing
`a_token_2022_mint_is_rejected_by_name` stays as it is: with no migration in
flight it documents the boundary rather than guarding one.)*

- [ ] Flip `a_token_2022_mint_is_rejected_by_name` into the positive case: a
      Token-2022 mint with no extensions settles with the same split as SPL
      Token. *(Blocked on a fixture: `tests/fixtures/` holds only `spl_token.so`
      and the ATA program, so a CPI into Token-2022 has no program to run
      in-process. Dump `TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb` the way the
      other two were dumped and register it in `program_test()`.) *(Re-read
      2026-11-26: the current test uses a **base-layout** Token-2022 mint and
      expects `Custom(6017)`; the owner check refuses it before any CPI. The
      "over-length mint instead, which will be admitted" suggestion is obsolete
      while the deferral keeps the SPL-Token-only owner constraint.)*
- [ ] One test per refused extension, each asserting its named error and no
      movement of tokens.
- [ ] A transfer-fee mint test if Task A admits that extension; otherwise it is
      a refusal test.
- [ ] Devnet: upgrade in place (extend `ProgramData` first if the ELF grows),
      open a treasury account for the test mint, settle one invoice through
      `pnpm smoke:silent-payment`.

## Acceptance criteria

*(Deferred with Tasks B/D/E/F; only the third applies today, and the unchanged
suite proves it.)*

- [ ] A Token-2022 mint the policy admits settles with the split the event
      reports.
- [ ] Every refused extension fails with its own error, before any transfer.
- [ ] An SPL Token settlement is byte-for-byte the same instruction as today
      (same accounts, same split), proven by the existing suite passing
      unchanged.
- [ ] `README.md` and `DEV.md` state the supported programs and extensions
      instead of "SPL Token only".

## Verification

*(Task C's run is recorded in the header. Re-checked 2026-11-26: after a fresh
`cargo build-sbf`, `cargo test --test settle` passes 74/74 and `cargo fmt --check`
is clean; clippy still fails, now on six test lints listed in the header. IDL was
regenerated for later work, so it is no longer byte-identical to the Task C run.
The list below is what a restart repeats.)*

- [ ] `cargo build-sbf --manifest-path programs/payvand/Cargo.toml`, then
      `cargo test --test settle` in `apps/contracts` (never `anchor test` here).
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
- [ ] `anchor idl build` regenerated and copied to
      `apps/web/app/assets/payvand.idl.json`.
- [ ] In `apps/web`: `vitest run`, `nuxt typecheck`, `eslint .`.
- [ ] Devnet smoke run recorded here.

## Dependencies

`0002` Task 4.4 (the named rejection and the interface account types) and
`0002` Task 4.3 (the events whose split a Token-2022 settle must keep truthful).

## Estimated scope

Large if it restarts: it touches `Config`, three token instructions, the ATA
derivation on both sides, the treasury script, the mint list, and the harness.
As shipped (Task C only) it was one file plus one helper: the five CPIs, the
account types, and `helpers::mint_decimals`.
