# Consultation Request Pack — RUSTSEC-2026-0459: the fix path, and the backup image

```yaml
---
id: CRP-0039
mission: MB-0041
owner: "@juuso-kalliomaki"
target_role: platform
risk_tier: low
urgency: low
state: open
runbook_trigger: "dependency-bump.md entry gate (any sqlx or libsqlite3-sys change); MB-0041 autonomy.escalate (the path → platform; the backup image's sqlite → platform; extending the interim ignore → security)"
---
```

## Decision statement

Decide how MB-0041 gets SQLite 3.54.0 or later into the app and the backup
image: wait for an sqlx release, carry a patched copy of sqlx's SQLite
driver, follow sqlx's unreleased `main`, or link the system SQLite. Also
decide when the backup image moves off Alpine 3.24's `sqlite` 3.53, and
what happens at RR-0016's review date (2026-12-31) if nothing upstream
has changed by then.

## Current state (checked 2026-11-14)

| What | State | Source |
| --- | --- | --- |
| Locked chain | sqlx 0.8.6 → sqlx-sqlite 0.8.6 → `libsqlite3-sys` 0.30.1 (bundled SQLite 3.46 series) | `Cargo.lock:1420` |
| Fixed | `libsqlite3-sys` ≥ 0.39.0 (SQLite 3.54.0); 0.39.1 (3.54.1) and, since 2026-11-11, 0.40.0 | crates.io; RUSTSEC "Patched: >=0.39.0" |
| Latest sqlx | 0.9.0 (2026-05-21). Nothing newer since the brief (2026-11-09) | crates.io |
| sqlx-sqlite 0.8.6 | `libsqlite3-sys ^0.30.1` | crates.io |
| sqlx-sqlite 0.9.0 | `>=0.30.1, <0.38.0`: no fixed version | brief, unchanged |
| sqlx `main` | `>=0.30.1, <0.40.0`: allows 0.39.x, not 0.40.0; unreleased | `sqlx-sqlite/Cargo.toml` on GitHub, today |
| Debian trixie `sqlite3` (runtime image) | 3.46.1-7+deb13u2, **vulnerable**, marked `<no-dsa>` (minor issue): no fix planned for trixie. Fixed in forky/sid 3.54.1 | Debian security tracker, CVE-2026-119375 |
| Alpine 3.24 `sqlite` (backup image, `sqlite~3.53`) | 3.53.4-r0, **vulnerable**. Edge has 3.54.1-r0 | pkgs.alpinelinux.org |
| Interim | `deny.toml` ignores the advisory until 2026-12-31 (RR-0016, PR #46) | `deny.toml:8-10` |

Call sites: every query goes through sqlx's `SqlitePool`
(`crates/kauppa-core/src/repo/sqlite/`); nothing calls `libsqlite3-sys`
directly. Features: workspace `sqlx` with `sqlite` (bundled). MB-0049's
macro work (in progress) doesn't change the version; a later sqlx upgrade
regenerates `.sqlx/` with `scripts/sqlx-prepare.sh`.

### Exposure (unchanged from RR-0016)

The attack needs the victim to open a database together with an
attacker-written sibling `-wal` file whose page size differs. On the
droplet only the app (uid 10001) and the backup job write the data
volume. `kauppa-backup restore` brings in one snapshot file from Spaces,
runs `PRAGMA integrity_check` on it, and first moves the old `.db`,
`-wal`, `-shm` and `-journal` aside together
(`deploy/backup/kauppa-backup:150-165`), so a restore can't pair an
outside file with a `-wal`. The backup image's `sqlite3` opens only the
live database (`VACUUM INTO`) and snapshots. Writing to Spaces needs the
backup key.

## Options and trade-offs

### Q1: the app's fix path (platform)

#### Option A — wait for an sqlx release that allows 0.39 (recommended)

Keep RR-0016's ignore. When sqlx releases a version whose `sqlx-sqlite`
allows `libsqlite3-sys` ≥ 0.39 (its `main` already does), upgrade under
this runbook: `cargo update -p libsqlite3-sys` as its own commit, P1–P5,
`.sqlx/` regenerated, the ignore removed.

- Pros: no fork, no git source, no unreleased code; the smallest diff.
- Cons: no date. The next release may be a 0.9.x, which is a breaking
  upgrade from 0.8 (its own changes to review).
- Risk: the exposure above, for longer.
- Cost: none now; a check at each review date.

#### Option B — a patched copy of sqlx-sqlite 0.8.6

Vendor `sqlx-sqlite` 0.8.6 with only its `libsqlite3-sys` bound raised,
wired with `[patch.crates-io]` to a path in the repository.

- Pros: fixes the app now, staying on sqlx 0.8.
- Cons: a non-crates.io source (an RR under the forbidden list); a fork to
  carry until upstream catches up; 0.30 → 0.39 crosses nine
  `libsqlite3-sys` releases whose bindings sqlx-sqlite 0.8.6 was never
  built against (sqlx `main` has changes to go with its wider range).
  Unknown until tried: whether it builds and behaves.
- Risk: a subtle FFI or behaviour change in the database layer, which the
  brief's P3 and P4 must catch.
- Cost: a trial, then a vendored crate to review and maintain.

#### Option C — sqlx from git `main`

- Cons: unreleased code; `deny.toml` denies git sources
  (`unknown-git = "deny"`), so an RR is needed; `main` is the 0.9 line or
  later (breaking), and MB-0049's macros and cache would change with it.
- Not recommended.

#### Option D — link the system SQLite (`sqlite-unbundled`)

- Cons: Debian trixie's `sqlite3` is vulnerable and Debian doesn't plan to
  fix it there; `cargo deny` still reports the advisory from the lockfile.
  It moves the problem, doesn't fix it.
- Rejected.

### Q2: the backup image (platform)

#### Option A — move to Alpine 3.25 when it is released (recommended)

Alpine edge already has `sqlite` 3.54.1; the next stable branch should
carry it. Change `FROM alpine:3.24.x` and the `sqlite~3.53` pin together,
under this runbook (a new Alpine branch isn't a same-line update, so not
RR-0032's standing approval).

- Pros: a supported, signed package; no mixed repositories.
- Cons: waits for the release.

#### Option B — `sqlite` from the edge repository on 3.24

- Cons: mixes a rolling branch into a pinned stable image; its libc and
  dependencies may drift. Not recommended.

#### Option C — accept the backup image's exposure until A

The backup image never opens a file next to an attacker-written `-wal`
(see Exposure). Recorded in the RR rather than left implicit.

### Q3: the review date (security)

RR-0016 ends the ignore on 2026-12-31 and says extending it needs an
amendment or a new RR. If no sqlx release allows 0.39 by then, the choice
is to extend the ignore (to a new date) or to take Option B. Recommended:
decide now that the review on 2026-12-31 re-checks crates.io, Debian and
Alpine and either extends the ignore by three months with this CRP's
exposure assessment or opens Option B's trial, so the date doesn't lapse
into a red `cargo deny` on every PR.

## Recommendation and question

Recommended: Q1 A (wait for an sqlx release; keep the ignore), Q2 A with
C (Alpine 3.25 when released; until then the backup image's exposure is
accepted as assessed), Q3 (a review on 2026-12-31 with a three-month
extension as the default if nothing has moved).

Crisp questions:

1. **Platform**: wait for an sqlx release that allows `libsqlite3-sys`
   ≥ 0.39 (Option A), rather than vendoring a patched sqlx-sqlite 0.8.6
   (B) or following sqlx `main` (C)? System linking (D) is rejected
   because trixie's sqlite stays vulnerable.
2. **Platform**: move the backup image to Alpine 3.25 and its 3.54
   `sqlite` when 3.25 is released, accepting the backup job's exposure
   until then?
3. **Security**: at RR-0016's review (2026-12-31), if nothing upstream has
   changed, extend the ignore by three months with this assessment (a
   new RR), or start Option B's trial instead?

## Blast radius and rollback

What can break: under A, nothing now. The eventual upgrade touches every
query (P3, P4 cover it) and regenerates `.sqlx/`. Under B, the vendored
driver is the risk.

Who is affected: nobody now; the security posture stays as RR-0016
assessed it, for longer.

Rollback plan: the upgrade PR reverts as one; the ignore stays until it
merges.

## Minimal context

- Brief: `.agentic/missions/MB-0041-sqlite-wal-advisory.md` (P1–P5,
  `escalate`, `forbidden`: no permanent ignore, no git source without an
  RR, no lower lint strictness).
- RR-0016 (the interim ignore, review by 2026-12-31).
- `deny.toml:8-10`; `Cargo.lock:1420`; `deploy/backup/Dockerfile:10-13`;
  `deploy/Dockerfile:16,39`; `deploy/backup/kauppa-backup:134-166`.

## Evidence bundle (Hardened)

Checked 2026-11-14:

- crates.io API: sqlx and sqlx-sqlite latest 0.9.0 (2026-05-21), previous
  0.8.6 (2025-05-19); `libsqlite3-sys` 0.40.0 (2026-11-11), 0.39.1
  (2026-10-25), 0.39.0 (2026-10-24), 0.38.2 (2026-08-08).
- Supply chain, `libsqlite3-sys`: owners `thomcc`, `gwenn` (the rusqlite
  maintainers); 317,510,358 downloads; MIT; `rust-version` 1.88 for
  0.39.x and 0.40.0 (the workspace is on 1.89 or later); repository
  `rusqlite/rusqlite`. `cargo deny check advisories` passes with only
  RR-0016's ignore, so nothing else in the tree is flagged.
- sqlx `main`: `sqlx-sqlite/Cargo.toml`, `libsqlite3-sys`
  `">=0.30.1, <0.40.0"`.
- RustSec RUSTSEC-2026-0459: patched `>=0.39.0`, unaffected `<0.6.0`.
- Debian security tracker, CVE-2026-119375: bookworm 3.40.1-2+deb12u2
  vulnerable; trixie 3.46.1-7+deb13u2 vulnerable (`<no-dsa>`, minor
  issue); forky and sid 3.54.1-2 fixed; fixed upstream by SQLite commit
  `e5bf1198` (3.54.0).
- pkgs.alpinelinux.org: `sqlite` 3.53.4-r0 on v3.24, 3.54.1-r0 on edge.
