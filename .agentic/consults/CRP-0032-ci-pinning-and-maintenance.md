# Consultation Request Pack — CI pinning and maintenance: action SHAs, token permissions, one Rust version, an update bot, unused checks, the `deny.toml` prune

```yaml
---
id: CRP-0032
mission: MB-0045
owner: "@juuso-kalliomaki"
target_role: platform
risk_tier: medium
urgency: medium
state: open
runbook_trigger: "feature.md escalation triggers: `rust-toolchain.toml`, `deny.toml`, `deploy/Dockerfile`; dependency-bump.md entry gate (an RR per bump) and a proposed edit to that Runbook; CODEOWNERS: `.github/`, `scripts/agentic/`; RR-0027 does-not-cover (branch-protection changes); MB-0045 autonomy.escalate"
---
```

## Decision statement

Decide how CI's inputs are pinned (actions, token permissions, the Rust
version), which bot proposes updates and how its PRs pass the gates, where
the three unused scripts run, which `deny.toml` skips go, and which checks
become required.

## Terms

- **Pin by SHA:** `uses: owner/action@<40-hex commit>  # v4.4.0` instead of
  `@v4`. A tag can be moved to other code; a commit can't.
- **`permissions:`** the rights of the `GITHUB_TOKEN` a workflow gets.
- **Trigger path:** a path in `scripts/agentic/check_triggers.py`. A PR
  that touches one fails `Escalation trigger paths` unless its body names
  an approved RR.
- **Standing RR:** one RR that approves a class of routine changes in
  advance, under written conditions, instead of one RR per change.
- **Lockfile-only update:** a PR that changes `Cargo.lock` and not
  `Cargo.toml`: newer versions inside the ranges `Cargo.toml` already
  allows.

## Current state

Verified on the branch (from `main` def0d2a) on 2026-11-12. Details and
commands are in the Evidence bundle.

- **Actions.** Ten actions, all pinned by tag or branch. Seven are on an
  old major (`actions/checkout@v4`; the latest release is v7.0.1).
  `dtolnay/rust-toolchain@stable` is a branch.
- **Permissions.** `deploy.yml` declares `contents: read`, and
  `packages: write` on the `image` job only (lines 23–24, 76–79).
  `ci.yml` and `agentic-gate.yml` declare nothing. The repository default
  is already `read`, so a block changes nothing today; it protects
  against a changed default.
- **Toolchain.** `rust-toolchain.toml` says `channel = "stable"`.
  `deploy/Dockerfile:11` pins `rust:1.99.1-slim-trixie@sha256:f6c4a35d…`.
  Stable is 1.99.1 today, so the two agree by coincidence.
  `dtolnay/rust-toolchain` does not read `rust-toolchain.toml`: it
  installs the version named by its ref or its `toolchain` input. The step
  name in `ci.yml:24` ("from rust-toolchain.toml") is wrong.
- **Bot.** None. No `.github/dependabot.yml`, no `renovate.json`.
- **Gates a bot PR meets.**
  - `Escalation trigger paths` runs on every PR
    (`agentic-gate.yml:52`). It reads the PR body from the event.
  - `Merge-Readiness evidence` runs only with the `agent-authored` label
    (`agentic-gate.yml:29`). Without the label the job is skipped, and
    GitHub counts a skipped required check as passed.
  - `Cargo.lock` is **not** a trigger path in the script; `Cargo.toml`,
    `rust-toolchain.toml`, `deny.toml` and the `deploy/` files are.
    `.github/` and `scripts/agentic/` are CODEOWNERS-owned, not triggers.
  - The dependency-bump runbook says more than the script: a
    `Cargo.lock` change "beyond what `cargo` writes automatically for an
    in-scope task" is a trigger, and its entry gate wants a brief, a
    consult and an approved RR before the PR opens.
- **Unused scripts.** `check_brief_tests.sh` (needs cargo; 64 s locally
  on a warm build, 167 checks), `test_check_triggers.sh` (Python, PyYAML
  and git; throwaway repos; seconds), `backup_restore_drill.sh` (Docker,
  Compose, curl; no credentials; accepts `KAUPPA_SMOKE_IMAGE`, as the two
  smoke scripts in the `image` job do).
