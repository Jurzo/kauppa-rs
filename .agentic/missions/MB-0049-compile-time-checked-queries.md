# Mission Brief — Decide on compile-time-checked SQL, and do it or retire the plan

```yaml
---
id: MB-0049
owner: "@juuso-kalliomaki"
goal: "`persistence.md`'s stated target (queries checked against the schema at compile time) is either implemented with an offline cache that CI verifies, or removed from the pack by decision, so the document and the code agree."
non_goals:
  - "Changing any query's behaviour or the schema."
  - "Switching database or ORM."
  - "Fixing the SQLite advisory (MB-0041), though the sqlx version matters to both."
acceptance:
  - id: P1
    property: "An RR records the decision: migrate to `sqlx::query!`/`query_as!` with `.sqlx/` offline data, migrate partly (which modules), or keep runtime queries and delete the target-state paragraph."
    check: "manual: the RR linked in MB-0049's Continuity Pack"
  - id: P2
    property: "Every query in `repo/sqlite/user.rs`, `category.rs`, `invitation.rs`, `recipe.rs` and `product.rs` (51) is compile-time checked, `.sqlx/` is committed, and CI builds with `SQLX_OFFLINE=true` and fails when the cache is stale (RR-0037 sub-answers 1 and 2)."
    check: "scripts/sqlx-prepare.sh --check"
  - id: P3
    property: "The list bypass guard and every repo test pass unchanged; each converted query keeps its SQL byte for byte apart from type-override aliases; `list.rs` and `mod.rs` are listed as runtime by decision (RR-0037), with the reason."
    check: "cargo test -p kauppa-core"
  - id: P4
    property: "`persistence.md` describes what the code does (RR roll-back)."
    check: "manual: the roll-back applied, recorded in MB-0049's Continuity Pack"
context:
  - .agentic/mentorship/persistence.md
  - crates/kauppa-core/src/repo/sqlite/
  - crates/kauppa-core/Cargo.toml
  - .github/workflows/ci.yml
  - .gitignore
  - .agentic/missions/MB-0041-sqlite-wal-advisory.md
autonomy:
  may_decide:
    - "the order modules are converted in"
    - "how the stale-cache check runs in CI"
  escalate:
    - { when: "converting `list.rs` or `mod.rs`, a sqlx version change, `sqlx-cli`, or any `Cargo.toml` change", to: platform-owner }
    - { when: "any query whose text must change to fit the macros", to: data-owner }
    - { when: "the Mentorship Pack edit", to: owner }
  forbidden:
    - "change query semantics"
    - "edit committed migrations"
    - "build SQL with `format!` from user input"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/persistence.md@v1
resolutions: [RR-0037]
---
```

## Context

Found in the audit of 2026-11-10 (finding 19).

`persistence.md` says the scaffold uses runtime `sqlx::query(...)` and that
the target is `sqlx::query!` with a `cargo sqlx prepare` cache and
`SQLX_OFFLINE=true` in CI: "Track in a Mission Brief." No brief tracked it.
The repo layer has grown to about 2,700 lines of SQLite code with runtime
queries, checked by tests only; `.gitignore` already has a `.sqlx/` entry.
A column rename today is caught at test time, not at compile time.

## Conceptual plan

1. **Clarify (S1).** CRP to platform-owner with a trial on one module
   (build time, CI changes, how the central list predicate fits a macro).
   Answered by RR-0037 (CRP-0037, all as recommended): convert the five
   modules, keep `list.rs` and `mod.rs` runtime, a repository script for
   the cache, on sqlx 0.8.
2. Write `scripts/sqlx-prepare.sh` and the CI step, then convert one module
   per commit, regenerating `.sqlx/` each time.

## Validation plan

- The trial's numbers in the CRP.
- `scripts/sqlx-prepare.sh --check` and the offline build in CI.
