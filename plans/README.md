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
- `On hold`: Partly implemented and paused until a named trigger; tasks already
  done stay done, and the plan records what would restart it.
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
- Move completed plan files into `plans/archive/` and keep active,
  draft, in-progress, superseded, or cancelled plans at the top level.
- Keep the active and archived tables below up to date.

## Active Plans

| ID | Plan | Status |
| ---- | ---- | -------- |
| 0003 | [Token-2022 migration (`token_interface`)](0003-token-interface-migration.md) | On hold |
| 0004 | [Relayer and sponsored fees (gasless payments)](0004-relayer-sponsored-fees.md) | In Progress |
| 0005 | [Per-shop fee overrides](0005-per-shop-fees.md) | In Progress |
| 0006 | [Compressed payment receipts (4 sharded trees + rotation)](0006-compressed-receipts.md) | Draft |

The 2026-11-09 contract review originally lived in one plan. It was split on
2026-11-12: `0001` ships the theft, replay, and status fixes, and `0002` builds
the fee model on top of it. Implement `0001` first; `0002` assumes it is
deployed.

Re-audited 2026-11-22 against the working tree. `0002`'s fee model has shipped,
but the single `fee_bps` and the 301-byte receipt it describes were replaced
after its last revision by the tiered `FeePolicy` (`1c0600d`) and the 48-byte
marker (`760053b`); its new *Current State* section is the live model, and the
older tables are marked historical. `0003`–`0005` all dated their "what exists
today" sections from revision 14–17 of `0002` and now describe the current
`Config`, `fee_for` and rents instead. `0006` designs unbuilt work and was
already current; only its mainnet/devnet rent labelling was wrong.

On 2026-11-22 `0003` moved from `In Progress` to **On hold**. USDC and EURC are
both classic SPL Token mints on both clusters, so the `token_interface`
migration changes nothing the product offers; and the Token-2022 stablecoins
worth accepting (PYUSD, USDG) all carry extensions, which `0003`'s decided
policy (owner plus the 82-byte base layout) refuses. Its Task C — the interface
CPIs, behaviour-identical and verified — stays in place, because it is what makes
the SPL Token refusal the named `Token2022Unsupported`. Tasks B/D/E/F are
deferred; the plan restarts when a Token-2022 mint is actually wanted in
`CURRENCY_MINTS`, and starts by re-deciding the extension policy.

Audited again on 2026-11-22 with the same question — *is this shipped, with only
checking left?* — and `0002` was archived, because its fee model, treasury,
budgets and events all shipped and what remains is verification (see
[`REMAINING.md`](REMAINING.md)). `0003`–`0005` were **not** archived: `0003` is
parked on purpose, `0004` still owes its devnet run and `0005` still owes the
setter and the settle-path account, so each keeps its own checkboxes.

`0004` began on 2026-11-22: its sponsor model is decided (in-house key in the
API, token invoices only, the sponsor fronts both record rents and takes the
treasury's refund of them), `settle_token_payment` now carries a `sponsor`
account, the harness covers the zero-lamport buyer, `POST /api/sponsor/settle`
rebuilds and byte-checks the one message it will sign, and the checkout asks for
that sponsor only when the wallet cannot front the fee and rents. What is left is
the devnet run and the session-key case.

`0005` also began on 2026-11-22, aimed at a per-shop multiplier rather than a
second policy: a `ShopPolicy` PDA seeded by the seller and a key the API stores
beside its row, `0..=10_000` set by the seller, scaling `Config`'s two rates and
leaving the boundaries and the token flats shared. That shape needs no `Config`
field, so it takes no version bump and the race with `0003` is gone. The
arithmetic and the account type are in; the instruction that writes one, the
optional account on the settle paths and the API/client side are next.

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
