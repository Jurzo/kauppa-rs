# Resolution Record — CI pinning and maintenance: SHA-pinned actions, one Rust version, Dependabot under a standing RR, every check in CI

```yaml
---
id: RR-0032
closes: CRP-0032
mission: MB-0045
decided_by: "@Jurzo"
role: platform
risk_tier: medium
decided_at: 2026-11-12T14:43:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/runbooks/dependency-bump.md
  - .agentic/missions/MB-0045-ci-supply-chain-and-toolchain.md
  - .agentic/consults/CRP-0032-ci-pinning-and-maintenance.md
---
```

## Decision and outcome

Approve CRP-0032's **recommended set, Q1–Q6**.

Sub-answers:

1. **Pins and permissions (Q1).**
   - Every `uses:` in `.github/workflows/` becomes
     `owner/action@<40-hex SHA>  # vX.Y.Z`, at the commit its current
     major tag points to (CRP-0032 Evidence). No major-version upgrade in
     this mission.
   - `ci.yml` and `agentic-gate.yml` gain a top-level
     `permissions: contents: read`. `deploy.yml` keeps its permissions.
   - New `scripts/agentic/check_workflows.sh` fails on: a non-local
     `uses:` that isn't a 40-hex SHA with a `# v…` comment; a workflow
     without a top-level `permissions:`; any `write-all`, or a `write`
     permission not on its allowlist. The allowlist has one entry today:
     `packages: write` on `deploy.yml`'s `image` job. The script tests
     itself against bad fixtures, as `test_check_triggers.sh` does.
2. **One Rust version (Q2).**
   - `rust-toolchain.toml` → `channel = "1.99.1"`, equal to
     `deploy/Dockerfile`'s `RUST_IMAGE` today.
   - CI installs the toolchain from that file with the runner's rustup;
     the `dtolnay/rust-toolchain` step is removed.
   - `check_workflows.sh` also fails when the channel isn't `N.N.N`, when
     it differs from the Rust image's version, or when a workflow names
     another toolchain.
   - A bump changes the channel and the image tag with its digest in one
     PR. Lint fixes a new compiler needs go in that PR; no new `#[allow]`
     and no lint-level change.
