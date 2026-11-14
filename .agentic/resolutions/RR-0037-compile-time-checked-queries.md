# Resolution Record — Compile-time-checked SQL in all repositories but the list one, a committed `.sqlx/` cache kept honest by a repository script, on sqlx 0.8

```yaml
---
id: RR-0037
closes: CRP-0037
mission: MB-0049
decided_by: "<resolver's GitHub handle, e.g. @Jurzo>"
role: platform
risk_tier: low
decided_at: <RFC 3339 UTC time of signing, e.g. 2026-11-14T00:00:00Z>
status: <approved | rejected>
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0049-compile-time-checked-queries.md
  - .agentic/consults/CRP-0037-compile-time-checked-queries.md
  - .agentic/mentorship/persistence.md
---
```

> **Scaffold, not a decision.** The owner hasn't answered CRP-0037 yet.
> The sub-answers below are the consult's recommendations, each marked
> *Proposed*. Before signing, the resolver edits any sub-answer answered
> differently (for Q1 C the record becomes "retire the target": sub-answers
> 2 and 3 fall away and the roll-back only edits `persistence.md`), removes
> this note and the *Proposed* marks, and fills `decided_by`, `decided_at`,
> `status` and the Approvals line.

## Decision and outcome

Approve CRP-0037's questions 1–4 as answered by the owner
(`<owner's answer and date, e.g. "all as recommended", 2026-11-14>`).

Sub-answers:

1. **Scope (Q1 B, platform).** *Proposed.* The 51 queries in
   `repo/sqlite/user.rs`, `category.rs`, `invitation.rs`, `recipe.rs` and
   `product.rs` move to `sqlx::query!`, `query_as!` and `query_scalar!`.
   - `list.rs` (28 queries) and `mod.rs` (2 setup queries) stay runtime
     queries **by decision**. `list.rs` builds its queries from shared SQL
     constants, which the macros can't reference (`query!` takes only
     string literals), and its central predicate and bypass guard (RR-0002
     constraint 1, RR-0009 constraint 3) stay exactly as they are.
   - Each converted query keeps its SQL byte for byte, except type-override
     aliases (`created_at AS "created_at: DateTime<Utc>"`). Ids stay
     `String` and go through `parse_uuid` as today. No query's behaviour
     changes.
   - No manifest change: the workspace already enables sqlx's `macros`.
2. **The cache and CI (Q2, platform).** *Proposed.*
   - `.sqlx/` at the workspace root is committed. With no `DATABASE_URL`
     the macros read it, so builds need no database.
   - `ci.yml` sets `SQLX_OFFLINE=true` for every build, so a missing cache
     entry fails CI at the query.
   - A repository script, `scripts/sqlx-prepare.sh`, creates a temporary
     SQLite database from `migrations/*.sql` (Python's `sqlite3`), builds
     `kauppa-core` online against it with `SQLX_OFFLINE_DIR` pointing at a
     temporary folder, and then replaces `.sqlx/` with it, or with
     `--check` fails if the two differ. CI runs `--check`; a developer or
     agent runs it after changing a query or adding a migration.
   - No `sqlx-cli` in CI or as a required tool.
3. **Timing (Q3, platform).** *Proposed.* Convert on sqlx 0.8 now. The
   later PR that upgrades sqlx (its own dependency-bump mission) regenerates
   `.sqlx/` with the script. sqlx 0.9 doesn't unblock MB-0041
   (`sqlx-sqlite` 0.9.0 caps `libsqlite3-sys` below 0.38).
4. **The Mentorship Pack (Q4, owner).** *Proposed.* `persistence.md`'s
   target-state paragraph is replaced by what the code does (Roll-back,
   below); `repo/sqlite/mod.rs:4`'s comment follows in the implementation
   PR.

Covers:

- MB-0049's implementation, P1–P4, as amended below;
- the five repository files, `.sqlx/`, `scripts/sqlx-prepare.sh`, the
  `ci.yml` environment and step;
- the roll-backs below.

Does not cover:

- converting `list.rs` or `mod.rs`, or any change to the list predicate,
  its constants or the bypass guard;
- any change to a query's SQL beyond type-override aliases, to the schema,
  or to a migration;
- a sqlx version change, a new dependency or feature, or `sqlx-cli`;
- building SQL with `format!` from input (the brief forbids it).

## Rationale and trade-offs

- **All but `list.rs` (B).** Most queries gain compile-time checks of
  columns, argument counts and row types (AGENTS.md: "prefer explicit types
  and compile-time checks"). Converting `list.rs` (A) would copy the
  central predicate into 12 literal queries and restate a security
  invariant the guard now enforces by name. Retiring the target (C) costs
  nothing but keeps SQL errors a test-run away.
- **A repository script, not `sqlx-cli`.** The trial showed the macros
  write the cache themselves with `SQLX_OFFLINE_DIR`; a pinned CLI would be
  one more binary in CI's supply chain (RR-0032) for the same result.
- **sqlx 0.8 now.** Waiting for a 0.9 upgrade nobody has scheduled buys
  only a one-time cache regeneration, which the script makes cheap.

## Approvals and authority

`<resolver>` as platform for sub-answers 1–3; owner for 4.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0037-compile-time-checked-queries.md`
- Mission Brief: `.agentic/missions/MB-0049-compile-time-checked-queries.md`
- Related: RR-0002 (central list predicate), RR-0009 (constraint 3, the
  guard), RR-0016 and MB-0041 (the SQLite advisory), RR-0032 (CI supply
  chain), MB-0048 (CRP-0036 proposes splitting `list.rs`; this record
  leaves `list.rs` alone, so the two don't conflict).
- Evidence: CRP-0037 → The trial and Evidence bundle (`user.rs` converted
  on a scratch worktree: offline builds, a stale cache and schema drift
  failing at the query, 95 core tests passing, `query!` rejecting a
  `macro_rules!` fragment).

## Constraints and follow-on rules (Hardened)

1. **A change to a query in the five modules, or a new migration, comes
   with a regenerated `.sqlx/`** (`scripts/sqlx-prepare.sh`); CI's
   `--check` enforces it.
2. **`list.rs` and `mod.rs` stay runtime queries** until a Resolution
   Record says otherwise; a new repository file uses the macros.
3. **No SQL is built at run time** in the repositories, as today.
4. **Every CI build is offline** (`SQLX_OFFLINE=true`); no CI job needs a
   long-lived database.

## Roll-back into the system (Hardened)

To be applied after this record is signed, in the consult's PR:

- `MB-0049`:
  - `resolutions: []` → `[RR-0037]`.
  - P2's check: `scripts/sqlx-prepare.sh --check` (and CI's offline build)
    instead of `cargo sqlx prepare --workspace --check`; "the chosen
    modules" named (the five).
  - P3: "dynamic SQL that can't be a macro is listed with its reason" →
    "`list.rs` and `mod.rs` are listed as runtime by decision (RR-0037),
    with the reason"; there is no dynamic SQL.
  - `escalate`: the decision, timing and `sqlx-cli` items are settled.
- `CRP-0037`: `state: open` → `answered`.
- `.agentic/mentorship/persistence.md:36-39`: the "target state" paragraph
  becomes: the repositories use the query macros with a committed `.sqlx/`
  cache, regenerated by `scripts/sqlx-prepare.sh` and checked in CI with
  `SQLX_OFFLINE=true`; `list.rs` and `mod.rs` use runtime queries by
  decision (RR-0037), because the list predicate is shared SQL constants
  the macros can't reference.

## Supersession and history

Status: active. Superseded by: none.
