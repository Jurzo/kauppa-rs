# Consultation Request Pack — The path to a fixed SQLite for RUSTSEC-2026-0459

```yaml
---
id: CRP-0021
mission: MB-0041
owner: "@juuso-kalliomaki"
target_role: platform
risk_tier: medium
urgency: medium
state: superseded
runbook_trigger: ".agentic/runbooks/dependency-bump.md (\"Bumping a crate across a major version\": sqlx 0.8 → 0.9; \"Changing enabled features\"; entry gate 2–3: CRP open, RR approved before the PR); CI trigger paths `Cargo.toml`, `deny.toml`, `deploy/backup/Dockerfile`; MB-0041 autonomy.escalate (the path → platform-owner; the backup image's sqlite → platform-owner; exposure → security)"
---
```

## Decision statement

Choose how MB-0041 gets a SQLite of 3.54.0 or later into the app and the
backup image: wait for an sqlx release that accepts `libsqlite3-sys` 0.39
under RR-0016's interim ignore, or move now by another route.

Sub-question (security): how the backup image's `sqlite3` reaches 3.54.0,
or whether its exposure is accepted until it does.

## Current state

- `Cargo.toml` (workspace): `sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio-rustls", "sqlite", "macros", "migrate", "chrono", "uuid"] }`.
  In 0.8, `sqlite` means bundled.
- Locked: sqlx 0.8.6 → sqlx-sqlite 0.8.6 → `libsqlite3-sys` 0.30.1
  (`bundled`). The app links SQLite **3.46.0** (`select sqlite_version()`,
  measured on `main` at 50322c0).
- sqlx-sqlite 0.8.6 requires `libsqlite3-sys ^0.30.1` and always enables
  its `unlock_notify` feature.
- Call sites: `crates/kauppa-core/src/repo/sqlite/` (pool options,
  `sqlx::migrate!`, and 112 runtime `query`/`query_as`/`query_scalar`
  calls in `kauppa-core`). `kauppa-web` has 8 more, in `error.rs`,
  `admin/bootstrap.rs` and two test modules.
- `deny.toml`: RUSTSEC-2026-0459 ignored per RR-0016, **review by
  2026-12-31**; `unknown-git = "deny"`.
- `rust-version = "1.89"` (workspace). Developers and CI use stable, and
  the image builds with rust 1.99.1.
- `deploy/backup/Dockerfile`: `alpine:3.24.4`, `'sqlite~3.53'` → 3.53.4-r0.

## Upstream state (2026-11-09)

