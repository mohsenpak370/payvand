# What Is Left

Active and deferred plans keep their own checkboxes; this file is the short
cross-plan index. An **archived** plan is not reopened to see what it still owes:
its design and code have shipped, and what remains is checking, not building.

Re-audited 2026-11-26 against the working tree at `92010cb`. After a fresh
`cargo build-sbf`, `cargo test --test settle` passes **74/74** and `cargo fmt
--check` is clean. `cargo clippy -p payvand --all-targets -- -D warnings` still
fails on six test lints; `task verify` is not recorded end to end, and CI does not
run web vitest or API tests.

## Archived `0002` — Fee Model, Treasury, and Budget Accounting

Archived 2026-11-22 ([archive/0002-fee-model-and-treasury.md](archive/0002-fee-model-and-treasury.md)).
The fee model, the treasury, the budgets, the events and the client estimates
shipped, and `0003`–`0006` came out of it. What is left is verification:

- [ ] `task verify` (lint, typecheck, API tests, the contract suite) run against
      the current tree and recorded in the archived plan's revision notes.
- [ ] `cargo clippy -p payvand --all-targets -- -D warnings`. It currently fails
      on **six** lints in `apps/contracts/programs/payvand/tests/settle.rs`:
      `manual_clamp` and the old `max` suggestion around :1405, the two constant
      assertions on `FLATS_REFRESH_SECS` / `PRICE_ANCHOR_BAND_BPS` around :3147,
      plus `clone_on_copy` around :4457 and `redundant_locals` around :4469. The
      first four come from the fee-anchor work; the last two are from later tests.
- [ ] `anchor test`'s full form. It upgrades devnet to prove what the harness
      already proves; either run it once and record the result, or record the
      decision to retire it in favour of `cargo test --test settle`.
- [ ] Task 5.2 leftovers: close the one SOL budget still open (`fQadYNyE…`) and
      record the signature, and do the browser click-through of `/budget` that
      was blocked on a missing wallet and session key.
- [ ] The manual localnet checks the repo never runs: 1.4 (config TTL refresh),
      1.5 (the fee note on a settled invoice), 2.3 (a drained float and the UI
      warning) and 4.2 (close/rotate and the invoice flows in a browser).
- [ ] The Definition of Done applied to the current model. Several bullets still
      describe the pre-tier `fee_bps` and the 301-byte receipt and need the same
      "historical" treatment *Current State* gives the tables.
- [ ] One open question to settle: the initial treasury funding amount, and the
      alert threshold before a top-up is needed.

## Deferred

- `0003` — Moved to [`deferred/0003-token-interface-migration.md`](deferred/0003-token-interface-migration.md)
  on 2026-11-26 after the decision not to implement Token-2022 support for now.
  Task C (the interface CPIs) shipped and stays in place; Tasks B/D/E/F are
  parked. Restart only when a named Token-2022 mint is wanted in `CURRENCY_MINTS`,
  starting with a re-decision of the extension policy — the current owner check
  refuses even a base-layout Token-2022 mint.
- `0006` — Moved to [`deferred/0006-compressed-receipts.md`](deferred/0006-compressed-receipts.md)
  on 2026-11-26. Nothing is built; its decisions are recorded and it should be
  picked up only when the product expects ~700,000 settlements, at which point
  the compatibility gate and the nullifier-rule reconciliation in the plan are
  also prerequisites. The 48-byte marker remains the cheaper choice below that
  traffic level.

## Still Active

These were re-audited on 2026-11-26 and are **not** archived — each has building
or proof left, not only checking:

- `0004` — In Progress. The program's `sponsor` account, harness tests,
  `POST /api/sponsor/settle`, caps and checkout's sponsored send shipped; `0007`
  extended them to SOL and made sponsorship anonymous. Left: a settle through the
  deployed API endpoint, provisioning/documenting `SPONSOR_KEYPAIR` and caps,
  a refusal naming the actual `needs_lamports`, the SOL cost panel showing two
  signatures, stale "payer needs SOL" copy, and a per-seller cap decision. The
  balance gate and drained session-float borrow are decided away.
- `0007` — In Progress after the 2026-11-26 audit. Program upgrades, the v4
  network-fee fields, the relayer's SOL rebuild and all three devnet paths are
  done. Left: deployed API key and endpoint smoke, the SOL cost panel/quote,
  stale copy in README/DEV/learn/i18n, the legacy one-signer Solana Pay builder,
  full verification, and the archive step. Its open question about reporting the
  fee payer on an externally built settle is unresolved.

## Cancelled

- `0005` — Cancelled and archived 2026-11-27. The per-shop fee multiplier was
  built on chain and in the relayer, never reached the product, and has been
  removed end to end (program, IDL, relayer request, web composables, tests and
  docs); `InvalidNetworkFee` keeps `Custom(6023)`. See
  [`archive/0005-per-shop-fees.md`](archive/0005-per-shop-fees.md).