- **`deny.toml`.** 17 skip entries. `cargo deny check` warns that 8 are
  unnecessary. Three comments are out of date.
- **Branch protection.** Seven required checks, `strict: true`,
  `enforce_admins: false`, code-owner reviews on (RR-0030).

## Options and trade-offs

### Q1 — Action pins and token permissions

- **Recommended:**
  - Every `uses:` becomes `owner/action@<40-hex SHA>  # vX.Y.Z`, at the
    commit its current major tag points to today (table in the Evidence
    bundle). No behaviour change. Moving to newer majors is the bot's
    first proposal (Q3), reviewed one PR at a time.
  - `ci.yml` and `agentic-gate.yml` gain a top-level
    `permissions: contents: read`. Their jobs only check out code; the PR
    body comes from the event, not the API. `deploy.yml` is unchanged.
  - `scripts/agentic/check_workflows.sh` fails when:
    1. a `uses:` (other than a local `./` path) is not a 40-hex SHA
       followed by a `# v…` comment;
    2. a workflow has no top-level `permissions:`;
    3. a `write` permission appears that is not on the script's short
       allowlist (today one entry: `packages: write` on `deploy.yml`'s
       `image` job), or `write-all` appears anywhere.
  - The script tests itself against bad fixtures in a temporary
    directory, as `test_check_triggers.sh` does.
  - Limit: the script works offline. It can't prove that a SHA is the
    version its comment claims. The bot keeps the two in step; a reviewer
    sees both in the diff.
- Alternative: pin to the latest majors now (checkout v7 and so on).
  Mixes a pinning change with seven behaviour changes in one PR. Not
  recommended.
- Alternative: no allowlist for `write`, only "has a `permissions:`
  block". Simpler, but a later `contents: write` would pass unseen.

### Q2 — One Rust version

- **Recommended:**
  - `rust-toolchain.toml` → `channel = "1.99.1"`. It is today's stable and
    the image's version, so nothing changes on the day.
  - CI installs what the file says with the runner's own rustup
    (`rustup toolchain install`, which reads the file), and the
    `dtolnay/rust-toolchain` step is removed. One third-party action
    fewer, and the file is the only place the version lives.
  - `check_workflows.sh` fails when the channel is not `N.N.N`, when it
    differs from the version in `RUST_IMAGE`, or when a workflow names
    another toolchain.
  - A bump changes both files in one PR: the channel, and the image tag
    with its digest. The bot proposes it (Q3); half a bump fails the
    check by design. If the new compiler's lints need code fixes, the
    agent adds them to that PR: no new `#[allow]`, no lint-level change
    (dependency-bump runbook → Forbidden).
- Alternative: keep the action, SHA-pinned, with `toolchain: 1.99.1`.
  Works, but the version then lives in three places (file, workflow,
  Dockerfile), and the SHA must be one on the action's `master` branch
  (its README says other commits get garbage-collected).
- Alternative: bump by hand only. No surprise PRs, but Rust ships every
  six weeks and nobody is reminded.
- Cost of pinning: new lints arrive when you choose, not on release day.
  The price is a bump PR about every six weeks.

### Q3 — The update bot

#### Q3a — Which bot

| | Dependabot | Renovate |
| --- | --- | --- |
| Who runs it | GitHub. Enabled by committing `.github/dependabot.yml`. | Mend's hosted GitHub App, which the owner installs with write access to code, PRs and workflows. |
| Actions pinned by SHA | Yes; updates the SHA and the version comment. | Yes. |
| Cargo | Yes; `versioning-strategy: lockfile-only` exists. | Yes; lock-file maintenance and range strategies. |
| `rust-toolchain.toml` | Yes (`rust-toolchain`, since 2025-08). | Yes (`rust-toolchain` manager). |
| `deploy/backup/Dockerfile` (`FROM alpine:…@sha256`) | Yes. | Yes. |
| `deploy/Dockerfile` (the whole reference is an `ARG` default) | **Not verified.** The docs say nothing about `ARG`. | Documented for `ARG` values; a whole reference with a digest is **not verified**. |
| `deploy/compose.yaml` (Caddy, tag and digest) | `docker-compose` ecosystem exists; the digest is **not verified**. | Yes (`docker-compose` manager). |
| Grouping | `groups` per ecosystem; `multi-ecosystem-groups` for one PR across ecosystems. | Any grouping across managers. |
| Minimum release age | `cooldown`. | `minimumReleaseAge`. |
| Custom text in the PR body | No. | Yes (`prBodyNotes`). |