3. **Update bot (Q3).**
   - **(a) Dependabot**, configured by `.github/dependabot.yml`. Not
     Renovate.
   - **(b) Standing approval.** This RR approves in advance four classes
     of update:
     1. lockfile-only cargo updates (`Cargo.lock` only;
        `versioning-strategy: lockfile-only`);
     2. GitHub Actions updates: a new SHA with its version comment,
        including a new major;
     3. image updates in `deploy/Dockerfile`, `deploy/compose.yaml` and
        `deploy/backup/Dockerfile`: a new digest, or a newer tag on the
        same line (Debian `trixie` snapshots, Alpine 3.24.x, Caddy 2.x);
     4. toolchain bumps: `rust-toolchain.toml` and the Rust image
        together, to a newer stable.

     Under all of these conditions: every required check is green
     (`cargo deny` included) with no `deny.toml` change; the diff holds
     only the pins (class 4 may add lint fixes); the owner approves each
     merge, never auto-merge (RR-0027, RR-0030); releases are at least 7
     days old (`cooldown`), except security fixes; this standing approval
     is reviewed with the quarterly `deny.toml` prune.

     Not covered, so the dependency-bump runbook applies per bump as
     today: any `Cargo.toml` change (new crate, major version, features),
     a new Debian release, a new Alpine minor, Caddy 3, any `deny.toml`
     change. Major crate versions are therefore not proposed by the bot;
     accepted.
   - **(c) Getting through the gates.** Bot PRs carry the `dependencies`
     label, not `agent-authored`; they need no Merge-Readiness Pack. For
     classes 3 and 4, which touch trigger paths, the agent (on the
     owner's word) or the owner adds the line "Standing approval:
     RR-0032, class N" to the PR body after checking the diff against the
     conditions. `agentic-gate.yml` also runs on the `pull_request`
     `edited` event, so that edit re-runs the trigger check.
   - If Dependabot can't update an image whose whole reference is an
     `ARG` default, the two references in `deploy/Dockerfile` move into
     their `FROM` lines (same images, no behaviour change). If the Caddy
     digest in `compose.yaml` isn't updated, that pin stays manual and the
     pack says so.
   - Schedule and grouping (the brief's `may_decide`): cargo weekly (one
     PR), actions weekly (one PR), images monthly (one PR), the toolchain
     pair when Rust ships; at most five open bot PRs.
4. **The unused scripts (Q4).**
   - `check_brief_tests.sh`: a step at the end of `ci.yml`'s
     `fmt / clippy / test` job.
   - `test_check_triggers.sh` and `check_workflows.sh`: a new job named
     `agentic scripts` in `agentic-gate.yml`, on every PR and on push to
     `main`.
   - `backup_restore_drill.sh`: a step in `deploy.yml`'s `image` job,
     after `smoke_compose.sh`. If it proves flaky on GitHub's runners, it
     moves to a weekly scheduled run and the pack records why.
5. **`deny.toml` (Q5).** Remove the eight unnecessary skips
   (`windows-targets` and the seven `windows_*` crates), keep the nine
   that still have two versions, and correct the comments for
   `windows-sys`, `syn` and `getrandom` from the current tree. No other
   change.
6. **Required checks (Q6).** `agentic scripts` becomes the eighth required
   check. The owner applies it after the job is on `main` (before that, it
   would never report and block every PR). The brief-test check and the
   drill bind through jobs already required. This satisfies RR-0027
   sub-answer 3's "the brief-test check joins them".

Covers:

- MB-0045's implementation PR, P1–P5;
- the workflow, `rust-toolchain.toml` (trigger path), `deny.toml`
  (trigger path), `deploy/Dockerfile` (trigger path, only the `ARG`→`FROM`
  move above), `.github/dependabot.yml` and `scripts/agentic/` changes
  above;
- bot PRs of classes 1–4 under the conditions in sub-answer 3, from now
  on;
- the one branch-protection change in sub-answer 6;
- the roll-backs below.

Does not cover:

- Renovate, or any app with write access to the repository;
- auto-merge;
- major-version action upgrades in this mission's own PR;
- any `Cargo.toml` change, `deny.toml` advisory or licence change, or a
  bot class not listed;
- any other branch-protection change.

## Rationale and trade-offs

- **SHA pins with a check.** Tags can move; a SHA can't. The check keeps
  the rule from eroding; the bot keeps pins current.
- **One Rust version.** CI and the shipped binary use the same compiler,
  and new lints arrive when chosen, about every six weeks, through a PR.
- **Dependabot.** No new third party with write access. Its cost, no
  custom PR text, is one hand-added line on image and toolchain PRs.
- **Standing approval.** An RR per digest bump would stop updates from
  being merged at all. Narrow classes with written conditions keep the
  dependency-bump runbook's intent: anything that changes what the code
  can do still gets its own consult.
- **Scripts inside existing jobs** avoid a second compile; one new small
  job holds the script tests.

## Approvals and authority

Approved by platform-owner (Q1–Q5 and the standing approval for bot
classes 1–4), and the owner (the Runbook paragraph and the
branch-protection change in Q6).

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0032-ci-pinning-and-maintenance.md`
- Mission Brief: `.agentic/missions/MB-0045-ci-supply-chain-and-toolchain.md`
- Related: RR-0027 and RR-0030 (merge approval, required checks, trigger
  paths); RR-0014 (image pins); RR-0016 (the one advisory ignore, not
  touched).
- Evidence: CRP-0032 → Evidence bundle (action SHAs, toolchain facts,
  `deny.toml` proof, branch protection of 2026-11-12).

## Constraints and follow-on rules (Hardened)

1. **Every action pinned by SHA**, with a version comment;
   `check_workflows.sh` enforces it. Adding a `write` permission to the
   allowlist needs an RR.
2. **One Rust version** in `rust-toolchain.toml` and the Rust image,
   changed together.
3. **Bot PRs merge only inside the standing approval's classes and
   conditions**, each with the owner's approval; anything else follows
   the dependency-bump runbook.
4. **The `edited` trigger stays** on `agentic-gate.yml` while bot PRs
   need a hand-added approval line.
5. **The standing approval is reviewed quarterly**, with the `deny.toml`
   prune; the review is recorded in a Continuity Pack or a new RR.

## Roll-back into the system (Hardened)

To be applied in MB-0045's implementation PR:

- `.agentic/runbooks/dependency-bump.md` → "When to use": add a
  paragraph: "Routine updates proposed by Dependabot in the four classes
  RR-0032 approves (lockfile-only cargo updates, action SHA updates,
  image digest or same-line tag updates under `deploy/`, and the
  toolchain-and-Rust-image pair) need no brief or consult: they merge
  under RR-0032's conditions, with the owner's approval of each PR, and
  image or toolchain PRs name 'Standing approval: RR-0032, class N' in
  their body. Any other dependency change follows this runbook."
- `MB-0045`: `resolutions: []` → `[RR-0032]`; Conceptual plan step 1
  gains "Answered by RR-0032."
- `CRP-0032`: `state: open` → `answered`.

## Supersession and history

Status: active once approved. Supersedes nothing. Adds a standing
approval alongside the dependency-bump runbook.
