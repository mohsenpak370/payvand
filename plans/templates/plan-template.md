# Plan: Feature Name

## Status

Draft

## Linked Docs

- `README.md` (relevant section)
- `plans/...` (related plans)
- `DEV.md` (local notes, if relevant)

## Objective

Describe the outcome this plan will deliver in one short paragraph.

## Scope

In scope:

- Item 1.

Out of scope:

- Item 1.

## Dependencies

- Product decisions:
- Technical decisions:
- Open questions:

## Issues Addressed

List the bugs, review findings, or requirements this plan closes. Link to the
review or issue where possible.

| ID | Issue | Severity | Phase |
| --- | --- | --- | --- |
| 1 | Short description | High | 1 |

## Phases

### Phase 1: Foundation

#### Task 1: Short Task Name

Description: Explain what this task accomplishes.

Acceptance criteria:

- [ ] Specific, testable condition.
- [ ] Specific, testable condition.

Verification:

- [ ] Tests pass: `command here`.
- [ ] Manual check: describe the behavior to verify.

Files likely touched:

- `path/to/file`

Dependencies: None.

Estimated scope: Small.

## Checkpoints

- [ ] After foundation tasks, `task verify` passes and docs are updated.
- [ ] After integration tasks, affected user flows work end-to-end against a
      local or devnet cluster.
- [ ] Before completion, the `README.md` sections affected by the change are
      updated.

## Risks And Mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Risk description | Medium | Mitigation description |

## Rollout / Deployment

Describe the deployment order, feature flags, migrations, and backward
compatibility. For on-chain changes, include the program upgrade, IDL update,
API deploy, and client deploy order.

- [ ] Program / migration:
- [ ] API / data:
- [ ] Client:
- [ ] Smoke tests:

## Definition Of Done

- [ ] All acceptance criteria are met.
- [ ] `task verify` passes.
- [ ] The affected docs are updated.
- [ ] The plan is moved to `plans/archive/` and listed in `plans/README.md`.

## Open Questions

- Question that must be answered before implementation or launch.
