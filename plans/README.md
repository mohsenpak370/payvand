# Implementation Plans

This directory tracks implementation plans for multistep Payvand work.

Plans translate requirements into small, verifiable tasks. They are not
product specs and should not introduce product policy that is missing from
`README.md` (and `DEV.md` for local, gitignored notes).

## When To Create A Plan

Create or update a plan when work:

- Touches multiple files or subsystems (web app, API, and the on-chain
  program at once).
- Implements a user-facing feature or behavior change — payments, the QR
  flow, discount codes, the monthly budget, accounts.
- Changes the Anchor program, its IDL, PDA derivation, cluster
  configuration, or the API's database schema.
- Needs multiple implementation sessions or could be parallelized.

Small mechanical edits do not need a plan.

## Plan Lifecycle

Use these statuses:

- `Draft`: Proposed task breakdown, not approved for implementation.
- `Approved`: Ready to implement.
- `In Progress`: Implementation has started.
- `On hold`: Partly implemented and paused, with a current intent to resume when
  a named trigger fires; tasks already done stay done.
- `Deferred`: Not currently intended for implementation. It may be a proposal
  waiting on traffic (`0006`) or a partly shipped plan parked until the product
  asks for it again (`0003`). These plans live in [`deferred/`](deferred/) and
  keep their decisions and shipped work recorded without occupying the active
  queue.
- `Completed`: Tasks and verification are complete.
- `Superseded`: Replaced by another plan.
- `Cancelled`: No longer intended.

## File Naming

Use sequential filenames:

```text
0001-local-cluster-workflow.md
0002-monthly-buyer-budget.md
0003-discount-codes.md
```

Start a new plan by copying `plans/templates/plan-template.md` to the next
sequential filename. Templates live in `plans/templates/`.

## Maintenance Rules

- Break work into small tasks with acceptance criteria and verification
  steps.
- Mark blocked tasks clearly instead of inventing missing requirements.
- Update a plan when scope changes during implementation.
- Run `task verify` (lint, typecheck, API tests, and the contract suite
  including `anchor test`) as the standard verification bar named in plan
  tasks.
- Update `README.md` when a completed plan changes documented behavior.
- Move completed plan files into `plans/archive/` and parked or trigger-gated
  plans into `plans/deferred/`; keep plans with current implementation intent at
  the top level.
- Keep the active, deferred, and archived tables below up to date.

## Active Plans

| ID | Plan | Status |
| ---- | ---- | -------- |
| 0004 | [Relayer and sponsored fees (gasless payments)](0004-relayer-sponsored-fees.md) | In Progress |
| 0007 | [SOL sponsored fees](0007-sol-sponsored-fees.md) | In Progress |

The 2026-11-09 contract review originally lived in one plan. It was split on
2026-11-12: `0001` ships the theft, replay, and status fixes, and `0002` builds
the fee model on top of it. Implement `0001` first; `0002` assumes it is
deployed.

Re-audited 2026-11-22 against the working tree. `0002`'s fee model has shipped,
but the single `fee_bps` and the 301-byte receipt it describes were replaced
after its last revision by the tiered `FeePolicy` (`1c0600d`) and the 48-byte
marker (`760053b`); its new *Current State* section is the live model, and the
older tables are marked historical. `0003`–`0005` all dated their "what exists
today" sections from revision 14–17 of `0002`; their stale counts and `Config`
facts were corrected in the 2026-11-26 audit below. `0006` designs unbuilt work
and was moved to [`deferred/`](deferred/) that day.

On 2026-11-22 `0003` moved from `In Progress` to `On hold`; on 2026-11-26 it
moved again to **Deferred** after the decision that there is no current intent to
implement Token-2022 support. USDC and EURC are both classic SPL Token mints on
both clusters, so the `token_interface` migration changes nothing the product
offers; and the Token-2022 stablecoins worth accepting (PYUSD, USDG) all carry
extensions, which `0003`'s decided policy (owner plus the 82-byte base layout)
refuses. Its Task C — the interface CPIs, behaviour-identical and verified —
stays in place, because it is what makes the SPL Token refusal the named
`Token2022Unsupported`. Tasks B/D/E/F are parked; the plan restarts only when a
Token-2022 mint is actually wanted in `CURRENCY_MINTS`, and starts by re-deciding
the extension policy.