| Release | Date | `libsqlite3-sys` requirement | Fixed SQLite reachable? |
| --- | --- | --- | --- |
| sqlx-sqlite 0.8.6 | 2025-05-19 | `^0.30.1` | no |
| sqlx-sqlite 0.9.0 (latest) | 2026-05-21 | `>=0.30.1, <0.38.0` | **no** |
| sqlx `main` (10462fa, PR #4502, merged 2026-11-08) | unreleased | `>=0.30.1, <0.40.0` | yes (0.39.1) |

- **No released sqlx accepts `libsqlite3-sys` ≥ 0.39.** The crates.io
  `max_version` for sqlx and sqlx-sqlite is 0.9.0.
- No release is scheduled. GitHub (repo now `transact-rs/sqlx`) has no
  milestone after 0.9.0, and no `release/0.8` branch, so a 0.8.x backport
  is unlikely.
- Two open issues ask for a release, with no maintainer reply yet:
  - #4516 (2026-11-09) asks for a 0.9.1 with #4502, citing this advisory.
  - #4495 (2026-11-01) asks for a release from `main` for other fixes.
- #4502's author says that sqlx's policy lets the `libsqlite3-sys` upper
  bound rise in a patch release, so a 0.9.1 can carry it.
- `libsqlite3-sys` 0.39.0 (2026-10-24) and 0.39.1 (2026-10-25) bundle
  SQLite 3.54.x, MIT, `rust_version` 1.88.

## Options and trade-offs

### Option A — wait for the sqlx release under RR-0016, then bump (recommended)

When sqlx 0.9.1 (or later) is on crates.io:

- `sqlx = "0.9.1"` with the features in "What sqlx 0.9 changes";
- `cargo update -p sqlx` and `-p libsqlite3-sys` as a separate commit;
- `rust-version` raised to 1.94;
- the `deny.toml` ignore removed (P2).

- Pros:
  - crates.io only;
  - one change, which actually fixes the advisory;
  - a trial of the same code on sqlx `main` already passes (Option D's
    trial: all tests, migrations, SQLite 3.54.1).
- Cons:
  - the date isn't ours. If no release lands by 2026-12-31, RR-0016 needs
    an amendment to extend, or a switch to D.
- Risk: low to medium. It's a breaking release, but the trial needed only
  test-code edits and one feature that was easy to miss
  (`sqlite-unlock-notify`, below).
- Cost: about an hour once released. Meanwhile, this pack watches #4516.

### Option B — sqlx 0.9.0 now

- **Doesn't fix the advisory.** sqlx-sqlite 0.9.0 still caps
  `libsqlite3-sys` at `<0.38.0`. The trial's
  `cargo update -p libsqlite3-sys --precise 0.39.1` fails with
  "required by package `sqlx-sqlite v0.9.0`".
- Pros:
  - takes the breaking migration early, so A later becomes a patch bump;
  - drops 42 lock entries and adds 23 (sqlx 0.9 no longer pulls `rsa`,
    `num-bigint-dig`, `pkcs1`/`pkcs8`, `whoami` 1, `windows-sys` 0.48 …).
- Cons:
  - a dependency-bump PR and an RR for no security gain;
  - the ignore stays.
- Risk: as A.
- Cost: as A, now plus later.

### Option C — link the system SQLite (`sqlite-unbundled`)

- **Doesn't fix the advisory on our runtime either.**
  - Debian trixie's `libsqlite3-0` is **3.46.1-7+deb13u2**. The Debian
    security tracker lists CVE-2026-119375 as **vulnerable** in trixie,
    with `<no-dsa> (Minor issue)`, so no security update is planned.
  - Only sid (3.54.1-2) is fixed; forky (3.53.4-2) and bookworm aren't.
- Further cons:
  - adds `libsqlite3-dev` to the builder and `libsqlite3-0` to the runtime
    image, and needs the system library on every developer machine (Ubuntu
    22.04 ships 3.37.2);
  - P1's version check would then depend on the host;
  - `cargo deny` still flags `libsqlite3-sys` 0.30.1 in the lockfile, so
    the ignore stays anyway (CRP-0016 Option C).
- Risk: medium (version drift between machines).
- Not recommended.

### Option D — git dependency on sqlx `main`

- `[patch.crates-io] sqlx = { git = "https://github.com/launchbadge/sqlx", rev = "10462fa44" }`,
  plus `[sources] allow-git` for that repo in `deny.toml`.
- Pros:
  - fixes the advisory now. The trial links SQLite **3.54.1**, and all
    tests pass twice in a row (core 78, password_compat 3, web 96).
- Cons:
  - an unreleased snapshot of a database driver, with whatever else is on
    `main` since 0.9.0 (for example the transaction-depth fixes #4394 and
    #4515);
  - `cargo deny` sources fails (5 × `source-not-allowed`) until
    `deny.toml` allows the git source, which MB-0041 forbids without an RR
    and the runbook forbids without one too;
  - it must be undone when the release lands (a second PR);
  - the image build fetches from GitHub.
- Risk: medium (unreviewed code; supply chain).
- Cost: as A, plus the `deny.toml` change and its later revert.
- Worth it only if the exposure were higher than CRP-0016 assessed.

### Option E — do nothing past RR-0016

- Pros: no work.
- Cons: RR-0016's review date passes, and MB-0041's goal isn't met.
- Not an option beyond 2026-12-31 without amending RR-0016.

## What sqlx 0.9 changes for this codebase (measured)

Trial: a scratch copy of `main` (50322c0), sqlx 0.9.0, then sqlx `main`
at 10462fa. Not committed.

1. **Feature names.**
   - `runtime-tokio-rustls` is gone. It becomes `runtime-tokio`
     + `tls-rustls-ring`, or `tls-none`, since SQLite needs no TLS.
   - `sqlite` now means `sqlite-bundled` + `sqlite-deserialize` +
     `sqlite-load-extension` + `sqlite-unlock-notify`. Proposed:
     `sqlite-bundled` + `sqlite-unlock-notify` explicitly, so extension
     loading isn't compiled in.
2. **`sqlite-unlock-notify` is required.**
   - 0.8.6 always enabled `libsqlite3-sys/unlock_notify`; 0.9 makes it
     opt-in.
   - Without it, the test suite's shared-cache in-memory databases
     (`test_support.rs`: `mode=memory&cache=shared`) get intermittent
     `SQLITE_LOCKED` errors: 1–4 `kauppa-web` tests fail with `500` on
     each of 4 full runs. The baseline passes 3/3.
   - With it, all green 4/4.
3. **Dynamic SQL needs `AssertSqlSafe`.** `query*` no longer takes a
   `&String` (`E0277`, "dynamic SQL strings should be audited"). There are
   3 errors, all in test-only code in `repo/name_key.rs` (lines 143, 153,
   216: table names and a constant in `format!`). Production code passes
   only literals. It's a mechanical fix inside `repo/` (brief
   `may_decide`).
4. **MSRV.** sqlx 0.9 declares `rust_version` 1.94. The workspace's
   `rust-version` (1.89) must rise to 1.94, a build setting
   (platform-owner). The toolchains in use (1.99.1) are unaffected.
5. **Migrations.**
   - The checksum is still SHA-384 over the file's SQL (`sqlx-core`
     `migrate/migration.rs`, 0.8.6 vs 0.9.0).
   - A database migrated by the 0.8.6 build opens and re-runs migrations
     cleanly under the 0.9 build. No `_sqlx_migrations` change, no
     data-owner escalation.
