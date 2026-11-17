# Mission Brief — Fix the actix-http advisories and settle chrono's "unmaintained" advisory

```yaml
---
id: MB-0063
owner: "@juuso-kalliomaki"
goal: "The app runs an actix-http release without RUSTSEC-2026-0552, -0553 and -0554, and RUSTSEC-2026-0543 (chrono unmaintained) is handled as the RR decides, so `cargo deny check` passes again on `main`."
non_goals:
  - "Other dependency upgrades than the ones the fix needs (actix-http; and, only if the RR chooses it, replacing chrono)."
  - "Changing server settings: keep-alive, timeouts, `h1_allow_half_closed` (RR-0050)."
  - "Changing how timestamps are stored, unless the RR chooses to replace chrono and a data RR approves the format."
acceptance:
  - id: P1
    property: "`Cargo.lock` has actix-http 3.21.3 or later, so `cargo deny check advisories` reports none of RUSTSEC-2026-0552, -0553 or -0554. The update is lockfile-only and its own commit (`cargo update -p actix-http --precise <version>`)."
    check: cargo deny check advisories
  - id: P2
    property: "`cargo deny check` passes. RUSTSEC-2026-0543 is either gone (chrono replaced) or listed in `deny.toml`'s `ignore` with its reason and a review date, as the RR approves."
    check: cargo deny check
  - id: P3
    property: "No behaviour change: every existing test passes unchanged; the browser suite passes; a closed page's live stream is still freed at once (RR-0050)."
    check: cargo test --workspace --all-targets
  - id: P4
    property: "The deploy image still builds and passes its smoke tests."
    check: scripts/deploy/smoke_image.sh
context:
  - Cargo.toml
  - Cargo.lock
  - deny.toml
  - crates/kauppa-core/Cargo.toml
  - crates/kauppa-web/Cargo.toml
  - crates/kauppa-web/src/main.rs
  - .agentic/runbooks/dependency-bump.md
  - .agentic/resolutions/RR-0016-sqlite-advisory-interim-ignore.md
  - .agentic/resolutions/RR-0032-ci-pinning-and-maintenance.md
  - .agentic/resolutions/RR-0050-close-streams-on-client-eof.md
  - .agentic/mentorship/persistence.md
autonomy:
  may_decide:
    - "the wording of the `deny.toml` comment, if an ignore is approved"
  escalate:
    - { when: "the actix-http version: the security release 3.21.3, or the newest 3.22.0 (released 2026-11-17, inside RR-0032's 7-day cooldown and not a security release) (decided: RR-0051, 3.21.3)", to: platform-owner }
    - { when: "chrono: an interim `ignore` in `deny.toml` (lint config, a trigger path) and its review date, or replacing it (a `Cargo.toml` change) (decided: RR-0051, an interim ignore, review by 2027-02-28; replacement is a later mission)", to: platform-owner }
    - { when: "replacing chrono changes the text written for timestamps in SQLite, which SQL compares and sorts as text (decided: RR-0051, not in this mission; the replacement mission consults on it)", to: data-owner }
  forbidden:
    - "a `Cargo.toml` change without an approved RR"
    - "ignore an advisory permanently or without an approved RR"
    - "lower lint strictness"
    - "edit committed migrations"
runbook: .agentic/runbooks/dependency-bump.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/persistence.md@v1
  - .agentic/mentorship/security.md@v1
resolutions: [RR-0051]
---
```

## Context

Requested by the owner (2026-11-17), after `cargo deny` failed on PRs #125
and #126 (merged with it red): "Start the dependency-bump work".

Four advisories were published since `main`'s last green run:

- **RUSTSEC-2026-0552** (fixed in actix-http 3.21.1): a bare LF in a
  chunked trailer is skipped, so a following pipelined request can be
  swallowed or body decoding stalls.
- **RUSTSEC-2026-0553** (fixed in 3.21.3): a CR inside an unclosed quoted
  chunk extension ends a chunked body early; a following pipelined request
  is dispatched.
- **RUSTSEC-2026-0554** (fixed in 3.21.2): the request header timeout
  applies only to the first request on a keep-alive connection, so one
  stalled partial request holds a connection for ever; repeated, it can
  exhaust connections or file descriptors.
- **RUSTSEC-2026-0543**: chrono is unmaintained (all versions); suggested
  alternatives `jiff` and `time`. No fixed version exists.

What exists:

- actix-http 3.20.3 comes in through actix-web 4.18.2; `Cargo.toml` asks for
  actix-web `4`, so a lockfile-only update reaches 3.21.3 or 3.22.0.
- chrono 0.4.45 is a direct dependency of both crates (`clock`, `serde`)
  and an sqlx feature (`chrono`); 41 files use it (108 `DateTime<Utc>`).
  `time` 0.3.55 is already in the tree (actix-web, cookie).
- Timestamps are stored as TEXT. sqlx writes chrono's `DateTime<Utc>` as
  RFC 3339 with `+00:00`; its `time` support writes `Z`. SQL compares
  (`expires_at > ?`, at whole seconds for RR-0012 constraint 7) and sorts
  (`ORDER BY created_at`) them as text, so mixing the two formats can
  misorder rows.
- RR-0032's standing approval covers lockfile-only bot updates only when
  every check is green with no `deny.toml` change; chrono's advisory keeps
  `cargo deny` red, so the bot route is blocked until it is settled.

## Conceptual plan

1. **Clarify (S1).** CRP-0051: the actix-http version and chrono (platform);
   the timestamp format, only if chrono is replaced (data). Done: CRP-0051
   answered, RR-0051 approved (Q1 A, Q2 A, review by 2027-02-28).
2. The actix-http lockfile update, its own commit (P1).
3. chrono as decided: the `deny.toml` entry, or a separate mission to
   replace it (P2).
4. Gates, the browser suite and the image smoke test (P3, P4).

## Validation plan

- `cargo deny check`, the usual gates, the browser suite, the RR-0050
  half-close check on the release binary, and the image smoke test in CI.