- **Recommended: Dependabot.** The mission exists to reduce trust in
  third parties. Renovate would add one with write access to the
  repository. Dependabot adds none and needs no setting change.
  - Its cost is the PR body: it can't carry an RR id by itself (Q3c).
  - If Dependabot can't read the `ARG` lines, MB-0045 moves the two
    references into the `FROM` lines of `deploy/Dockerfile`: same images,
    no behaviour change, and nothing else reads the `ARG`s
    (`grep -rn RUST_IMAGE` finds only the Dockerfile and the brief). If
    the Caddy digest isn't updated, that pin stays manual and the pack
    says so.
- Alternative: Renovate. Smoother (body notes, cross-file grouping), at
  the price of a third-party app with write access. Self-hosting it in
  Actions would need a stored token, which CI avoids today.

#### Q3b — How bot PRs pass the governance rules

- **(a) Recommended: a standing RR.** The RR that answers this consult
  approves these classes in advance:
  1. **Lockfile-only cargo updates.** `Cargo.lock` only. Dependabot runs
     with `versioning-strategy: lockfile-only`, so it never proposes a
     `Cargo.toml` change.
  2. **Action updates.** New SHA with its version comment, including new
     majors. `.github/` is not a trigger path today; the owner's review
     is the gate, as now.
  3. **Image updates** in `deploy/Dockerfile`, `deploy/compose.yaml`,
     `deploy/backup/Dockerfile`: a new digest, or a newer tag on the same
     line (Debian `trixie` snapshots, Alpine 3.24.x, Caddy 2.x).
  4. **Toolchain bumps:** `rust-toolchain.toml` and `RUST_IMAGE`
     together, to a newer stable (Q2).

  Conditions, all of them:
  - every required check is green, `cargo deny` included, with no
    `deny.toml` change in the PR;
  - the diff holds only the pins (class 4 may add lint fixes, as in Q2);
  - the owner approves each merge, as RR-0027 and RR-0030 already say. No
    auto-merge;
  - releases are at least 7 days old (`cooldown`), security fixes
    excepted;
  - the standing RR is reviewed with the quarterly `deny.toml` prune.

  Not covered, so the dependency-bump runbook applies as today (brief,
  consult, RR per bump): any `Cargo.toml` change (new crate, major
  version, features), a new Debian release, a new Alpine minor (the
  backup image pins apk packages to that branch), Caddy 3, any
  `deny.toml` change.

  The dependency-bump runbook gains one paragraph under "When to use"
  that says so (a Runbook edit; it rolls back from the RR).
- (b) An RR per bot PR. Today's rule, unchanged. Roughly four consults a
  week for digests and patch releases; the updates would stop being
  merged.
- (c) Exempt the bot's account in `check_triggers.py`. No RR at all for
  bot PRs. Weakens the gate script and leaves no written conditions. Not
  recommended.

With lockfile-only, nothing proposes major crate versions. They stay a
manual task under the runbook, as today. Accepted gap.

#### Q3c — How bot PRs pass `agentic-gate.yml`

- **Label. Recommended:** bot PRs carry `dependencies`, not
  `agent-authored`. `Merge-Readiness evidence` is then skipped, which
  GitHub counts as passed. The Merge-Readiness Pack maps a brief's
  acceptance IDs to proof; a bot PR has no brief. The standing RR and the
  green checks are its record.
- **RR id in the body.** Classes 1 and 2 fire no trigger and need
  nothing. Classes 3 and 4 touch trigger paths, so the body must name
  the standing RR. **Recommended:**
  - the agent (on your word) or you add one line to the PR body,
    "Standing approval: RR-00NN, class 3", after checking the diff
    against the conditions;
  - `agentic-gate.yml` listens for `pull_request` `edited` as well, so
    the edit re-runs the check. Today a body edit triggers nothing, and a
    manual re-run reuses the old body.
  - Images are grouped into one PR a month, so this is about one edit a
    month plus one per Rust release.
