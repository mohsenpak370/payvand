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
| 0001 | [Contract Security Hardening](0001-contract-security-hardening.md) | Draft |

## Archived Plans

Completed plans are archived in `plans/archive/` to keep active planning
easy to scan while preserving implementation history.

| ID | Plan | Status |
| ---- | ---- | -------- |
| | | |
