# Consultation Request Pack — The actix-http advisories and chrono's "unmaintained" advisory

```yaml
---
id: CRP-0051
mission: MB-0063
owner: "@juuso-kalliomaki"
target_role: platform
risk_tier: medium
urgency: high
state: answered
runbook_trigger: "dependency-bump.md entry gate (a `Cargo.lock` change outside an in-scope task; any `deny.toml` change); MB-0063 autonomy.escalate (the actix-http version, chrono → platform-owner; the timestamp format → data-owner); feature.md escalation trigger `deny.toml` (platform:lint-config)"
---
```

## Decision statement

Which actix-http release fixes RUSTSEC-2026-0552, -0553 and -0554 (Q1), and
how is RUSTSEC-2026-0543, chrono unmaintained, handled (Q2)?

## Current state

- actix-http **3.20.3**, through actix-web 4.18.2 (`Cargo.toml`: actix-web
  `4`, `default-features = false`). Call sites: none direct; the server
  in `main.rs` (`HttpServer`, `h1_allow_half_closed(false)`, RR-0050).
- chrono **0.4.45**, features `clock` and `serde`, a direct dependency of
  `kauppa-core` and `kauppa-web` and sqlx's `chrono` feature; 41 files, 108
  `DateTime<Utc>`, `Duration`, `TimeZone`, `SubsecRound`.
- `cargo deny check` on `main` fails on all four advisories; `main`'s last
  green run was before they were published.

## Q1 — The actix-http version (platform)

### Option A — 3.21.3, the security release

`cargo update -p actix-http --precise 3.21.3`, lockfile only.

- Pros:
  - The smallest step that fixes all three advisories.
  - Released 2026-11-15; the security-fix exception to RR-0032's 7-day
    cooldown applies.
  - Tried: every gate passes (see the evidence bundle).
- Cons:
  - Also brings 3.20.4–3.21.0's changes: `TCP_NODELAY` on by default,
    HTTP/1 memory improvements, HTTP/2 GOAWAY on shutdown, an HTTP/1
    client framing fix (we use no client).
- Risk: low.
- Cost / timeline: one commit.

### Option B — 3.22.0, the newest

- Pros:
  - The newest release; nothing to bump next week.
- Cons:
  - Released today (2026-11-17), not a security release, so inside
    RR-0032's cooldown; its changes (`InfallibleStream`, comma-delimited
    header splitting with quoted strings) aren't needed.
- Risk: low.
- Cost / timeline: one commit. Also tried: every gate passes.

### Option C — Wait for Dependabot

- Cons: blocked: RR-0032 class 1 needs every check green, and chrono's
  advisory keeps `cargo deny` red. The vulnerabilities stay open meanwhile.

**Recommended: A.**

## Q2 — chrono (platform; data if replaced)

### Option A — An interim ignore with a review date, and a later mission to replace it

`deny.toml` gains, beside RR-0016's entry:
`{ id = "RUSTSEC-2026-0543", reason = "chrono unmaintained, no known vulnerability; replacement is MB-<next>. RR-<this>, review by 2027-02-28." }`

- Pros:
  - "Unmaintained" is not a vulnerability; the code works as today.
  - `cargo deny` goes green now, so Q1's fix and Dependabot's lockfile
    updates (RR-0032) can merge.
  - The replacement gets its own mission and its own data decision on
    the timestamp format.
- Cons:
  - A second ignore in `deny.toml`; a future chrono vulnerability would
    have no fix.
- Risk: low until a chrono vulnerability appears; `cargo deny` still
  reports any new chrono advisory, because the ignore names one id.
- Cost / timeline: one commit; a follow-up brief.

### Option B — Replace chrono with `time` now

- Pros:
  - `time` 0.3.55 is already in the tree (actix-web, cookie): no new
    package; sqlx has a `time` feature.
  - No ignore.
- Cons:
  - A `Cargo.toml` change in both crates and sqlx's features, and edits in
    41 files.
  - sqlx writes `time`'s UTC timestamps with `Z`, chrono's with `+00:00`.
    SQL compares (`expires_at > ?` in invitations) and sorts
    (`ORDER BY created_at`, `updated_at`) them as text, and RR-0012
    constraint 7 relies on a fixed-width text (`whole_seconds` in
    `repo/sqlite/mod.rs`), so old and new rows can misorder. It needs a
    data-owner decision: write chrono's format with a custom encoding, or a
    migration that rewrites stored timestamps.
  - Delays Q1's fix, or Q1 merges with `cargo deny` still red.
- Risk: medium (data ordering, expiry checks).
- Cost / timeline: a mission of its own, with a data consult.

