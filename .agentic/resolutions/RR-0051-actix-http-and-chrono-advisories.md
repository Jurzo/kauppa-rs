# Resolution Record — actix-http 3.21.3 for three HTTP/1 advisories; chrono's "unmaintained" advisory ignored until 2027-02-28

```yaml
---
id: RR-0051
closes: CRP-0051
mission: MB-0063
decided_by: "<resolver's GitHub handle, e.g. @Jurzo>"
role: platform
risk_tier: medium
decided_at: <RFC 3339 UTC time of signing, e.g. 2026-11-17T00:00:00Z>
status: <approved | rejected>
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0063-actix-http-and-chrono-advisories.md
  - .agentic/consults/CRP-0051-actix-http-and-chrono-advisories.md
---
```

## Decision and outcome

Approve CRP-0051 Q1 A and Q2 A.

1. **actix-http (Q1 A).** `cargo update -p actix-http --precise 3.21.3`,
   in a commit of its own that changes only `Cargo.lock` (actix-http's
   version and checksum, its `windows-sys` requirement). This fixes
   RUSTSEC-2026-0552, -0553 and -0554. No `Cargo.toml` change.
2. **chrono (Q2 A).** `deny.toml`'s `[advisories] ignore` gains one entry,
   in a commit of its own, with a comment naming RR-0051 and MB-0063:
   `{ id = "RUSTSEC-2026-0543", reason = "chrono unmaintained, no known vulnerability; a later mission replaces it. RR-0051, review by 2027-02-28." }`
3. **Replacing chrono** is a later mission, with its own consult on the
   stored timestamp format (data-owner; RR-0012 constraint 7). Its brief is
   drafted when the owner asks for it, and before 2027-02-28.
4. **Gates.** `cargo deny check` passes; the usual gates, the browser
   suite, RR-0050's half-close check on the release binary and the image
   smoke test pass.

Covers:
- the lockfile commit and the `deny.toml` entry above;
- the roll-back below.

Does not cover:
- actix-http 3.22.0 or any other update;
- a `Cargo.toml` change, or replacing chrono;
- ignoring any other advisory, or this one past 2027-02-28.

## Rationale and trade-offs

- 3.21.3 is the security release that fixes all three advisories; 3.22.0
  is a feature release inside RR-0032's cooldown.
- "Unmaintained" is not a vulnerability. The ignore names one id, so a
  new chrono advisory still fails `cargo deny`. A green `cargo deny` also
  unblocks Dependabot's lockfile updates (RR-0032 class 1).
- Replacing chrono changes stored timestamp text that SQL compares and
  sorts, so it gets its own mission and data decision.
- Rejected: 3.22.0 (Q1 B); waiting for Dependabot (Q1 C, blocked);
  replacing chrono with `time` now (Q2 B) or `jiff` (Q2 C); an ignore
  without a review date (Q2 D).

## Approvals and authority

<resolver: e.g. "@Jurzo as platform.">

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0051-actix-http-and-chrono-advisories.md`
  (its evidence bundle)
- Mission Brief: `.agentic/missions/MB-0063-actix-http-and-chrono-advisories.md`
- Continuity Pack: `.agentic/continuity/MB-0063.md`
- Related: RR-0012 (constraint 7), RR-0016, RR-0032, RR-0050.

## Constraints and follow-on rules (Hardened)

1. **Lockfile only.** The actix-http update changes only `Cargo.lock`; a
   new package in the diff stops the work and asks.
2. **One ignore, dated.** The `deny.toml` entry names RUSTSEC-2026-0543
   only, with the review date 2027-02-28. Extending it needs a consult; a
   new chrono advisory is not covered.
3. **Review.** By 2027-02-28 the entry is removed by the replacement
   mission, or a consult extends it.

## Roll-back into the system (Hardened)

To be applied after this record is signed, with the owner's OK:

- `MB-0063`: `resolutions: [RR-0051]`; plan step 1 marked done; the three
  escalate entries marked decided (RR-0051).
- `CRP-0051`: `state: open` → `answered`.

## Supersession and history

Status: active once signed. Amends nothing; supersedes nothing.