Audited again on 2026-11-22 with the same question — *is this shipped, with only
checking left?* — and `0002` was archived, because its fee model, treasury,
budgets and events all shipped and what remains is verification (see
[`REMAINING.md`](REMAINING.md)). `0003`–`0005` were **not** archived: `0003` was
parked on purpose, while `0004` and `0005` still had building left. Those
specifics are historical; the 2026-11-26 audit below is current.

`0004` began on 2026-11-22 with an in-house API-held sponsor key for token
invoices only, fronts both record rents and a refund to the sponsor; the
zero-lamport harness case and the byte-checked `POST /api/sponsor/settle` rebuild
shipped. Its original below-threshold, signed-in-only scope was later
superseded: `0007` added SOL sponsorship and the checkout now asks the relayer
first for both currencies, with wallet-keyed anonymous caps.

`0005` also began on 2026-11-22, aimed at a per-shop multiplier rather than a
second policy: a `ShopPolicy` PDA seeded by the seller and a key the API stores
beside its row, `0..=10_000` set by the seller, scaling `Config`'s two rates and
leaving the boundaries and the token flats shared. That shape needs no `Config`
field, so it takes no version bump. The setter and the optional accounts on all
four settle paths shipped, then were removed again on 2026-11-27 when the plan
was cancelled: the seller pays the fee, so a shop rate only cut the protocol's
own revenue and never changed the buyer's price, and nothing in the product ever
named a policy. See [`archive/0005-per-shop-fees.md`](archive/0005-per-shop-fees.md).

Re-audited 2026-11-26 against `92010cb`: `0003` moved from **On hold** to
**Deferred** (no current intent to implement Token-2022; the on-chain owner check
is even stricter than its policy text — SPL Token only); `0004` remains **In
Progress** because its API-endpoint devnet run, deployed sponsor key,
client refusal text, SOL cost panel and copy are still owed; `0005` was **In
Progress** at that audit and was cancelled and removed on 2026-11-27; `0007` was
added to the active table —
its program, relayer and checkout code plus the devnet runs are done, but the
same deployment/copy/verification items and its archive step remain. A fresh
`cargo build-sbf` followed by `cargo test --test settle` passes 74/74; clippy
still fails on six test lints. `0003` and `0006` were moved to `plans/deferred/`
as described below. Details and remaining checkboxes are in each plan and in
[`REMAINING.md`](REMAINING.md).

## Deferred Plans

| ID | Plan | Status | Trigger |
| ---- | ---- | -------- | ------- |
| 0003 | [Token-2022 migration (`token_interface`)](deferred/0003-token-interface-migration.md) | Deferred | A named Token-2022 mint is actually wanted |
| 0006 | [Compressed payment receipts (4 sharded trees + rotation)](deferred/0006-compressed-receipts.md) | Deferred | ~700,000 expected settlements |

`0003` moved here on 2026-11-26 after the decision not to implement Token-2022
support for now. Its Task C is shipped and stays in place; Tasks B/D/E/F are
parked. Restart only when a named mint is wanted in `CURRENCY_MINTS`, and start by
re-deciding the extension policy — the current owner check refuses even a
base-layout Token-2022 mint.

`0006` moved here on 2026-11-26 after an audit found no implementation: its
decisions and the README's "planned, not shipped" note are recorded, but none of
Phase 1–5 has started. **It makes sense only when traffic justifies it**: the
48-byte marker stays cheaper in total below roughly 700,000 settlements, even
though rent alone breaks even at ~7,700–8,000. Even then, the compatible
compression program gate and the replay-rule reconciliation in the plan must
pass first.

## Archived Plans

Completed plans are archived in `plans/archive/` to keep active planning
easy to scan while preserving implementation history. A plan is archived when
its design and code have shipped and only checking is left; what it still owes
is listed in [`REMAINING.md`](REMAINING.md), so the archived file does not have
to be reopened for it.

| ID | Plan | Status |
| ---- | ---- | -------- |
| 0001 | [Contract Security Hardening](archive/0001-contract-security-hardening.md) | Completed |
| 0002 | [Fee Model, Treasury, and Budget Accounting](archive/0002-fee-model-and-treasury.md) | Completed |
| 0005 | [Per-shop fee overrides](archive/0005-per-shop-fees.md) | Cancelled |
