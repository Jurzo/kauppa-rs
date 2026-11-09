# Mission Brief — Fix the SQLite advisory RUSTSEC-2026-0459

```yaml
---
id: MB-0041
owner: "@juuso-kalliomaki"
goal: "The app and the backup job run SQLite 3.54.0 or later, so RUSTSEC-2026-0459 (CVE-2026-119375) no longer applies, and `cargo deny check` passes again on `main` without a permanent ignore."
non_goals:
  - "Other dependency upgrades than the ones the fix needs (sqlx and its SQLite crates; the backup image's sqlite package)."
  - "Changing WAL, `synchronous`, the busy timeout or the pool size (MB-0025)."
  - "Moving off SQLite or off sqlx."
acceptance:
  - id: P1
    property: "The SQLite library the app links reports version 3.54.0 or later at runtime (`select sqlite_version()`), whichever way it is linked."
    check: cargo test -p kauppa-core repo::sqlite::tests::sqlite_version_patched
  - id: P2
    property: "`cargo deny check` passes with no `ignore` entry for RUSTSEC-2026-0459. If the RR approves an interim ignore while upstream has no fix, the entry names the advisory, the reason, and the review date, and is removed by this mission's final PR."
    check: cargo deny check
  - id: P3
    property: "The pool's settings are unchanged: WAL, `synchronous=NORMAL`, the busy timeout and `MAX_CONNECTIONS` (MB-0025's tests), and migrations 0001–0008 apply to a fresh database and to a copy of a 0008 database."
    check: cargo test -p kauppa-core repo::sqlite::tests
  - id: P4
    property: "The whole workspace builds and passes under clippy pedantic + nursery with `-D warnings`, with no change in behaviour (all existing tests pass unchanged, or the brief delta lists API-forced test edits)."
    check: cargo test --workspace --all-targets
  - id: P5
    property: "The deploy image still builds and runs (P1/P2 of MB-0022), and the backup image's `sqlite3` is 3.54.0 or later, or the RR records why its exposure is accepted."
    check: scripts/deploy/smoke_image.sh
context:
  - Cargo.toml
  - Cargo.lock
  - crates/kauppa-core/Cargo.toml
  - crates/kauppa-core/src/repo/sqlite/mod.rs
  - crates/kauppa-core/src/repo/sqlite/tests.rs
  - deny.toml
  - deploy/Dockerfile
  - deploy/backup/Dockerfile
  - .agentic/runbooks/dependency-bump.md
  - .agentic/mentorship/persistence.md
autonomy:
  may_decide:
    - "how P1's test reads the version (a query on the pool)"
    - "mechanical code changes an sqlx upgrade forces, inside `repo/`"
  escalate:
    - { when: "the path: wait for an sqlx release that accepts `libsqlite3-sys` 0.39, upgrade to sqlx 0.9 (a breaking release), link the system SQLite (`sqlite-unbundled`), or a git dependency on sqlx `main` (denied by `deny.toml` today)", to: platform-owner }
    - { when: "an interim `ignore` in `deny.toml` (lint config, a trigger path), its review date, and the exposure assessment behind it", to: security }
    - { when: "linking the system SQLite, which moves the version to the Debian runtime image and to every developer machine", to: platform-owner }
    - { when: "the upgrade changes migration behaviour, `_sqlx_migrations` checksums, or how the pool applies its options", to: data-owner }
    - { when: "the backup image's sqlite package can't reach 3.54.0 from its pinned Alpine branch", to: platform-owner }
  forbidden:
    - "edit committed migrations"
    - "lower lint strictness"
    - "ignore the advisory permanently or without an approved RR"
    - "allow git sources in `deny.toml` without an approved RR"
runbook: .agentic/runbooks/dependency-bump.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/persistence.md@v1
  - .agentic/mentorship/security.md@v1
resolutions: [RR-0016]
---
```

## Context

Found on 2026-11-09 by `cargo deny` on PR #44: RUSTSEC-2026-0459
(CVE-2026-119375). SQLite before 3.54.0 can return uninitialised
page-cache memory when it opens a database with a sibling `-wal` file
whose page size differs; a crafted pair of files can make ordinary queries
return stale bytes from the process's memory. The attack is local and
needs the victim to open files an attacker wrote. `libsqlite3-sys`
0.6.0–0.38.2 bundle affected SQLite; 0.39.0 (SQLite 3.54.0) and 0.39.1
(3.54.1) are fixed. 0.39 needs Rust 1.88; the workspace's
`rust-version` is 1.89.

The dependency path (as of 2026-11-09, checked on crates.io and GitHub):

- `kauppa-core`/`kauppa-web` → sqlx 0.8.6 (`sqlite` feature, i.e.
  bundled) → sqlx-sqlite 0.8.6 → `libsqlite3-sys ^0.30.1`, locked 0.30.1.
- sqlx 0.9.0 (2026-05-21) requires `libsqlite3-sys >=0.30.1, <0.38.0`:
  still no fixed version.
- sqlx `main` allows `>=0.30.1, <0.40.0`, so the next sqlx release can
  take 0.39, but none is released.
- sqlx 0.8 has a `sqlite-unbundled` feature that links the system
  library instead; the runtime image is Debian trixie, and its
  `libsqlite3` version and patch status decide the exposure then.
  `cargo deny` reports the advisory from the lockfile either way, so
  that path also needs an ignore entry.

Every PR fails `cargo deny` until this is resolved (`deny.toml` has
`ignore = []`).

Exposure, for the security owner to confirm: the app opens only its own
database on the droplet's data volume (`/data/db/`), written only by the
app (uid 10001) and by `kauppa-backup restore`, which writes a snapshot
taken by SQLite. Restoring a backup is the one path that brings in a
database file from elsewhere (Spaces). The backup image runs Alpine's
`sqlite3` (pinned `sqlite~3.53`, also before 3.54.0) against the same
files.

## Conceptual plan

1. **Clarify (S1).** CRP under the dependency-bump runbook: current state,
   the options above (including "do nothing" and an interim ignore),
   supply-chain checks for each, the exposure assessment, and the backup
   image. Platform-owner chooses the path; security approves any interim
   ignore.
2. If the chosen path waits for upstream: one PR with the approved ignore
   (review date) and P1's test marked to expect the current version, and
   this pack tracking the sqlx release; then the upgrade PR removes the
   ignore.
3. Upgrade per the RR; P1–P5; Merge-Readiness Pack with `cargo tree -i
   libsqlite3-sys` before and after.
4. After merge, MB-0028's redeploy (or a later one) ships it to the
   droplet.

Slow-mode triggers: the sqlx upgrade changes query or migration semantics;
the backup image can't get a fixed sqlite; the RR's review date passes
with no upstream release.

## Validation plan

- P1: a new test in `repo::sqlite::tests` (red today: 3.46-series bundled
  in 0.30.1).
- P2: `cargo deny check` locally and in CI.
- P3, P4: existing suites; a fresh and an upgraded database.
- P5: `smoke_image.sh` in CI's `deploy` workflow; `sqlite3 --version` in
  the backup image.
