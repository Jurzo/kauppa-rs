# Mission Brief — SQLite WAL mode and busy timeout

```yaml
---
id: MB-0025
owner: "@juuso-kalliomaki"
goal: "Make concurrent use safe under real traffic: the pool opens SQLite in WAL mode with a busy timeout and `synchronous=NORMAL`, so readers don't block the writer and short write contention waits instead of failing with \"database is locked\"."
non_goals:
  - Schema changes or migrations.
  - "Running more than one app process against the file."
  - "Backups (MB-0022 P5), though WAL changes what a safe backup is; see Context."
  - Tuning the pool size beyond keeping it consistent with the chosen settings.
acceptance:
  - id: P1
    property: "Every pooled connection reports `journal_mode=wal`, `synchronous=1` (NORMAL), `foreign_keys=1`, and a `busy_timeout` of at least 5000 ms."
    check: cargo test -p kauppa-core repo::sqlite::tests::pragmas_on_every_connection
  - id: P2
    property: "Concurrent writers don't fail: 8 tasks each adding 50 list items to one file-backed database all succeed, and every item is there afterwards."
    check: cargo test -p kauppa-core repo::sqlite::tests::concurrent_writes_succeed
  - id: P3
    property: "A reader isn't blocked by an open write transaction: a read started while another connection holds a write transaction returns within 100 ms."
    check: cargo test -p kauppa-core repo::sqlite::tests::reads_during_write
  - id: P4
    property: "An existing database in rollback-journal mode opens, switches to WAL, and keeps all its data; the existing repo suite passes unchanged."
    check: "cargo test -p kauppa-core repo::"
context:
  - crates/kauppa-core/src/repo/sqlite/mod.rs
  - .agentic/mentorship/persistence.md
  - .env.example
autonomy:
  may_decide:
    - "the exact busy timeout (5–30 s) and pool size, with the reasoning in the pack"
    - "whether settings come from `SqliteConnectOptions` methods or pragmas"
  escalate:
    - { when: "making any of these settings configurable through `.env`", to: platform-owner }
    - { when: "`synchronous=NORMAL` is judged not durable enough (lost data is unacceptable, AGENTS.md priority 1)", to: data-owner }
    - { when: "a dependency or sqlx feature change is needed", to: platform-owner }
  forbidden:
    - schema changes or migrations
    - weaken or delete existing repo tests
    - lower lint strictness
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/persistence.md@v1
resolutions: []
---
```

## Context

Found in the deployment assessment (2026-11-08). `SqlitePool::connect`
(`repo/sqlite/mod.rs`) sets only `create_if_missing` and `foreign_keys`,
with up to 8 connections. The journal mode and busy timeout are whatever
sqlx and SQLite default to; the code doesn't state them. In the rollback
journal, a writer blocks readers, and with several users on a shared list a
write that finds the database locked can fail.

WAL is the usual choice for one process with many connections. Two
consequences for operations, to note in MB-0022's operator guide:
- the database becomes three files (`kauppa.db`, `-wal`, `-shm`), all on the
  same volume;
- copying `kauppa.db` alone is not a backup. MB-0022 P5 already requires an
  online `.backup` or `VACUUM INTO`, which handles WAL correctly.

`synchronous=NORMAL` in WAL mode can lose the last transactions on power
loss, but not corrupt the database. Whether that's acceptable is the
escalation above.

## Conceptual plan

- Set the options on `SqliteConnectOptions` so every new connection gets
  them; assert them per connection in P1.
- Tests use a temp-file database: WAL doesn't apply to `:memory:`.

Slow-mode trigger: an existing test depends on rollback-journal behaviour.

## Validation plan

- P1–P3 in `repo::sqlite::tests` with a temp-file database.
- P4: open a database created in rollback-journal mode (fixture made in the
  test), run the full repo suite.