- Alternative: Renovate writes the line itself (Q3a).

Grouping and schedule (inside the brief's `may_decide`; stated so the
volume is clear): weekly for cargo (one PR) and actions (one PR), monthly
for images (one PR), the toolchain pair when Rust ships. At most five
open bot PRs. `strict: true` means each merge makes the other bot PRs
stale; few, grouped PRs keep that cheap.

### Q4 — The three unused scripts

- **Recommended:**
  - `check_brief_tests.sh`: a step at the end of `ci.yml`'s
    `fmt / clippy / test` job. It needs the compiled tests that job has
    just built; a job of its own would compile everything again. About a
    minute more.
  - `test_check_triggers.sh` and the new `check_workflows.sh`: a new job
    in `agentic-gate.yml`, named `agentic scripts`, on every PR and on
    push to `main`. Python, PyYAML and git only; seconds.
  - `backup_restore_drill.sh`: a step in `deploy.yml`'s `image` job,
    after `smoke_compose.sh`, with `KAUPPA_SMOKE_IMAGE: kauppa-rs:smoke`.
    Feasible: the job has Docker and the built image, and the drill
    needs no credentials (`scripts/deploy/compose.drill.yaml` points the
    backup at a local directory). It runs when the `image` job runs: PRs
    that touch the image's inputs, and every push to `main`.
- Risk: the drill is timing-sensitive (a writer must get 20 lists
  acknowledged within 10 s) and was never run on a GitHub runner. If it
  proves flaky it blocks image PRs. Then it moves to a weekly scheduled
  run, and the pack records why (P4 allows that).
- Alternative for the drill: weekly schedule only. Never blocks a PR, and
  also catches drift in the backup image's apk packages, but a break is
  found up to a week late, after the merge.

### Q5 — `deny.toml` prune

- **Recommended:** remove the 8 entries `cargo deny` reports, keep the
  other 9, and rewrite the comments from the current tree.
  - **Remove:** `windows-targets`, `windows_x86_64_msvc`,
    `windows_x86_64_gnu`, `windows_x86_64_gnullvm`, `windows_i686_msvc`,
    `windows_i686_gnu`, `windows_aarch64_msvc`,
    `windows_aarch64_gnullvm` (lines 43–50). One version each.
  - **Keep:** `windows-sys`, `syn`, `base64`, `derive_more`,
    `derive_more-impl`, `hashbrown`, `foldhash`, `getrandom`, `rand_core`.
    Two versions each.
  - **Comments to correct:** `windows-sys` is 0.52 (ring) and 0.61
    (tokio, mio and others), not 0.48 via walkdir. `syn` is 2.x and 3.x,
    not 1.x and 2.x. `getrandom` is 0.2 and 0.4, not 0.2 and 0.3.
  - Proof: a scratch copy without the 8 entries gives
    `advisories ok, bans ok, licenses ok, sources ok` and no warning. A
    scratch copy with no skips warns about exactly the 9 kept crates.
- No new entry, no advisory or licence change.

### Q6 — Required checks

