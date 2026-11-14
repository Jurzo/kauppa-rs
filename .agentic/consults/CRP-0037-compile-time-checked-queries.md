# Consultation Request Pack — Compile-time-checked SQL: migrate all, migrate all but the list repository, or retire the plan

```yaml
---
id: CRP-0037
mission: MB-0049
owner: "@juuso-kalliomaki"
target_role: platform
risk_tier: low
urgency: low
state: open
runbook_trigger: "MB-0049 autonomy.escalate (the decision and its timing against sqlx 0.9 → platform-owner; the stale-cache tool in CI → platform-owner; the Mentorship Pack edit → owner); feature.md escalation trigger `.agentic/{mentorship,resolutions}/**`"
---
```

## Decision statement

Decide whether the SQLite repositories move from runtime `sqlx::query(…)`
to the compile-time-checked `sqlx::query!` macros with a committed offline
cache, and if so which modules and how CI keeps the cache honest, or
whether `persistence.md`'s "target state" paragraph is retired.

## Terms

- **Runtime query**: `sqlx::query("…").bind(…)`. The SQL is checked only
  when it runs, which in this repo means when a test runs it.
- **Query macro**: `sqlx::query!("…", args…)`. At compile time the macro
  asks a database to prepare the SQL. It checks that the tables and columns
  exist, that the argument count matches, and gives the result row typed
  fields.
- **Offline cache (`.sqlx/`)**: one JSON file per query, holding what the
  database answered. With the cache committed, builds need no database.
  `SQLX_OFFLINE=true` makes a missing entry an error.
- **Stale cache**: the code or the schema changed, but `.sqlx/` wasn't
  regenerated.

## Current state

- `persistence.md:36-39`: the scaffold uses runtime queries "so builds
  don't require a live DB"; **target state** is `query!`/`query_as!`, a
  `.sqlx/` cache and `SQLX_OFFLINE=true` in CI, "Track in a Mission Brief".
  `repo/sqlite/mod.rs:4` repeats it.
- `kauppa-core`'s SQLite code has **81 runtime queries** outside tests:
  `list.rs` 28, `product.rs` 18, `recipe.rs` 14, `invitation.rs` 8,
  `category.rs` 7, `user.rs` 4, `mod.rs` 2. No SQL is built at run time
  (no `format!`, `QueryBuilder` or `push_str` in the repositories).
- The workspace already enables sqlx's `macros` feature (`Cargo.toml:51-58`),
  so **no manifest change is needed** for any option.
- `.gitignore:30-31` ignores only `/.sqlx/.tmp`, so a root `.sqlx/` would
  be committed as is.
- The app reads `KAUPPA_DATABASE_URL`, not `DATABASE_URL`. A developer's
  `.env` therefore can't switch the macros into online mode by accident.
- **The list repository is special.** RR-0002 constraint 1 and RR-0009
  constraint 3 require one central access predicate. `list.rs` builds its
  queries from four shared SQL constants (`ACCESS_SELECT` used 3 times,
  `ITEM_SELECT` 4, `STORE_VIEW_SELECT` 2, `SHARE_SELECT` 3). The bypass
  guard in `repo/share.rs:622-705` allows exactly those constants by name.
- **sqlx 0.9.0 is released.** It needs Rust 1.94 (the toolchain is 1.99.1).
  It doesn't unblock MB-0041: `sqlx-sqlite` 0.9.0 requires
  `libsqlite3-sys >=0.30.1, <0.38.0`, and MB-0041 needs 0.39 or later.
- MB-0048 (in S1 in parallel) proposes splitting `list.rs` by area.

## The trial (2026-11-14)

`user.rs`'s four queries were converted in a scratch worktree, never
committed. A schema database was made from `migrations/*.sql` with Python's
`sqlite3` module. The details and commands are in the Evidence bundle.

| What | Result |
| --- | --- |
| Conversion | Mechanical. One override was needed: `created_at AS "created_at: DateTime<Utc>"` (SQLite stores text). Ids stay `String` and go through `parse_uuid` as today. |
| Cache | Building with `DATABASE_URL` set and `SQLX_OFFLINE_DIR=.sqlx` wrote 4 JSON files. **No `sqlx-cli` needed.** |
| Offline build | With no `DATABASE_URL`, the build used `.sqlx/` on its own. Same with `SQLX_OFFLINE=true`. |
| Query edited, cache not | `SQLX_OFFLINE=true` build fails: "no cached data for this query", pointing at the line. |
| Schema drift | Building against a database with `users.display_name` renamed fails: "no such column: display_name", pointing at the query. |
| Tests | `cargo test -p kauppa-core`: 95 passed, unchanged. |
| Build time | `kauppa-core` rebuild after an edit: 0.26 s before, 0.75 s online with the macros, 0.26 s offline. Negligible at this size; a full migration multiplies the per-query cost by about 20. |
| Composed SQL | `query!("…" + "…")` compiles: the macro joins string literals. `query!(frag!())` (a `macro_rules!` fragment) fails: "expected string literal". **A shared SQL constant can't be referenced from a macro query.** |

That last row decides `list.rs`. The central predicate would have to be
copied as literal text into each of its 12 uses. That turns "one constant
the guard allows by name" into "12 copies a test must prove identical".

## Options and trade-offs

### Q1 — How far to migrate (platform; security for `list.rs`)

**A: everything, `list.rs` included.**

- Pros: every query checked at compile time; one way of writing queries.
- Cons: `list.rs`'s shared constants become 12 literal copies. The bypass
  guard must be rewritten to check the copies match, which changes "how
  list queries are guarded" (RR-0002, RR-0009 constraint 3, a security
  decision). It also collides with MB-0048's split of the same file.
- Risk: medium (the security invariant is restated, not just kept).
  Cost: 81 queries, and a guard rewrite.

**B (recommended): everything except `list.rs` (and the two setup queries
in `mod.rs`).**

- That is `user`, `category`, `invitation`, `recipe` and `product`: **51
  queries**.
- `list.rs` stays runtime, behind its central constants and the unchanged
  bypass guard. Its 28 queries keep being checked by the list and sharing
  tests, which run every one (RR-0002 constraint 5).
- Pros:
  - Most queries gain compile-time checks of columns, argument counts and
    row types.
  - Nothing changes in the security-tier file.
  - No collision with MB-0048.
- Cons:
  - Two styles in one folder. `persistence.md` names the exception and the
    reason.
  - Changing a query in the five modules now needs the cache regenerated
    (one script, below).
- Risk: low. Cost: 51 queries, a script and a CI step.

**C: keep runtime queries; retire the target.** `persistence.md:36-39` and
`mod.rs:4` drop the target-state text and say why.

- Pros: no work, no cache to maintain, no new CI job.
- Cons: a column rename or a typo in SQL is found by a test run, not by the
  compiler. Every query has a test today, so the gap is feedback time, not
  coverage. It gives up AGENTS.md's correctness preference ("prefer
  explicit types and compile-time checks").
- Risk: low. Cost: a paragraph.

### Q2 — Keeping the cache honest in CI (platform)

Applies to A and B.

- **Every CI build runs offline:** `SQLX_OFFLINE=true` in the `ci.yml`
  environment, so a missing cache entry fails the build.
- **A staleness job** (recommended: a repository script, no new tool):
  `scripts/sqlx-prepare.sh [--check]` creates a temporary SQLite database
  from `migrations/*.sql` (Python's `sqlite3`, which `scripts/agentic/`
  already needs), builds `kauppa-core` online with `DATABASE_URL` pointing
  at it and `SQLX_OFFLINE_DIR` at a temporary folder, and then:
  - without `--check`, replaces `.sqlx/` (what a developer runs after
    changing a query or adding a migration);
  - with `--check`, fails if the temporary folder differs from `.sqlx/`
    (what CI runs).
  This catches both cases from the trial: a query edited without
  regenerating, and a migration that breaks a query.
- Alternative: install `sqlx-cli` pinned in CI and run
  `cargo sqlx prepare --workspace --check`, as the brief's P2 names it.
  It's the standard tool, but it's a new pinned binary in CI (more supply
  chain under RR-0032) and a `cargo install` on every run or a cache for it.

### Q3 — Timing against sqlx 0.9 (platform)

- **Recommended: migrate on 0.8 now.** The 0.9 upgrade is its own
  dependency-bump mission whenever it's wanted. With a cache in place, that
  PR also regenerates `.sqlx/` by running the script. The upgrade doesn't
  help MB-0041 (Current state), so there's no reason to wait for it.
- Alternative: upgrade to 0.9 first, then migrate, so the cache is
  generated once. It delays MB-0049 behind a bump nobody has scheduled.

### Q4 — Brief and pack amendments (owner, for information)

- P2's check becomes `scripts/sqlx-prepare.sh --check` (if Q2 recommended)
  plus the offline CI build.
- P3: "dynamic SQL that can't be a macro is listed" becomes "`list.rs` is
  listed as runtime by decision, with its reason". No dynamic SQL exists.
- `persistence.md:36-39` (roll-back, owner): replaces "target state" with
  what the code does: macros with a committed `.sqlx/` cache in the five
  modules; `list.rs` runtime because its central predicate is shared
  constants that the macros can't reference; how to regenerate the cache.
  `repo/sqlite/mod.rs:4`'s comment follows.

## Recommendation and question

Recommended: Q1 B (all but `list.rs` and `mod.rs`, 51 queries); Q2
`SQLX_OFFLINE=true` in CI and a repository script for regenerating and
checking `.sqlx/`, no `sqlx-cli`; Q3 migrate on sqlx 0.8 now.

Crisp questions:

1. Scope: convert `user`, `category`, `invitation`, `recipe` and
   `product` (51 queries) to `query!`/`query_as!`/`query_scalar!`, keep
   `list.rs` and `mod.rs` on runtime queries by decision (B)? Or all (A),
   or none and retire the target (C)?
2. CI: `SQLX_OFFLINE=true` for every build, plus a staleness check by a
   repository script (`scripts/sqlx-prepare.sh --check`, Python `sqlite3`
   and an online build into a temporary cache), rather than a pinned
   `sqlx-cli`?
3. Timing: migrate on sqlx 0.8 now, and regenerate the cache in whichever
   later PR upgrades sqlx?
4. `persistence.md`: replace the target-state paragraph with the decided
   state (roll-back, applied after the RR is signed)?

## Blast radius and rollback

What can break:
- Builds, if `.sqlx/` is missing or stale. CI catches it, and the error
  names the query and says to regenerate.
- A conversion that changes a query's text. Forbidden by the brief. Each
  converted query keeps its SQL byte for byte, apart from the type
  override aliases, and the repo tests run unchanged.
- Nothing at run time: the same SQL runs; only how it's bound changes.

Who is affected: developers and agents changing queries in the five
modules (one script to run). No user-visible change.

Rollback plan: revert the PR; `.sqlx/` goes with it.

## Minimal context

- MB-0049 P1–P4, `autonomy`.
- `persistence.md:36-39`, `crates/kauppa-core/src/repo/sqlite/mod.rs:4`.
- RR-0002 constraint 1 and RR-0009 constraint 3 (one central list
  predicate, the bypass guard in `repo/share.rs`), which option B leaves
  untouched.
- MB-0041 (the SQLite advisory; sqlx 0.9 doesn't unblock it), MB-0048
  (proposes splitting `list.rs`).
- Files B touches: the five repository files, `.sqlx/` (new),
  `scripts/sqlx-prepare.sh` (new), `.github/workflows/ci.yml`,
  `persistence.md` and `mod.rs`'s comment (roll-back).

## Evidence bundle (Hardened)

Scratch worktree at `origin/main` (`b800583`), not committed; session
scratchpad.

```text
# schema database from the migrations
python3: for f in sorted(migrations/*.sql): sqlite3.executescript(f)
  → tables: users, shopping_lists, list_items, recipes, ingredients,
    categories, products, list_shares, admins, invitations

# baseline
cargo build -p kauppa-core                     (warm deps)  9.33 s first, 0.26 s after an edit

# user.rs converted (4 queries: query!, query_scalar!, query!, query!)
DATABASE_URL=sqlite://…/schema.db SQLX_OFFLINE_DIR=…/.sqlx \
  cargo build -p kauppa-core                   0.77 s; .sqlx/ gets 4 query-<sha256>.json
cargo build -p kauppa-core (no DATABASE_URL)   0.26 s, uses .sqlx/
SQLX_OFFLINE=true cargo build -p kauppa-core   0.26 s
# query text edited (" LIMIT 1"), cache not regenerated
SQLX_OFFLINE=true cargo build → error: `SQLX_OFFLINE=true` but there is no cached
  data for this query, run `cargo sqlx prepare` … --> user.rs:58:13
# schema drift: ALTER TABLE users RENAME COLUMN display_name TO name
DATABASE_URL=…/drift.db cargo build → error returned from database: (code: 1)
  no such column: display_name --> user.rs:17:19
cargo test -q -p kauppa-core                   92 + 3 passed, 0 failed (9.62 s incl. build)

# composing SQL
sqlx::query!("SELECT id FROM users " + "WHERE id = ?1", "x")   compiles
sqlx::query!(frag!(), "x")  (macro_rules! frag)                 error: expected string literal
```

- Query counts: `grep -c "sqlx::query"` per file under
  `crates/kauppa-core/src/repo/sqlite/` (tests excluded).
- Shared constants: `grep -c` of each constant's name in `list.rs`; the
  guard's allow-list at `repo/share.rs:692-700`.
- sqlx 0.9.0: `cargo search sqlx`; its `sqlx-sqlite` requirement read from
  the crates.io index (`libsqlite3-sys >=0.30.1, <0.38.0`,
  `rust-version 1.94.0`).
