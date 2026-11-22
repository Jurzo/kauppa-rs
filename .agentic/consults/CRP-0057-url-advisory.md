# Consultation Request Pack — RUSTSEC-2026-0592: update `url` to 2.5.9 in `Cargo.lock` (MB-0072)

```yaml
---
id: CRP-0057
mission: MB-0072
owner: "@juuso-kalliomaki"
target_role: platform
risk_tier: low
urgency: high
state: answered
runbook_trigger: "dependency-bump.md entry gate 2: a `Cargo.lock` change beyond what cargo writes for an in-scope task (an agent-made update; RR-0032's standing approval covers Dependabot's only)"
---
```

## Decision statement

Approve updating `url` from 2.5.8 to 2.5.9, with `form_urlencoded` 1.2.2
→ 1.2.3, in `Cargo.lock` only, so `cargo deny check` passes again without
an ignore.

## Current state

- `url` 2.5.8 (default features: `std`), a transitive dependency.
  - Through actix-web 4.18.2: actix-web itself, actix-session 0.10.1,
    actix-identity 0.8.0 and tracing-actix-web 0.7.26.
  - Through sqlx-core 0.8.6: sqlx-sqlite and sqlx-macros-core.
  - No workspace `Cargo.toml` names it.
- RUSTSEC-2026-0592, published 2026-11-22: `Url::origin()` (and
  `url::quirks::origin()`) recurse without limit on nested `blob:` URLs,
  and a stack overflow aborts the process. Fixed in 2.5.9.
- **Call sites:** none.
  - Neither our crates nor the six dependents above call `Url::origin()`
    or `url::quirks::origin()` (a search of their sources).
  - The cross-site check parses `Origin`/`Referer` with its own parser
    (`crates/kauppa-web/src/csrf/origin.rs`).
  - sqlx parses only the configured database URL.
- `deny.toml` fails on any advisory, so `cargo deny` fails on `main` and
  on every PR (first seen on #146, run 45009661837).

## Target state and justification

- **`Cargo.lock`:** `url` 2.5.9 and `form_urlencoded` 1.2.3, as
  `cargo update -p url` writes them. `url` 2.5.9 requires
  `form_urlencoded ^1.2.3`, so `--precise 2.5.9` changes the same two.
- **The diff:** four lines each way (two versions, two checksums).
- **Behaviour change in 2.5.9:** `Url::origin()` of a `blob:` URL whose
  inner scheme isn't `http`, `https` or `file` is now opaque. Nothing here
  calls it.
- **Tried locally on `main` `f3fef8d`:**
  - `cargo deny check`: advisories ok, bans ok, licenses ok, sources ok.
  - Clippy `-D warnings`: clean, apart from kauppa-core's two known
    unused-dependency warnings.
  - `cargo test --workspace --all-targets`: 108 + 3 + 242 passed.

## Supply-chain check

| | `url` 2.5.9 | `form_urlencoded` 1.2.3 |
|---|---|---|
| Source | crates.io, `servo/rust-url` | crates.io, `servo/rust-url` (same repository) |
| Owners | the servo/rust-url maintainers and the `servo` GitHub teams (`cargo-publish`, `rust-url`); unchanged from 2.5.8 | the same teams; unchanged |
| Published | 2026-11-20 | 2026-11-20 |
| Downloads | about 1.1 billion in all, 278 million recent | about 1.0 billion in all, 273 million recent |
| Licence | MIT OR Apache-2.0 (allowed by `deny.toml`) | MIT OR Apache-2.0 |
| Advisories | none open after the update (`cargo deny check`) | none |

## Options and trade-offs

### Option A — Update now, lockfile only (recommended; the owner's choice of path)

- Pros:
  - CI passes again today, for `main` and every PR.
  - The smallest change: two patch releases from the same maintainers, no
    manifest change.
- Cons: an agent-made PR through the runbook (brief, consult, RR), where
  Dependabot's would merge under RR-0032.

### Option B — Wait for Dependabot's update (RR-0032 class 1)

- Pros: no brief or consult; standing approval.
- Cons: until its weekly cargo run (or a security update), `cargo deny`
  fails on every PR, so #145 and #146 can't show a green CI.

### Option C — An interim ignore in `deny.toml`

- Pros: CI passes without touching the lockfile.
- Cons: a lint-config change (a trigger path) to hide an advisory that a
  released fix resolves; it must be removed again later.

### Option D — Do nothing

- Cons: `cargo deny` fails on every PR until someone updates.

## Recommendation and question

Recommended option: A.
Because: a released fix exists, it is a lockfile-only patch update from
the same maintainers, and nothing else in the lockfile changes.

Crisp question to answer: approve updating `url` to 2.5.9 and
`form_urlencoded` to 1.2.3 in `Cargo.lock` only (A)?

## Blast radius and rollback

- What can break: anything that parses URLs through `url`. That means
  actix-web's request URLs (`HttpRequest::url_for`, connection info) and
  sqlx's database URL. Both are covered by the workspace tests and the
  image smoke test.
- Who is affected: the whole app, at build time.
- Rollback plan: revert the PR (the lockfile returns to 2.5.8, and
  `cargo deny` fails again).

## Minimal context

- `.agentic/runbooks/dependency-bump.md`: entry gate (brief, CRP, RR
  before the PR); gates (`cargo deny check` passes; the `cargo update`
  is its own commit).
- RR-0032, sub-answer 3 (b): the standing approval covers Dependabot's
  lockfile-only updates.
- RR-0039 (MB-0041): the earlier advisory fix.

## Evidence bundle (Hardened)

- CI run 45009661837 (PR #146), the `cargo deny` job: RUSTSEC-2026-0592,
  "Solution: Upgrade to >=2.5.9 (try `cargo update -p url`)".
- `cargo update -p url --dry-run` on `f3fef8d`: "Updating form_urlencoded
  v1.2.2 -> v1.2.3", "Updating url v2.5.8 -> v2.5.9".
- crates.io: `url` 2.5.9's dependencies (`form_urlencoded ^1.2.3`,
  `idna ^1.1.0`, `percent-encoding ^2.3.2`); owners, downloads, releases
  and licences as in the table (2026-11-22).
- A search for `.origin()` and `quirks::origin` in our crates and in
  actix-web 4.18.2, actix-session 0.10.1, actix-identity 0.8.0,
  tracing-actix-web 0.7.26, sqlx-core 0.8.6 and sqlx-sqlite 0.8.6: no
  match.

## Owner's answer (2026-11-22)

A ("Brief accepted and CRP-0057 option A approved"). Recorded in RR-0057
(signed 2026-11-22).
