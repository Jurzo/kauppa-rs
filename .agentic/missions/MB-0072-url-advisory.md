# Mission Brief — Fix the `url` advisory RUSTSEC-2026-0592

```yaml
---
id: MB-0072
owner: "@juuso-kalliomaki"
goal: "`Cargo.lock` takes `url` 2.5.9, the release that fixes RUSTSEC-2026-0592 (a stack overflow in `Url::origin()` on nested `blob:` URLs), so `cargo deny check` passes again on `main` and on every open pull request, without an ignore."
non_goals:
  - "Any other dependency update than the ones `url` 2.5.9 requires (`form_urlencoded` 1.2.3)."
  - "A `Cargo.toml` change: `url` is a transitive dependency (actix-web, sqlx), and no manifest names it."
  - "A `deny.toml` change, including an ignore for this advisory."
  - "kauppa-core's two unused dependencies (a separate follow-up)."
acceptance:
  - id: P1
    property: "`cargo deny check` passes, with no `ignore` entry for RUSTSEC-2026-0592: `Cargo.lock` has `url` 2.5.9 or later."
    check: cargo deny check
  - id: P2
    property: "The pull request changes `Cargo.lock` only: `url` 2.5.8 → 2.5.9 and `form_urlencoded` 1.2.2 → 1.2.3 (`url` 2.5.9 requires `^1.2.3`), as `cargo update -p url` writes them. No manifest, `deny.toml` or source change."
    check: "git diff --stat main (Cargo.lock only), recorded in the PR"
  - id: P3
    property: "The whole workspace builds and passes clippy with `-D warnings`, and every existing test passes unchanged."
    check: cargo test --workspace --all-targets
  - id: P4
    property: "The deploy image still builds and passes its smoke tests."
    check: scripts/deploy/smoke_image.sh
context:
  - Cargo.lock
  - deny.toml
  - crates/kauppa-web/src/csrf/origin.rs
  - .agentic/runbooks/dependency-bump.md
  - .agentic/resolutions/RR-0032-ci-pinning-and-maintenance.md
  - .agentic/resolutions/RR-0039-sqlite-advisory-fix-path.md
autonomy:
  may_decide:
    - "nothing beyond the lockfile update the RR approves"
  escalate:
    - { when: "the fix path: a lockfile-only update now, waiting for Dependabot's (RR-0032 class 1), or an interim ignore in `deny.toml`", to: platform-owner }
    - { when: "the update needs more than `url` and what it requires, or a `Cargo.toml` change", to: platform-owner }
    - { when: "the exposure assessment, if code is found that calls `Url::origin()` on untrusted input", to: security }
  forbidden:
    - "ignore the advisory in `deny.toml`"
    - "change a `Cargo.toml` or `rust-toolchain.toml`"
    - "lower lint strictness"
    - "open the pull request before the RR is approved (dependency-bump runbook, entry gate 3)"
runbook: .agentic/runbooks/dependency-bump.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/security.md@v1
resolutions: []
---
```

## Context

Found on 2026-11-22: `cargo deny check` failed on PR #146 (CI run
45009661837) with advisory RUSTSEC-2026-0592, published that day. It fails
on `main` and on every pull request from now on. The owner chose a
lockfile-only update through this runbook over waiting for Dependabot
("Go with option 2").

- **The advisory:** in `url` 2.5.8 and earlier, `Url::origin()` on a
  `blob:` URL parses its path as another URL and calls itself again, with
  no depth limit. About 10,000 nested `blob:` prefixes overflow a thread's
  stack and abort the process. `url::quirks::origin()` calls it too.
  2.5.9 takes the inner origin only for `http`, `https` and `file`, and
  never recurses.
- **Where `url` comes from:** `url` 2.5.8, through actix-web 4.18.2
  (and actix-session, actix-identity, tracing-actix-web) and sqlx-core
  0.8.6 (and sqlx-sqlite, sqlx-macros-core). No `Cargo.toml` in the
  workspace names it.
- **Exposure:** nothing calls `Url::origin()` or `url::quirks::origin()`.
  A search of our crates and of those six dependents' sources found no
  call. The cross-site check parses `Origin` and `Referer` with its own
  parser (`crates/kauppa-web/src/csrf/origin.rs`). sqlx parses only the
  configured database URL. So the advisory can't be triggered here, but
  `deny.toml` denies every advisory, so CI fails.
- **The update** (tried locally on `main` `f3fef8d`, not committed):
  - `cargo update -p url` changes `Cargo.lock` only: `url` 2.5.8 → 2.5.9
    and `form_urlencoded` 1.2.2 → 1.2.3. `url` 2.5.9 requires
    `form_urlencoded ^1.2.3`, so `--precise 2.5.9` gives the same two.
  - With it, `cargo deny check` gives advisories ok, bans ok, licenses ok,
    sources ok.
  - Clippy with `-D warnings` is clean apart from kauppa-core's two known
    unused-dependency warnings.
  - `cargo test --workspace --all-targets` passes: 108 + 3 + 242.
- **Precedent:** MB-0041 / RR-0039 (the SQLite advisory), which needed a
  crate upgrade. RR-0032 approves lockfile-only updates in advance only
  when Dependabot proposes them; this one is agent-made, so it follows the
  runbook.

## Conceptual plan

1. **Clarify (S1).** CRP-0057 for the fix path (platform); RR before the
   PR (entry gate 3).
2. **The update.** `cargo update -p url`, one commit, `Cargo.lock` only
   (gates: a separate commit; no version string changes elsewhere).
3. **Checks.** `cargo deny check`, clippy, the workspace tests, the image
   smoke test (CI's image job).
4. **Then:** re-run `cargo deny` on the open PRs (#145, #146).

Slow-mode triggers:

- The update pulls in more than `url` and `form_urlencoded`.
- A test changes behaviour.

## Validation plan

- **P1:** `cargo deny check`, locally and in CI.
- **P2:** the PR's file list (Cargo.lock only) and its diff (four lines
  each way).
- **P3:** CI's fmt / clippy / test job and the browser job.
- **P4:** CI's image build and smoke tests.
