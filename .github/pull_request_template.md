<!--
Merge-Readiness Pack (Table 5, MVP tier).

This template is required for every PR labeled `agent-authored`. Human PRs
should still fill it when the change is non-trivial.

Keep this section visible as a self-review checklist — reviewers audit this,
not the full diff.
-->

## Mission Brief

- Brief: `.agentic/missions/MB-<id>-<slug>.md`
- Runbook: `.agentic/runbooks/<name>.md@<version>`

## Scope-to-proof map

| Acceptance ID | Property | Check | Evidence |
| --- | --- | --- | --- |
| P1 |  |  |  |

Every `acceptance[].id` from the Mission Brief MUST appear in this table.

## Verification bundle

- Tests run: `cargo test --workspace --all-targets`
- New tests added:
- Edge / failure cases covered:
- Logs or traces (if applicable):

## Engineering hygiene

- [ ] `cargo fmt --all -- --check` clean
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean
- [ ] `cargo deny check` clean
- [ ] Change set is coherent (one mission, no drive-by refactors)

## Rationale and trade-offs

What was done and why. Alternatives considered and rejected.

## Risk and rollout plan

- Blast radius:
- Backward compatibility:
- Rollout steps:
- Rollback plan:
- Open risks:

## Escalations

- Trigger paths touched (migrations, auth, deps, …):
- Linked Resolution Records: `.agentic/resolutions/RR-…`
- Consults opened during the mission: `.agentic/consults/CRP-…`

<!--
Hardened-tier sections (plan ledger, exploration archive, machine-readable
manifest) are deferred until agent volume justifies them. See the baseline.
-->
