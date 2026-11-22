# What Is Left

Active plans keep their own checkboxes. This file exists so an **archived** plan
does not have to be reopened to see what it still owes: a plan is archived once
its design and code have shipped, and what remains is checking, not building.

## Archived `0002` — Fee Model, Treasury, and Budget Accounting

Archived 2026-11-22 ([archive/0002-fee-model-and-treasury.md](archive/0002-fee-model-and-treasury.md)).
The fee model, the treasury, the budgets, the events and the client estimates
shipped, and `0003`–`0006` came out of it. What is left is verification:

- [ ] `task verify` (lint, typecheck, API tests, the contract suite) run against
      the current tree and recorded in the archived plan's revision notes.
- [ ] `cargo clippy -p payvand --all-targets -- -D warnings`. It currently fails
      on three lints in `apps/contracts/programs/payvand/tests/settle.rs` —
      `proportional.max(TEST_FIXED_FEE)` and the two constant assertions on
      `FLATS_REFRESH_SECS` / `PRICE_ANCHOR_BAND_BPS` — all from the fee-anchor
      work that landed after the plan's last revision.
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

## Still Active

These were checked on 2026-11-22 and are **not** archived — each has building
left, not only checking:

- `0003` — On hold by decision. Task C (interface CPIs) shipped; Tasks B, D, E
  and F wait on a Token-2022 mint the product actually wants, and start by
  re-deciding the extension policy.
- `0004` — In Progress. The program's `sponsor` account, its harness tests, the
  `POST /api/sponsor/settle` endpoint and the checkout's sponsored send (with its
  funded-wallet fallback test) shipped. Left: the devnet run, and letting a
  drained session float borrow the sponsor's fee — which needs the same
  refund-to-sponsor account on `pay_from_token_budget`.
- `0005` — In Progress. The `ShopPolicy` type, `fee_for`'s multiplier, the
  seller-signed `set_shop_policy` with its `0..=10_000` write cap, the optional
  policy account on the four settle paths (all four covered by the harness:
  wallet and allowance, SOL and token), and the relayer's rebuild carrying the
  policy as its trailing account shipped. Left: the `PaymentSettled` field, a
  settlement through a policy written at the cap, and the API/client side that
  stores the policy address and passes it to the quote, the builder and
  `POST /api/sponsor/settle`.
