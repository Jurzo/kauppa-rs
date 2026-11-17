# Mission Brief — Replace chrono, so RR-0051's interim ignore can go

```yaml
---
id: MB-0064
owner: "@juuso-kalliomaki"
goal: "The workspace no longer depends on chrono (RUSTSEC-2026-0543, unmaintained), every stored timestamp keeps reading, comparing and sorting exactly as before, and `deny.toml` loses RR-0051's interim ignore before its review date, 2027-02-28."
non_goals:
  - "Changing what any timestamp means, its precision, or when anything expires (invitations, sessions, archive dates)."
  - "Time zones: dates shown stay as they are today (UTC, formatted per language)."
  - "Other dependency upgrades than the ones the replacement needs."
  - "Session or identity cookie handling (actix-session and actix-identity keep their own `time` use)."
acceptance:
  - id: P1
    property: "chrono is gone from `Cargo.lock`, from both crates' `Cargo.toml` and from sqlx's features; `deny.toml` has no entry for RUSTSEC-2026-0543; `cargo deny check` passes."
    check: cargo deny check
  - id: P2
    property: "A timestamp written after the change is stored as the same text a chrono build wrote for the same instant (RFC 3339, `+00:00`, sqlx's `AutoSi` fraction), unless the data RR approves another format with a migration that rewrites every stored timestamp to it."
    check: cargo test -p kauppa-core repo::sqlite::tests::timestamp_text_unchanged
  - id: P3
    property: "Rows written by the chrono build (copied from a database at the newest migration) read back as the same instants, and `expires_at > ?` comparisons and `created_at`/`updated_at` ordering give the same answers across old and new rows; the whole-second rule of RR-0012 holds."
    check: cargo test -p kauppa-core repo::sqlite::tests::old_timestamps_compare_and_sort
  - id: P4
    property: "Dates and times shown on pages are unchanged in both languages (the `i18n` formatting, the archive date, the admin portal)."
    check: cargo test -p kauppa-web i18n::tests
  - id: P5
    property: "No behaviour change: every existing test passes, with only the edits the new types force (listed in the PR); the browser suite passes; clippy pedantic + nursery with `-D warnings` and no new `#[allow]`."
    check: cargo test --workspace --all-targets
  - id: P6
    property: "The deploy image builds and passes its smoke tests."
    check: scripts/deploy/smoke_image.sh
context:
  - Cargo.toml
  - Cargo.lock
  - deny.toml
  - crates/kauppa-core/Cargo.toml
  - crates/kauppa-web/Cargo.toml
  - crates/kauppa-core/src/domain/
  - crates/kauppa-core/src/repo/
  - crates/kauppa-core/src/repo/sqlite/mod.rs
  - crates/kauppa-core/src/export/
  - crates/kauppa-web/src/i18n.rs
  - crates/kauppa-web/src/i18n/
  - crates/kauppa-web/src/admin/bootstrap.rs
  - crates/kauppa-web/src/routes/
  - migrations/
  - .agentic/runbooks/dependency-bump.md
  - .agentic/mentorship/persistence.md
  - .agentic/resolutions/RR-0012-invite-only-registration.md
  - .agentic/resolutions/RR-0051-actix-http-and-chrono-advisories.md
autonomy:
  may_decide:
    - "mechanical type and call-site changes inside `domain/`, `repo/`, `export/`, `i18n` and the routes, once the replacement and its encoding are approved"
    - "test names inside the modules named above"
  escalate:
    - { when: "the replacement crate: `time` (already locked through actix-web and cookie; sqlx has a `time` feature) or `jiff` (new; no sqlx 0.8 support), its version and features, and sqlx's feature change", to: platform-owner }
    - { when: "the stored text: keep chrono's format through our own encoding (a newtype or a helper at the bind sites), switch to `time`'s format (`Z`) with a migration that rewrites every stored timestamp, or another way", to: data-owner }
    - { when: "a migration is needed (migrations/ is a trigger path; append-only)", to: data-owner }
    - { when: "any change to expiry, session or invitation timing, or to what a timestamp means", to: security }
  forbidden:
    - "a `Cargo.toml` change without an approved RR"
    - "edit committed migrations"
    - "store timestamps in two formats at once"
    - "lower lint strictness"
    - "extend RR-0051's ignore past 2027-02-28 without a consult"
runbook: .agentic/runbooks/dependency-bump.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/persistence.md@v1
  - .agentic/mentorship/security.md@v1
resolutions: [RR-0051]
---
```

## Context

Requested by the owner (2026-11-17): "Create the brief for chrono". RR-0051
(MB-0063) ignores RUSTSEC-2026-0543 (chrono unmaintained) in `deny.toml`
until 2027-02-28 and leaves the replacement to a later mission: this one.

What exists:

- **chrono 0.4.45** (`clock`, `serde`) is a direct dependency of
  `kauppa-core` and `kauppa-web` and an sqlx feature. About 41 files use it:
  `domain/` types carry `DateTime<Utc>` (serialised with serde), `repo/`
  binds and reads them, `export/` and the `i18n` formatting (`%-d.%-m.%Y`
  and `%Y-%m-%d`) render them, and tests build them with `Duration::days`.
  Outside tests, `kauppa-web` uses it in `i18n`, `admin/bootstrap.rs` and the
  routes.
- **`time` 0.3.55** is already locked (actix-web, cookie). sqlx 0.8 has a
  `time` feature; `OffsetDateTime` decodes RFC 3339 text with `+00:00`, so
  rows written today read back.
- **Stored text.** sqlx writes chrono's `DateTime<Utc>` as
  `to_rfc3339_opts(AutoSi, false)` (`…+00:00`, 0/3/6/9 fraction digits);
  it writes `time`'s `OffsetDateTime` as `Rfc3339` (`…Z`). SQL compares
  (`expires_at > ?`, three queries in `invitation.rs`) and sorts
  (`ORDER BY created_at` / `updated_at`, five queries) these as text, and
  RR-0012 binds compared timestamps at whole seconds so the text has a
  fixed width (`whole_seconds` in `repo/sqlite/mod.rs`). Mixing `+00:00`
  and `Z` rows would misorder them, so the format is a data decision.

## Conceptual plan

1. **Clarify (S1).** One CRP: the replacement (platform), the stored
   format and any migration (data), anything touching expiry (security).
2. Tests first: P2's and P3's tests on the chrono build, so they pin
   today's text and behaviour.
3. The `Cargo.toml` change and the type changes, crate by crate; the
   encoding as decided.
4. Remove RR-0051's `deny.toml` entry; gates, browser suite, image smoke
   test.

Slow-mode triggers: a decoded instant differs; a test needs more than a
type edit; a query needs rewriting.

## Validation plan

- P2 and P3 written and passing before the switch, and unchanged after.
- `cargo deny check`, the usual gates, the browser suite, the image smoke
  test in CI.