6. **Nothing else.** Pool options, WAL, `synchronous`, busy timeout,
   `Row`/`try_get` and `Error` matching compile unchanged. Clippy
   (pedantic + nursery, `-D warnings`) is clean. `cargo deny check` passes
   with sqlx 0.9.0 from crates.io; it reports only unnecessary-skip
   warnings because `windows-sys` 0.48 drops out.

## Backup image sub-question (security, platform)

- `alpine:3.24.4` (latest stable, 2026-11-05) has sqlite **3.53.4-r0**.
  Alpine's security tracker lists CVE-2026-119375 as unfixed in 3.19–3.24.
  Edge has **3.54.1-r0**, and Alpine 3.25 isn't released.
- Choices:
  - (i) **Accept until Alpine ships it.** Keep `'sqlite~3.53'`, and bump
    the pin when 3.24 gets 3.54.x or when 3.25 is out (next Alpine release
    expected around December). The exposure is as CRP-0016 assessed: the
    job opens only `/data/kauppa.db`, and its own `VACUUM INTO` snapshots.
  - (ii) Install `sqlite` from edge (`--repository …/edge/main`). That
    mixes release branches against 3.24's musl/openssl. Not recommended.
  - (iii) Build `sqlite3` from the 3.54.1 amalgamation in a builder stage,
    pinned by checksum. This adds a compiler stage to an image that is
    "no compiling" today.
