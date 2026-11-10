# Mission Brief — Pin and maintain CI: actions, toolchain, dependency updates, unused checks

```yaml
---
id: MB-0045
owner: "@juuso-kalliomaki"
goal: "CI runs on pinned inputs that are updated on purpose: actions by commit, one Rust version for tests and the image, automated update proposals, least-privilege tokens, and every check script the repo has."
non_goals:
  - "New product tests (MB-0047 covers a browser test)."
  - "Changing lint levels."
  - "Fixing the SQLite advisory (MB-0041)."
acceptance:
  - id: P1
    property: "Every `uses:` in `.github/workflows/` is pinned to a full commit SHA with the version in a comment, and each workflow declares `permissions:` with the least it needs; a check fails on a tag-pinned action or a workflow without `permissions:`."
    check: "scripts/agentic/check_workflows.sh"
  - id: P2
    property: "`rust-toolchain.toml` names one exact Rust version, CI uses it, and it equals the version in `deploy/Dockerfile`'s `RUST_IMAGE`; a check fails when they differ."
    check: "scripts/agentic/check_workflows.sh"
  - id: P3
    property: "An update bot (Dependabot or Renovate, as the RR decides) proposes updates for GitHub Actions, cargo dependencies and the pinned Docker image digests (Rust, Debian, Caddy, Alpine), grouped so the PR volume is manageable, and its PRs fit the agentic gate (label and RR rules as the RR decides)."
    check: "manual: the bot's first PRs recorded in MB-0045's Continuity Pack"
  - id: P4
    property: "CI runs `scripts/agentic/check_brief_tests.sh` and `scripts/agentic/test_check_triggers.sh` on pull requests, and `scripts/deploy/backup_restore_drill.sh` where the image job already has Docker, or the RR records why one stays manual."
    check: "manual: the CI run of each script linked in MB-0045's Continuity Pack"
  - id: P5
    property: "`deny.toml`'s `skip` list holds only crates that still have duplicate versions (`cargo deny check` reports no `unnecessary-skip`), with comments that match the current tree."
    check: "cargo deny check"
context:
  - .github/workflows/ci.yml
  - .github/workflows/agentic-gate.yml
  - .github/workflows/deploy.yml
  - rust-toolchain.toml
  - deploy/Dockerfile
  - deploy/compose.yaml
  - deploy/backup/Dockerfile
  - deny.toml
  - scripts/agentic/
  - .agentic/runbooks/dependency-bump.md
autonomy:
  may_decide:
    - "the check script's name and implementation"
    - "job layout and caching"
    - "update grouping and schedule inside what the RR allows"
  escalate:
    - { when: "pinning the toolchain (`rust-toolchain.toml` is a trigger path) and who bumps it", to: platform-owner }
    - { when: "which update bot, and how its PRs pass the gates (they touch `Cargo.toml`/`Cargo.lock`; the dependency-bump runbook needs an RR per bump today)", to: platform-owner }
    - { when: "any `deny.toml` change (trigger path)", to: platform-owner }
    - { when: "any Runbook change", to: owner }
  forbidden:
    - "lower lint strictness or drop a check"
    - "grant a workflow write permissions it doesn't need"
    - "add an advisory ignore"
runbook: .agentic/runbooks/dependency-bump.md@v1
mentorship:
  - AGENTS.md@v1
resolutions: []
---
```

## Context

Found in the audit of 2026-11-10 (findings 6, 9, 12 and 14).

- Actions are pinned by tag (`actions/checkout@v4`,
  `dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2`,
  `EmbarkStudios/cargo-deny-action@v2`, `docker/*`, `hashicorp/setup-terraform@v3`).
  `ci.yml` and `agentic-gate.yml` have no `permissions:` block.
- `rust-toolchain.toml` says `channel = "stable"`, and clippy runs with
  pedantic and nursery as errors, so a new stable release can fail CI on an
  unrelated PR. The image is built with `rust:1.99.1`, a different compiler
  than the one that ran the tests once stable moves on.
- Nothing proposes updates. Image digests (Rust, Debian, Caddy, Alpine) and
  crates only change when someone remembers.
- `check_brief_tests.sh`, `test_check_triggers.sh` and
  `backup_restore_drill.sh` exist but no workflow runs them.
- `deny.toml`'s skip list cites `walkdir`, removed by MB-0032, and
  `cargo deny` warns about unnecessary skips; the file asks for a quarterly
  prune.

## Conceptual plan

1. **Clarify (S1).** CRP to platform-owner: toolchain pin, update bot and
   its route through the gates, required-check changes (with MB-0043).
2. The check script and workflow edits; the toolchain pin; the bot's
   configuration; the `deny.toml` prune.
3. Watch the first bot PRs and adjust grouping.

## Validation plan

- P1, P2: the new script, locally and in CI.
- P3, P4: links to CI runs and bot PRs.
- P5: `cargo deny check` output without warnings about skips.
