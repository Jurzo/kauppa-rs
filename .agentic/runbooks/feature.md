# Workflow Runbook — Feature

Version: v1 · Owner: @juuso-kalliomaki · Applies to: PRs labeled `agent-authored` under task class `feature`.

Every stage below has deterministic triggers and gates. CI mirrors them in
`.github/workflows/agentic-gate.yml`.

## Entry gate

Before any file is edited:

1. A Mission Brief exists at `.agentic/missions/MB-<id>-<slug>.md` with
   non-empty `goal`, `non_goals`, `acceptance[]`, and `autonomy` fields that
   conform to `.agentic/schemas/mission-brief.schema.json`.
2. The brief's `mentorship` and `runbook` references point at files that
   exist at the referenced revision.
3. The baseline compiles: `cargo check --workspace --all-targets` is green on
   the branch head.

**Stop rule:** if the entry gate fails, do not edit code. Open a
Consultation Request Pack describing the missing input.

## Stages

### S1 — Clarify

Read the brief, read referenced mentorship, read linked code paths in
`context`. Confirm acceptance properties are testable in this repo.
Checkpoint: brief is unchanged OR you have proposed a brief delta via consult.

### S2 — Plan

Write the conceptual plan into the brief body (not a step list — strategy,
checkpoints, slow-mode triggers). Identify forbidden paths from
`autonomy.forbidden` and the escalation-trigger paths below.

Checkpoint: plan references real file paths and real repo state.

### S3 — Implement

Edit code and tests. Respect `autonomy.forbidden`. For every acceptance
property, add or extend a test that evaluates it.

Checkpoint: `scripts/agentic/fast-gate.sh` passes locally.

### S4 — Verify

Run the full gate set (see "Deterministic checks" below).

Checkpoint: all gates green; no warnings suppressed.

### S5 — Package

Assemble the Merge-Readiness Pack as the PR description from
`.github/pull_request_template.md`. Fill the scope-to-proof map, link the
Mission Brief, link any Resolution Records.

Checkpoint: every acceptance property ID in the brief appears in the
scope-to-proof map.

## Deterministic checks

| Gate | Command | When |
| --- | --- | --- |
| Format | `cargo fmt --all -- --check` | S3, S4, CI |
| Lint | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | S4, CI |
| Build | `cargo check --workspace --all-targets` | S3 (fast), S4 |
| Unit tests | `cargo test --workspace --all-targets` | S4, CI |
| Dependency hygiene | `cargo deny check` | CI |
| Artifact schemas | `scripts/agentic/validate-artifacts.sh` | CI |
| Evidence completeness | `scripts/agentic/check-ready.sh` | CI (if `agent-authored`) |

## Escalation triggers

Any PR touching the paths below without a linked, approved Resolution Record
with status `approved` **fails CI** and requires a Consultation Request Pack
first.

| Path | Routed to |
| --- | --- |
| `migrations/**` | @juuso-kalliomaki (data) |
| `crates/kauppa-core/src/password.rs` | @juuso-kalliomaki (security) |
| `crates/kauppa-web/src/routes/auth.rs` | @juuso-kalliomaki (security) |
| Session middleware in `crates/kauppa-web/src/main.rs` | @juuso-kalliomaki (security) |
| `Cargo.toml`, `crates/*/Cargo.toml` dependency changes | @juuso-kalliomaki (platform) |
| `rust-toolchain.toml`, `rustfmt.toml`, `deny.toml` | @juuso-kalliomaki (platform) |
| `.agentic/{mentorship,runbooks,resolutions}/**` | @juuso-kalliomaki |

## Required outputs

A merged feature PR must have:

- A Mission Brief in `.agentic/missions/`.
- A Merge-Readiness Pack in the PR body (fills the scope-to-proof map).
- A Resolution Record in `.agentic/resolutions/` **only if** an escalation
  trigger fired or a consult was raised.
- Green CI (`ci.yml` and `agentic-gate.yml`).

## Stop conditions

- Tests fail after three consecutive attempts to fix → open a consult.
- Escalation trigger fired without an approved RR → stop, open a consult.
- The agent cannot articulate why a change is correct → stop, open a consult.
