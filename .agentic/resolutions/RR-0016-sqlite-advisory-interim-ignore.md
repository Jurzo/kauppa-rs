# Resolution Record — Interim ignore for RUSTSEC-2026-0459 (SQLite < 3.54.0)

```yaml
---
id: RR-0016
closes: CRP-0016
mission: MB-0041
decided_by: "@juuso.kalliomaki"
role: security
risk_tier: low
decided_at: 2026-11-09T19:38:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0041-sqlite-wal-advisory.md
  - .agentic/consults/CRP-0016-sqlite-advisory-interim-ignore.md
---
```

## Decision and outcome

Approve CRP-0016 **Option A**: `deny.toml` ignores RUSTSEC-2026-0459 until
**2026-12-31**, as an interim measure inside MB-0041.

Covers:

- the single `[advisories] ignore` entry for RUSTSEC-2026-0459, with a
  `reason` that names this RR and the review date;
- the roll-back below.

Does not cover:

- any other advisory ID, or ignoring by crate;
- any dependency, feature or `Cargo.lock` change (MB-0041's main CRP);
- extending the date (needs an amendment or a new RR).

## Rationale and trade-offs

- **A — chosen.** The attack needs attacker-written database and `-wal`
  files on the data volume, where only the app and the backup job write;
  a restore brings in one snapshot file and moves the old `-wal` aside.
  The ignore makes `cargo deny` meaningful again for every other change.
- **B — rejected.** Merging past a known red check would hide new
  advisories, license or source problems.
- **C — deferred** to MB-0041's main CRP; it needs the same ignore anyway.

## Approvals and authority

Approved by security-owner @juuso.kalliomaki.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0016-sqlite-advisory-interim-ignore.md`
- Mission Brief: `.agentic/missions/MB-0041-sqlite-wal-advisory.md`
- Evidence: CRP-0016 → Evidence bundle; https://rustsec.org/advisories/RUSTSEC-2026-0459

## Constraints and follow-on rules (Hardened)

1. **One ID.** The entry ignores RUSTSEC-2026-0459 only. Any new advisory,
   including another one on `libsqlite3-sys`, fails CI as before.
   RR-0007 constraint 2 otherwise stands.
2. **Review date 2026-12-31.** By then MB-0041 either removes the entry
   (fixed SQLite in the lockfile) or its Continuity Pack records why not,
   and the owner amends this RR to extend it. MB-0041's pack tracks the
   date.
3. **Removed by the fix.** MB-0041's fix PR deletes the entry; its P2
   requires `cargo deny check` to pass without it.
4. **No new exposure.** While the entry stands, nothing may open SQLite
   database or `-wal` files from an untrusted source (e.g. user uploads or
   importing databases); such a change needs security review.

## Roll-back into the system (Hardened)

To be applied in the PR that adds the entry:

- `MB-0041`: `resolutions: [RR-0016]`.
- `CRP-0016`: `state: open` → `answered`.

No Mentorship Pack or Runbook change.

## Supersession and history

Status: active. Supersedes nothing.
