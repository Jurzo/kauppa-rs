# Resolution Record — Fix path for RUSTSEC-2026-0459: wait for sqlx 0.9.x, then upgrade in one PR

```yaml
---
id: RR-0021
closes: CRP-0021
mission: MB-0041
decided_by: "@juuso.kalliomaki"
role: platform
risk_tier: medium
decided_at: 2026-11-10T08:21:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0041-sqlite-wal-advisory.md
  - .agentic/consults/CRP-0021-sqlite-fix-path.md
---
```

## Decision and outcome

Approve CRP-0021 **Option A** with backup sub-answer **(i)**: keep RR-0016's
interim ignore, and when the first sqlx 0.9.x release whose `sqlx-sqlite`
allows `libsqlite3-sys` >= 0.39 is on crates.io, move to it in **one PR**.

Sub-answers:

1. **Path (Q1, platform).** Option A. Option D (git dependency on sqlx
   `main`) is not approved now; B and C don't fix the advisory.
2. **Features (Q2, platform).** The workspace `sqlx` entry, with
   `default-features = false`, enables exactly:
   `runtime-tokio`, `tls-rustls-ring`, `sqlite-bundled`,
   `sqlite-unlock-notify`, `macros`, `migrate`, `chrono`, `uuid`.
   Not the `sqlite` umbrella (it adds `sqlite-load-extension` and
   `sqlite-deserialize`), and not `tls-none`.
3. **MSRV (Q3, platform).** The workspace `rust-version` goes from 1.89 to
   **1.94** in the same PR.
4. **Deadline (Q4, security).** On **2026-12-15** MB-0041 reports back in
   its Continuity Pack and to the owner. If no qualifying sqlx release
   exists, the owner decides between an RR-0016 amendment (extend the
   review date) and Option D. Neither is chosen in advance; the agent does
   not pick one.