### Option C — Replace chrono with `jiff`

- Cons: a new crate, and sqlx 0.8 has no `jiff` support, so every
  timestamp needs a hand-written encoding. Not recommended.

### Option D — Ignore it with no review date

- Cons: unlike RR-0016's interim ignore, nothing would bring it back for
  review; MB-0063 forbids a permanent ignore without an RR. Not
  recommended.

**Recommended: A**, with the replacement (Option B's work, with its data
consult) as a later mission.

## Recommendation and question

Recommended: **Q1 A, Q2 A.**

Because: Q1 A fixes the three HTTP/1 vulnerabilities with the smallest
proven step. Q2 A turns `cargo deny` green now without a risky change to
how timestamps are stored, and gives the replacement its own mission.

Crisp questions to answer:
- **Q1:** A (3.21.3), B (3.22.0) or C (wait)?
- **Q2:** A (interim ignore, review by 2027-02-28, and a later mission), B
  (replace with `time` now), C or D? With A, is 2027-02-28 the right review
  date?

## Blast radius and rollback

What can break: HTTP/1 request handling (Q1); nothing at runtime for Q2 A.
Who is affected: every user, through the server.
Rollback plan: revert the lockfile commit (actix-http back to 3.20.3, and
the advisories again); revert the `deny.toml` line.

## Minimal context

- Advisories (cargo-deny 0.20.2, the advisory database as fetched by CI on
  2026-11-17):
  - RUSTSEC-2026-0552, bare LF in chunked trailers; fixed in 3.21.1
    (GHSA-9w4r-xc43-v236).
  - RUSTSEC-2026-0553, malformed quoted chunk extensions end bodies early;
    fixed in 3.21.3 (GHSA-9543-cj4x-h76q).
  - RUSTSEC-2026-0554, partial requests on keep-alive connections bypass
    the request header timeout; fixed in 3.21.2 (GHSA-54pm-2wm5-hpmg).
  - RUSTSEC-2026-0543, chrono unmaintained; alternatives `jiff`, `time`.
- The deployment sits behind Caddy 2.11.9; the advisories say the tested
  intermediaries rejected or normalised the malformed input. RUSTSEC-2026-0554
  depends on upstream connection controls; Caddy's keep-alive to the app
  isn't evaluated here.
- RR-0016 (the SQLite advisory's interim ignore) is the precedent for Q2 A.
- RR-0032: lockfile-only bot updates need every check green with no
  `deny.toml` change.

## Evidence bundle (Hardened)

- **Supply-chain check (crates.io, 2026-11-17):** actix-http, owners
  `fafhrd91`, `JohnTitor`, `robjtede` and the `actix:core` team; 90.9 M
  downloads (13.6 M recent); MIT OR Apache-2.0 (already allowed); 3.21.1,
  3.21.2 and 3.21.3 released 2026-11-15, 3.22.0 on 2026-11-17;
  `rust-version` 1.89 (the toolchain is 1.100.0).
- **The lockfile diff** for either version: actix-http's version and
  checksum, and its `windows-sys` requirement 0.52.0 → 0.61.2 (already in
  the lockfile); no new package.
- **Tried on a scratch worktree of `main` (`d3205d3`), for both 3.21.3 and
  3.22.0:** clippy `-D warnings` clean (kauppa-core's known unused-dependency
  warnings only); `cargo test --workspace --all-targets` 106 + 3 + 233;
  `scripts/browser/run.sh` 12 of 12; RR-0050's half-close check on the
  release binary: three closed streams freed within 3 s. `cargo deny check`:
  only RUSTSEC-2026-0543 left.
- actix-http 3.22.0's dispatcher still starts the shutdown on a read EOF
  when `h1_allow_half_closed` is `false` (`h1/dispatcher.rs`), as 3.20.3 did.
- **Timestamp formats (sqlx-sqlite 0.8.6):** chrono `DateTime<Utc>` →
  `to_rfc3339_opts(AutoSi, false)` (`+00:00`); `time` `OffsetDateTime` →
  `Rfc3339` (`Z` for UTC). Text comparisons on `expires_at` (three queries
  in `invitation.rs`, bound at whole seconds for RR-0012 constraint 7) and
  text ordering on `created_at`/`updated_at` (five queries).

## Owner's answer (2026-11-17)

"Brief is ok. Q1 and Q2 as recommended with the recommended review date":
Q1 A (actix-http 3.21.3, lockfile only); Q2 A (an interim `deny.toml`
ignore for RUSTSEC-2026-0543, review by 2027-02-28, and a later mission to
replace chrono).

Recorded in RR-0051 (scaffold; the owner signs).