- Recommended: (i), recorded in the RR per MB-0041 P5 ("or the RR records
  why its exposure is accepted"), with the same RR-0016 review date.

## Recommendation and question

Recommended option: **A**, with backup sub-answer **(i)**.
Because:

- B and C don't fix anything: sqlx 0.9.0 is still capped, and Debian
  trixie is unfixed with no DSA planned.
- D trades a low, local exposure for an unreleased driver and a git
  source.
- A's code change is already known and small (above), so it can ship the
  day sqlx releases.

Crisp question to answer: **Approve Option A: wait for an sqlx release
that allows `libsqlite3-sys` ≥ 0.39 under RR-0016, then move to sqlx 0.9.x
with the features and MSRV below in one PR?**

Numbered sub-questions:

1. **Path (platform):** A, or D now (git source allowed in `deny.toml`)?
   B and C are listed only to show they don't fix the advisory.
2. **Features (platform):** for 0.9, `runtime-tokio`, `tls-none` (SQLite
   needs no TLS) or `tls-rustls-ring` (same as today), `sqlite-bundled`,
   `sqlite-unlock-notify`, `macros`, `migrate`, `chrono`, `uuid`? Not the
   `sqlite` umbrella, which adds `load-extension` and `deserialize`.
3. **MSRV (platform):** raise the workspace `rust-version` from 1.89 to
   1.94 in the same PR?
4. **Deadline (security):** if sqlx hasn't released by **2026-12-15**,
   should this mission come back with an RR-0016 amendment (extend the
   date) or switch to D?
5. **Backup image (security/platform):** accept (i): keep `sqlite~3.53`
   until Alpine 3.24 or 3.25 ships 3.54.x, with the bump as part of this
   mission's final PR, or as its own PR if it lands first?
6. **P1 meanwhile:** add `sqlite_version_patched` only in the upgrade PR
   (it's red until then), rather than as an expected-failure test now?

## Blast radius and rollback

What can break (A): every query path in `repo/` (driver upgrade);
migrations (checksums shown unchanged); the shared-cache test databases
(if `sqlite-unlock-notify` is missed).
Who is affected: every user; the data file is untouched by the upgrade
itself (no migration added).
Rollback plan: revert the PR. The lockfile restores sqlx 0.8.6 and
`libsqlite3-sys` 0.30.1, and the ignore comes back with it. A database
written by SQLite 3.54 stays readable by 3.46 (same file format).

## Supply-chain check

From crates.io and GitHub on 2026-11-09:

| Crate | Target | Owners | Downloads (all / recent) | License | Open advisories at target |
| --- | --- | --- | --- | --- | --- |
| sqlx (+ -core, -macros, -sqlite) | 0.9.1 when released (trial: 0.9.0, `main` 10462fa) | abonander, mehcode | 185.1M / 55.9M (sqlx); 155.3M / 52.5M (sqlx-sqlite) | MIT OR Apache-2.0 | none (`cargo deny` on the trial) |
| libsqlite3-sys | 0.39.1 | thomcc, gwenn | 308.0M / 105.1M | MIT | none (fixes RUSTSEC-2026-0459) |

- Same owners and repositories as the versions locked today
  (`launchbadge/sqlx`, now `transact-rs/sqlx`; `rusqlite/rusqlite`).
- New transitive crates in the 0.9 trial: the RustCrypto 0.11/0.13
  generation (`digest`, `sha2` 0.11, `hmac`, `hkdf`, `md-5`, `sha1`,
  `block-buffer`, `crypto-common`, `hybrid-array`, `cmov`, `ctutils`),
  plus `whoami` 2, `etcetera` 0.11, `flume` 0.12, `hashlink` 0.11 and
  `hashbrown` 0.16. All are from crates.io, and `cargo deny` licenses and
  bans are ok.
- Option D adds a git source: the same repository, pinned by commit, but
  outside crates.io's immutability guarantees.
- Backup image: Alpine `main` packages, as today.

## Minimal context

- `.agentic/missions/MB-0041-sqlite-wal-advisory.md`, `.agentic/continuity/MB-0041.md`
- RR-0016 (interim ignore, constraints 2–4), CRP-0016 (exposure assessment)
- `Cargo.toml`, `deny.toml`, `crates/kauppa-core/src/repo/sqlite/mod.rs`,
  `crates/kauppa-core/src/repo/name_key.rs`,
  `crates/kauppa-web/src/routes/test_support.rs`, `deploy/backup/Dockerfile`

## Evidence bundle (Hardened)

- crates.io API (User-Agent `kauppa-rs-agent`):
  - sqlx and sqlx-sqlite `max_version` 0.9.0;
  - sqlx-sqlite 0.9.0's dependencies: `libsqlite3-sys >=0.30.1, <0.38.0`;
  - 0.8.6: `^0.30.1`, with features `["pkg-config", "vcpkg", "unlock_notify"]`.
- GitHub `launchbadge/sqlx`:
  - `sqlx-sqlite/Cargo.toml` on `main`: `>=0.30.1, <0.40.0` (commit
    10462fa, PR #4502);
  - the latest tag is v0.9.0, with no later milestone;
  - issues #4516 and #4495 are open.
- Trial (scratch copy of 50322c0, deleted afterwards):

  | Step | Result |
  | --- | --- |
  | sqlx 0.9.0, `cargo update -p libsqlite3-sys --precise 0.39.1` | fails (`<0.38.0`) |
  | sqlx 0.9.0, first `cargo check` | 3 × E0277 in `repo/name_key.rs` (tests) |
  | + `AssertSqlSafe` | check and clippy `-D warnings` clean |
  | without `sqlite-unlock-notify`, 4 full runs | web: 4, 1, 3, 1 failures (500s) |
  | with it, 4 full runs | 78 + 3 + 96 pass each time |
  | `cargo deny check` (0.9.0) | advisories ok, bans ok, licenses ok, sources ok |
  | sqlx `main` 10462fa + `libsqlite3-sys` 0.39.1 | `sqlite_version()` = 3.54.1; all tests pass 2/2 |
  | `cargo deny check` (git) | sources FAILED (5 × `source-not-allowed`) |
  | 0.8.6-migrated file DB reopened by 0.9 build | migrations ok, no checksum error |
  | baseline `sqlite_version()` | 3.46.0 |

- Debian: sources.debian.org shows trixie at `3.46.1-7+deb13u2`.
  security-tracker.debian.org/tracker/CVE-2026-119375 shows trixie
  vulnerable, `<no-dsa> (Minor issue)`, fixed in unstable 3.54.1-1
  (bug 1152790).
- Alpine: pkgs.alpinelinux.org shows v3.24 sqlite 3.53.4-r0 and edge
  3.54.1-r0. security.alpinelinux.org lists 3.24-main 3.53.4-r0 as not
  fixed. dl-cdn shows latest-stable 3.24.4, and v3.25 is absent.
