# Consultation Request Pack — Interim ignore for RUSTSEC-2026-0459 (SQLite < 3.54.0)

```yaml
---
id: CRP-0016
mission: MB-0041
owner: "@juuso-kalliomaki"
target_role: security
risk_tier: low
urgency: high
state: answered
runbook_trigger: "CI trigger path `deny.toml` (platform:lint-config); RR-0007 constraint 2 (`[advisories] ignore` stays empty unless a later RR approves an entry); MB-0041 autonomy.escalate (interim ignore → security) and forbidden (no ignore without an approved RR)"
---
```

## Decision statement

Approve a time-boxed `deny.toml` ignore for RUSTSEC-2026-0459 while no
released sqlx accepts the fixed `libsqlite3-sys`, so `cargo deny` stops
failing every PR. Choosing MB-0041's final path (sqlx upgrade, system
SQLite, …) is a separate question for this mission's later CRP.

## Current state

- `deny.toml`: `[advisories] ignore = []` (RR-0007 constraint 2).
- `Cargo.lock`: sqlx 0.8.6 → sqlx-sqlite 0.8.6 → `libsqlite3-sys` 0.30.1,
  built with the `bundled` feature (SQLite before 3.54.0).
- Fixed in `libsqlite3-sys` 0.39.0 (SQLite 3.54.0). sqlx-sqlite 0.8.6
  requires `^0.30.1` and 0.9.0 requires `<0.38.0`; sqlx `main` allows
  `<0.40.0` but is unreleased (crates.io and GitHub, 2026-11-09).
- Since 2026-11-09 `cargo deny` fails on `main` and on every PR (#44, #45)
  with this one advisory; bans, licenses and sources are ok.

## Options and trade-offs

### Option A — interim ignore with a review date (recommended)

- `ignore = [{ id = "RUSTSEC-2026-0459", reason = "… RR-0016, review by 2026-12-31." }]`.
- Pros: CI is green again for every other mission (MB-0028's go-live work,
  the feature briefs) at once; no code or dependency change; the entry
  names the RR and the date.
- Cons: cargo-deny has no expiry field, so the date is enforced only by
  this RR and MB-0041's Continuity Pack, not by CI; a *new* advisory on
  the same crate would still fail (the ignore is by ID), which is what we
  want.
- Risk: low (see exposure below).
- Cost: one line; removed by MB-0041's fix PR.

### Option B — no ignore; merge past the red check until the fix

- Pros: no exception in config.
- Cons: every PR needs an owner merge past a failing required check, which
  hides any *other* new advisory or license problem behind a known red.
- Risk: medium (masking).

### Option C — link the system SQLite now (`sqlite-unbundled`)

- Pros: could get a fixed library without waiting for sqlx, if the
  distribution ships 3.54.0 or a backport.
- Cons: a dependency-bump decision with build changes on every machine and
  in the image; `cargo deny` still flags the lockfile, so it needs the same
  ignore anyway. Belongs to MB-0041's main CRP.

## Recommendation and question

Recommended option: **A**, review by **2026-12-31**.
Because: the exposure is low and the alternative masks other failures.

Crisp question to answer: may `deny.toml` ignore RUSTSEC-2026-0459 until
2026-12-31 (or the date you set), to be removed by MB-0041's fix?

Exposure assessment for the security owner:

- The attack needs SQLite to open a database together with a crafted
  sibling `-wal` file of a different page size, both written by the
  attacker. It's local; nothing reaches it over HTTP.
- The app opens only `/data/kauppa.db` on the droplet's volume. Only the
  app container (uid 10001) and the backup container write there. An
  attacker who can write a `-wal` there can already rewrite the database.
- `kauppa-backup restore` brings in one file from Spaces, a `VACUUM INTO`
  snapshot with no `-wal`, and first moves the old `kauppa.db`, `-wal`,
  `-shm` and `-journal` aside (`deploy/backup/kauppa-backup`). So a restore
  can't pair a foreign `-wal` with the database.
- Development machines open only their own `data/kauppa.db` and test temp
  files.

## Blast radius and rollback

What can break: nothing at runtime; only `cargo deny`'s verdict changes.
Who is affected: CI for every PR.
Rollback plan: remove the entry (one line).

## Minimal context

- `.agentic/missions/MB-0041-sqlite-wal-advisory.md`, `.agentic/continuity/MB-0041.md`
- `.agentic/resolutions/RR-0007-actix-advisory-upgrade.md` (constraint 2)
- `deny.toml`, `deploy/compose.yaml`, `deploy/backup/kauppa-backup`

## Evidence bundle (Hardened)

- PR #44's `cargo deny` job (run 42663506680) and PR #45's: only
  `error[vulnerability]: libsqlite3-sys via C SQLite CVE-2026-119375`
  (RUSTSEC-2026-0459); bans, licenses, sources ok.
- crates.io dependency data: sqlx-sqlite 0.8.6 `libsqlite3-sys ^0.30.1`;
  0.9.0 `>=0.30.1, <0.38.0`; `libsqlite3-sys` 0.39.0 (2026-10-24) and 0.39.1
  (2026-10-25), `rust_version` 1.88. sqlx `main`'s `sqlx-sqlite/Cargo.toml`:
  `>=0.30.1, <0.40.0`.
- With the entry (this branch): `cargo deny check` → advisories ok, bans ok,
  licenses ok, sources ok (cargo-deny 0.20.2).
