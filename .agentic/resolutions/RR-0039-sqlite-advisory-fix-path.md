# Resolution Record — RUSTSEC-2026-0459: wait for an sqlx release, Alpine 3.25 for the backup image, a three-month default at the review

```yaml
---
id: RR-0039
closes: CRP-0039
mission: MB-0041
decided_by: "<resolver's GitHub handle, e.g. @Jurzo>"
role: platform
risk_tier: low
decided_at: <RFC 3339 UTC time of signing, e.g. 2026-11-14T00:00:00Z>
status: <approved | rejected>
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0041-sqlite-wal-advisory.md
  - .agentic/consults/CRP-0039-sqlite-advisory-fix-path.md
---
```

> **Scaffold, not yet signed.** The owner answered CRP-0039 in the session
> of 2026-11-14 ("Go with the recommendations"). Before signing, the
> resolver checks the sub-answers, removes this note, and fills
> `decided_by`, `decided_at`, `status` and the Approvals line.
> `validate_artifacts.py` fails on those placeholders until then.

## Decision and outcome

Approve CRP-0039 Q1 A, Q2 A with C, and Q3 as recommended.

Sub-answers:

1. **The app (Q1 A, platform).** Wait for an sqlx release whose
   `sqlx-sqlite` allows `libsqlite3-sys` ≥ 0.39 (sqlx `main` already
   does). Then upgrade under the dependency-bump runbook:
   `cargo update -p libsqlite3-sys` as its own commit, MB-0041's P1–P5,
   `.sqlx/` regenerated with `scripts/sqlx-prepare.sh` (MB-0049), and
   RR-0016's ignore removed in the same PR. If that release is a breaking
   one (0.9.x or later), its changes are reviewed in that PR. No vendored
   driver (B), no git source (C); system linking (D) is rejected because
   Debian trixie's `sqlite3` stays vulnerable (`<no-dsa>`).
2. **The backup image (Q2 A with C, platform).** Move
   `deploy/backup/Dockerfile` to Alpine 3.25 and its 3.54 `sqlite` (the
   `sqlite~3.53` pin changes with it) when 3.25 is released, under the
   dependency-bump runbook, not RR-0032's standing approval. Until then
   the backup job's exposure is accepted as CRP-0039 assessed it: it opens
   only the live database and snapshots, never a file next to an
   attacker-written `-wal`. No packages from edge.
3. **The review date (Q3, security).** At RR-0016's review on
   2026-12-31, the agent re-checks crates.io, the Debian tracker and
   Alpine. If no sqlx release allows 0.39 by then, the default is a new
   RR extending the ignore by three months (to 2027-03-31) with CRP-0039's
   exposure assessment, re-checked; the owner may instead choose a trial
   of the vendored driver (CRP-0039 Q1 B).

Covers:

- MB-0041's waiting state, the eventual upgrade PR as above, and the
  backup image's Alpine move;
- the roll-backs below.

Does not cover:

- any change to `deny.toml` now (RR-0016's ignore stands as is);
- extending the ignore (sub-answer 3 needs its own RR);
- a vendored or git sqlx, or system linking.

## Rationale and trade-offs

- **Wait (A)**: the fix is already on sqlx `main`; a release brings it
  with the least risk. B carries an untested fork of the database driver
  across nine `libsqlite3-sys` releases; C brings unreleased code and a
  git source; D doesn't fix trixie.
- **Alpine 3.25** over edge packages: a supported branch, no mixed
  repositories. The backup job's exposure is low (CRP-0039 → Exposure).
- **A default at the review** so the date can't lapse into a red
  `cargo deny` on every PR.

## Approvals and authority

<resolver: e.g. "@Jurzo as platform for sub-answers 1 and 2, and as
security for 3.">

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0039-sqlite-advisory-fix-path.md`
- Mission Brief: `.agentic/missions/MB-0041-sqlite-wal-advisory.md`
- Related: RR-0016 (the interim ignore), RR-0032 (standing approvals),
  RR-0037 (the `.sqlx/` cache).
- Evidence: CRP-0039 → Current state and Evidence bundle (2026-11-14).

## Constraints and follow-on rules (Hardened)

1. RR-0016's ignore is removed by the PR that brings `libsqlite3-sys`
   ≥ 0.39, never kept past it.
2. No vendored, forked or git sqlx without a new RR.
3. The backup image's `sqlite` pin and Alpine branch move together.

## Roll-back into the system (Hardened)

To be applied after this record is signed, with the owner's OK:

- `MB-0041`: `resolutions: [RR-0016]` → `[RR-0016, RR-0039]`; the
  Conceptual plan's step 1 marked done with the chosen path, step 2 (the
  ignore PR) marked done by #46; Context's dependency facts dated
  2026-11-14.
- `CRP-0039`: `state: open` → `answered`.

No Mentorship Pack or Runbook change.

## Supersession and history

Status: active once signed. Supersedes nothing; follows RR-0016.