- **Recommended:** add one required check, `agentic scripts` (Q4). The
  set becomes eight. The brief-test check and the drill are steps inside
  jobs that are already required (`fmt / clippy / test`,
  `image build, smoke tests, push (main)`), so they bind without a
  settings change. That also settles RR-0027 sub-answer 3 ("the
  brief-test check joins them").
- You apply the setting. I give the exact `gh api` command and record
  before and after in the pack. Order matters: the job must exist on
  `main` first, or the new required check never reports and blocks every
  PR.
- Alternative: separate jobs for the brief-test check and the drill, each
  required. Clearer names in the checks list; a second full compile, and
  ten required checks.

## Recommendation and question

Recommended: Q1–Q6 as above. In one line each:

Crisp questions:

1. **Pins and permissions.** Pin every action by SHA at its current major
   with a version comment, add `permissions: contents: read` to `ci.yml`
   and `agentic-gate.yml`, and let `check_workflows.sh` fail on a tag
   pin, a missing `permissions:` block, or a `write` permission outside
   its allowlist (today only `packages: write` on the `image` job)?
2. **Toolchain.** Pin `rust-toolchain.toml` to `1.99.1`, install it in CI
   with rustup from that file (dropping `dtolnay/rust-toolchain`), check
   that it equals the `RUST_IMAGE` version, and bump both in one PR that
   the bot proposes?
3. **Bot.** (a) Dependabot, not Renovate? (b) A standing RR for the four
   routine classes under the listed conditions, with a paragraph in the
   dependency-bump runbook, and everything else per bump as today?
   (c) Bot PRs labelled `dependencies` (no Merge-Readiness Pack), the
   standing RR id added to the body by hand for image and toolchain PRs,
   and `agentic-gate.yml` re-running on `edited`? And if Dependabot can't
   read the `ARG` lines, may the two image references move into the
   `FROM` lines of `deploy/Dockerfile`?
4. **Scripts.** `check_brief_tests.sh` as a step in `fmt / clippy / test`;
   `test_check_triggers.sh` and `check_workflows.sh` in a new
   `agentic scripts` job; the restore drill as a step in the `image` job,
   moving to a weekly schedule only if it proves flaky?
5. **`deny.toml`.** Remove the 8 `windows*` skips, keep the other 9, and
   correct the three comments?
6. **Required checks.** Add `agentic scripts` as the eighth required
   check, applied by you once the job is on `main`?

"As recommended" answers all six.

Only you can sign the RR and change the branch-protection setting.
Nothing is implemented until then.

## Blast radius and rollback

What can break:
- A wrong SHA or a too-narrow `permissions:` block fails a workflow. It
  shows on the implementation PR itself, before merge.
- `rustup toolchain install` behaving differently on the runner than
  locally. Same: visible on the PR.
- A pinned toolchain goes stale if bumps are ignored. The bot reminds.
- Bot PRs add review work, and each merge re-queues the others
  (`strict`). Grouping and the limit of five bound it.
- A lockfile-only update can change behaviour inside a semver range. The
  tests, `cargo deny` and the 7-day cooldown are the guard; the owner
  still approves each merge.
- A standing RR is a wider approval than one per change. Its conditions
  and the per-merge approval bound it; it can be superseded at any time.
- A flaky drill blocks image PRs until moved to a schedule.
- A required check that never reports blocks every PR (Q6 order).

Who is affected: the owner and the agent. No user-facing change. The
image is built by the same compiler version as today.

Rollback plan: revert the implementation PR; delete
`.github/dependabot.yml` to stop the bot and close its PRs; supersede the
standing RR; restore the previous required checks (recorded in the pack
before the change).

## Minimal context

- MB-0045 P1–P5; `autonomy.escalate` (toolchain, bot, `deny.toml`,
  Runbook change); `forbidden` (no lower lint level, no dropped check, no
  unneeded write permission, no advisory ignore).
- RR-0027 sub-answer 3 and "Does not cover"; RR-0030 constraints 1–2.
- `.agentic/runbooks/dependency-bump.md` (entry gate, "When to use",
  Forbidden).
- Files the implementation would touch: `.github/workflows/*.yml`,
  `.github/dependabot.yml` (new), `scripts/agentic/check_workflows.sh`
  (new), `rust-toolchain.toml`, `deny.toml`, possibly
  `deploy/Dockerfile`, and `dependency-bump.md` (roll-back).
- Out of scope here: Terraform provider and CLI versions
  (`deploy.yml:149`), the apk package pins in `deploy/backup/Dockerfile`,
  Dependabot security alerts (a repository setting), newer action majors
  (the bot's first PRs).

## Evidence bundle (Hardened)

All read on 2026-11-12. `gh api` calls were GETs.

**Actions: tag → commit** (`gh api repos/<repo>/git/ref/tags/<tag>`,
annotated tags dereferenced with `git/tags/<sha>`; exact version from
`repos/<repo>/tags`; latest from `releases/latest`):

| Action (used at) | Commit | Version | Latest |
| --- | --- | --- | --- |
| `actions/checkout@v4` | `11d5960a326750d5838078e36cf38b85af677262` | v4.4.0 | v7.0.1 |
| `actions/setup-python@v5` | `a26af69be951a213d495a4c3e4e4022e16d87065` | v5.6.0 | v7.1.0 |
| `Swatinem/rust-cache@v2` | `d1527c8f67ef9af357290475d4af4b0fab8f12e0` | v2.9.3 | v2.9.3 |
| `EmbarkStudios/cargo-deny-action@v2` | `3c6349835b2b7b196a839186cb8b78e02f7b5f25` | v2.1.1 | v2.1.1 |
| `docker/setup-buildx-action@v3` | `8d2750c68a42422c14e847fe6c8ac0403b4cbd6f` | v3.12.0 | v4.4.1 |
| `docker/build-push-action@v6` | `10e90e3645eae34f1e60eeb005ba3a3d33f178e8` | v6.19.2 | v7.4.0 |
| `docker/metadata-action@v5` | `c299e40c65443455700f0fdfc63efafe5b349051` | v5.10.0 | v6.2.0 |
| `docker/login-action@v3` | `c94ce9fb468520275223c153574b00df6fe4bcc9` | v3.7.0 | v4.8.0 |
| `hashicorp/setup-terraform@v3` | `b9cd54a3c349d3f38e8881555d616ced269862dd` | v3.1.2 | v4.0.1 |
| `dtolnay/rust-toolchain@stable` (a branch) | `stable` → `b725eda525362e7cf34af9a7243cac1e3f01a7ec`; `master` → `683b588c487a69d01f6c6ddf40f036ff682cc2c8` | none | none |

- `uses:` lines: `ci.yml:22,25,29,48,49`;
  `agentic-gate.yml:13,16,31,36,54,59`;
  `deploy.yml:41,81,83,86,110,120,128,145,147,159`.
- `dtolnay/rust-toolchain`: `action.yml` at `b725eda` takes the version
  from its `toolchain` input and runs `rustup toolchain install` and
  `rustup default`; it never reads `rust-toolchain.toml`. The `stable`
  branch is one commit ahead of `master` (`compare` → `ahead 1`), and the
  README ("Choice of full-length commit SHA") requires a SHA from
  `master`'s history.

**Permissions.**
- `gh api repos/Jurzo/kauppa-rs/actions/permissions/workflow` →
  `{"default_workflow_permissions":"read","can_approve_pull_request_reviews":false}`.
- `grep -n permissions: .github/workflows/*.yml` → only `deploy.yml:23`
  and `deploy.yml:76`.

**Toolchain.**
- `rustup check` → `stable-x86_64-unknown-linux-gnu - up to date: 1.99.1
  (58cf8f9c3 2026-10-13)`. `rustc --version` → `rustc 1.99.1`.
- `rust-toolchain.toml`: `channel = "stable"`, components `rustfmt`,
  `clippy`, profile `minimal`.
- `Cargo.toml:18-19`: clippy `pedantic` and `nursery` at `warn`; CI runs
  clippy with `-D warnings`.
- `deploy/Dockerfile:19-20`: `rust-toolchain.toml` is not in the build
  context, so the image's own compiler builds the binary.

**Gates.**
- `agentic-gate.yml:29`: `evidence-check` needs the `agent-authored`
  label. `agentic-gate.yml:52`: `trigger-path-check` runs on every pull
  request. No `types:` key in any workflow, so `edited` triggers nothing.
- `check_triggers.tripped_labels` (the function only, no fetch):
  `Cargo.lock` → none; `.github/workflows/ci.yml` → none;
  `.github/dependabot.yml` → none; `scripts/agentic/check_workflows.sh`
  → none; `Cargo.toml` → `platform:deps`; `rust-toolchain.toml` →
  `platform:toolchain`; `deny.toml` → `platform:lint-config`;
  `deploy/backup/Dockerfile` → `platform:deploy`.
- The last five PRs (#68–#72) all carry `agent-authored`. The repository
  has no `dependencies` label yet.

**Branch protection**
(`gh api repos/Jurzo/kauppa-rs/branches/main/protection`): contexts
`cargo deny`, `fmt / clippy / test`, `validate .agentic/ artifacts`,
`Merge-Readiness evidence`, `Escalation trigger paths`,
`image build, smoke tests, push (main)`, `no secrets committed`;
`strict: true`; `enforce_admins: false`;
`require_code_owner_reviews: true`; `required_approving_review_count: 0`.

**Pinned images.** `deploy/Dockerfile:11` (Rust 1.99.1-slim-trixie),
`:12` (Debian trixie-20261104-slim), `deploy/compose.yaml:56` (Caddy
2.11.9-alpine), `deploy/backup/Dockerfile:10` (Alpine 3.24.4). All tag
plus `sha256` digest.

**Scripts.**
- `time ./scripts/agentic/check_brief_tests.sh` → `checked 167 brief
  checks, 0 selecting no tests`, 64 s wall on a warm local build.
- `backup_restore_drill.sh:35` needs `docker curl`; `:41` uses
  `KAUPPA_SMOKE_IMAGE`; ports 28080 and 28443; `:175-179` wait up to 10 s
  for 20 acknowledged writes. Not run for this consult.

**`deny.toml`** (cargo-deny 0.20.2).
- `cargo deny check` → 8 × `warning[unnecessary-skip]: skip '<crate>'
  applied to a crate with only one version` (the eight crates in Q5,
  `deny.toml:43-50`), then `advisories ok, bans ok, licenses ok, sources
  ok`.
- `cargo deny --config <scratch copy without those 8> check` →
  `advisories ok, bans ok, licenses ok, sources ok`, no warning.
- `cargo deny --config <scratch copy with skip = []> check bans` →
  `warning[duplicate]` for exactly `base64`, `derive_more`,
  `derive_more-impl`, `foldhash`, `getrandom`, `hashbrown`, `rand_core`,
  `syn`, `windows-sys`.
- `cargo tree -i` on each kept pair: `windows-sys` 0.52.0 (ring) and
  0.61.2 (tokio, mio, socket2, rustix, tempfile …); `syn` 2.0.119
  (sqlx-macros, derive_more-impl, tracing-attributes …) and 3.0.9
  (serde_derive, actix-web-codegen, thiserror-impl, tokio-macros …);
  `base64` 0.20.0 (cookie 0.16.2) and 0.22.1; `derive_more` 1.0.0
  (actix-identity, actix-session) and 2.1.1 (actix-web, actix-http);
  `hashbrown` 0.15.5 (sqlx-core, hashlink) and 0.17.1 (indexmap);
  `foldhash` 0.1.5 and 0.2.0; `getrandom` 0.2.17 (ring, rand_core 0.6)
  and 0.4.3 (uuid, tempfile, rand 0.10); `rand_core` 0.6.4
  (password-hash, rand 0.8) and 0.10.1 (rand 0.10, chacha20).
- `grep -n walkdir Cargo.lock` → nothing.

**Bots** (vendor documentation, read 2026-11-12; not tried on this
repository).
- GitHub Docs, "Dependabot supported ecosystems and repositories":
  `cargo`, `docker`, `docker-compose`, `github-actions`,
  `rust-toolchain` (`channel = "1.xx.yy"` in `rust-toolchain.toml`),
  `terraform`. For actions: a commit SHA is supported and "a tag or link
  in a comment on the same line is updated".
- GitHub Docs, "Dependabot options reference": `versioning-strategy`
  supports cargo and has `lockfile-only` ("Only create pull requests to
  update lockfiles"); `groups`; `multi-ecosystem-groups`; `cooldown`;
  `labels`; `open-pull-requests-limit` (default 5). No option adds text
  to the PR body.
- Renovate Docs: `rust-toolchain` manager; `dockerfile` manager "will
  automatically expand variables and `ARG` directives" (the example is a
  tag in an `ARG`, not a whole reference).

**Not verified.**
- Whether Dependabot or Renovate updates an image whose whole reference
  (tag and digest) is an `ARG` default, and whether Dependabot updates a
  digest in a Compose file. The bot's first run answers both.
- Whether Dependabot keeps a hand-edited PR body when it later updates
  the same PR.
- Whether one Dependabot PR can carry the toolchain file and the Rust
  image together (`multi-ecosystem-groups`), and whether the two releases
  appear close enough in time.
- The rustup version on `ubuntu-latest` runners (`rustup toolchain
  install` without arguments needs rustup 1.28 or newer; the fallback is
  to pass the version read from the file).
- The drill's duration and stability on a GitHub runner.