5. **Backup image (Q5, security/platform).** The exposure of the backup
   image's `sqlite3` (Alpine `sqlite~3.53`, 3.53.4-r0) is **accepted until
   Alpine ships 3.54.x** for the pinned branch (3.24 or 3.25) or until **2026-12-31**. The bump goes
   in this mission's final PR, or in its own PR if Alpine ships first. This
   is the record MB-0041 P5 asks for ("or the RR records why its exposure
   is accepted").
6. **P1 (Q6).** `repo::sqlite::tests::sqlite_version_patched` is added only
   in the upgrade PR, not earlier as an expected-failure test.

The upgrade PR also deletes RR-0016's `deny.toml` ignore entry for
RUSTSEC-2026-0459 (RR-0016 constraint 3; MB-0041 P2).

Covers:

- the upgrade PR: the `sqlx` version and feature change in the workspace
  `Cargo.toml`, `rust-version` 1.94, and the removal of the RR-0016 ignore;
- the lockfile changes the upgrade brings (`cargo update -p sqlx` and
  `-p libsqlite3-sys` as a separate commit, per `dependency-bump.md`),
  including the new transitive crates listed in CRP-0021 → Supply-chain
  check and the dropped ones (`rsa`, `num-bigint-dig`, `pkcs1`/`pkcs8`,
  `whoami` 1, `windows-sys` 0.48 …);
- the three known code changes: `AssertSqlSafe` at the test-only dynamic
  SQL sites in `crates/kauppa-core/src/repo/name_key.rs` (lines 143, 153,
  216 at 50322c0);
- the P1 test (sub-answer 6);
- the backup image's `sqlite~3.53` → 3.54.x pin bump (sub-answer 5), and
  the base image bump to Alpine 3.25 if that is how 3.54.x arrives.

Does not cover:

- any other dependency addition, removal or version bump;
- git sources or `[patch.crates-io]`; any `deny.toml` change other than
  deleting the RUSTSEC-2026-0459 entry (Option D needs its own decision
  under sub-answer 4);
- `sqlite-unbundled`, `sqlite-load-extension`, `sqlite-deserialize` or
  `tls-none`;
- source changes beyond the three `AssertSqlSafe` sites. If the release
  forces others, the agent stops and re-consults, except mechanical changes
  inside `repo/` that MB-0041 already lets it decide; anything touching
  pool options, migrations or `_sqlx_migrations` goes to data-owner (MB-0041
  escalation);
- extending RR-0016's review date (an amendment to RR-0016).

## Rationale and trade-offs

- **A — chosen.** crates.io only, and the one change actually fixes the
  advisory. The code change is known and small: the trial on sqlx `main`
  (10462fa) linked SQLite 3.54.1 and passed every test. The cost is that
  the date isn't ours, which sub-answer 4 handles.
- **B (sqlx 0.9.0 now) — rejected.** `sqlx-sqlite` 0.9.0 caps
  `libsqlite3-sys` at `<0.38.0`; a dependency-bump PR with no security gain.
- **C (`sqlite-unbundled`) — rejected.** Debian trixie's `libsqlite3-0` is
  itself vulnerable with no DSA planned, the version would depend on the
  host, and the lockfile still trips `cargo deny`.
- **D (git dependency) — not now.** Trades a low, local exposure (CRP-0016)
  for an unreleased driver snapshot and a git source; kept as one of the
  two choices on 2026-12-15.
- **E (nothing past RR-0016) — rejected.** Doesn't meet MB-0041's goal.
- **`tls-rustls-ring` over `tls-none` (Q2).** Same TLS stack as today's
  `runtime-tokio-rustls`, so no behaviour change; `tls-none` was the
  smaller alternative.
- **`sqlite-unlock-notify` explicit (Q2).** 0.8.6 always enabled it;
  without it the shared-cache test databases fail intermittently
  (CRP-0021 trial: 1–4 web test failures per run).
- **Backup image (i) — chosen.** The job opens only `/data/kauppa.db` and
  its own `VACUUM INTO` snapshots. Not chosen: (ii) installing from edge
  (mixes release branches), (iii) building from the amalgamation (adds a
  compiler stage).

## Approvals and authority

Approved by platform-owner @juuso.kalliomaki; sub-answers 4 and 5 by
security-owner @juuso.kalliomaki.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0021-sqlite-fix-path.md`
- Mission Brief: `.agentic/missions/MB-0041-sqlite-wal-advisory.md`
- Continuity Pack: `.agentic/continuity/MB-0041.md`
- Related: RR-0016 (interim ignore, constraints 2–4), CRP-0016 (exposure
  assessment), RR-0007 constraint 2 (no ignores without an RR).
- Evidence links: CRP-0021 → Upstream state, "What sqlx 0.9 changes",
  Supply-chain check and Evidence bundle (the 2026-11-09 trial; crates.io
  `max_version` 0.9.0; sqlx PR #4502 and issues #4516, #4495; Debian and
  Alpine trackers); https://rustsec.org/advisories/RUSTSEC-2026-0459

## Constraints and follow-on rules (Hardened)

1. **Released crates only.** The upgrade uses a crates.io sqlx 0.9.x whose
   `sqlx-sqlite` allows `libsqlite3-sys` >= 0.39, and the lockfile resolves
   `libsqlite3-sys` >= 0.39.0. No git source, no `[patch]`.
2. **One PR.** Features, MSRV, the `AssertSqlSafe` edits, the P1 test and
   the deletion of the ignore land together. The PR's Merge-Readiness Pack
   shows `cargo tree -i libsqlite3-sys` before and after, `select
   sqlite_version()` >= 3.54.0, and `cargo deny check` passing with no
   RUSTSEC-2026-0459 entry.
3. **Features are exact.** The eight features in sub-answer 2 and no
   others. Adding `sqlite-load-extension` or `sqlite-deserialize` later
   needs security review.
4. **2026-12-15 report.** MB-0041's Continuity Pack tracks the sqlx release
   (crates.io, issue #4516) and Alpine's sqlite. On 2026-12-15, with no
   qualifying release, the agent reports to the owner and waits; the
   decision (RR-0016 amendment or Option D) is recorded as an RR-0016
   amendment or a new RR before 2026-12-31.
5. **Backup image.** The `sqlite~3.53` pin is bumped to 3.54.x as soon as
   the pinned Alpine branch (or 3.25) ships it: in MB-0041's final PR, or
   in its own PR if that comes first. RR-0016 constraint 4 (no SQLite files from untrusted sources)
   holds for the backup job until then.
6. **Rollback.** Reverting the upgrade PR restores sqlx 0.8.6,
   `libsqlite3-sys` 0.30.1 and the ignore entry together; RR-0016 applies
   again, with its review date.

## Roll-back into the system (Hardened)

To be applied in the PR that adds this RR (MB-0041):

- `MB-0041`: `resolutions: [RR-0016]` → `[RR-0016, RR-0021]`.
- `CRP-0021`: `state: open` → `answered`.

No Mentorship Pack or Runbook change.

## Supersession and history

Status: active once signed. Supersedes nothing. Leaves RR-0016 in force
until the upgrade PR removes its entry; RR-0016's review date is unchanged.
