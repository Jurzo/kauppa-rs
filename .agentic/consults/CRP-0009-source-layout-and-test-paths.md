# Consultation Request Pack — Source layout, test paths and the bypass guard for MB-0019

```yaml
---
id: CRP-0009
mission: MB-0019
owner: "@juuso-kalliomaki"
target_role: platform
risk_tier: medium
urgency: low
state: answered
runbook_trigger: "MB-0019 autonomy.escalate (test layout and check-path rewrite; bypass guard change → security); feature.md escalation trigger `.agentic/{mentorship,runbooks,resolutions}/**` (the trigger-path row and the test-layout convention change); AGENTS.md → Agent role (Mentorship Pack and Runbook changes need explicit approval)"
---
```

## Decision statement

Approve the test layout for split modules (Option A, chosen by the owner on
2026-11-07). This covers:

- rewriting the briefs' `cargo test` check paths to match the layout;
- re-pointing MB-0008's sharing bypass guard at the split repo files;
- the runbook and Mentorship Pack updates that follow.

## Current state

- **Tests sit at module level.** Every brief's check names an exact test
  path, and `cargo test <filter>` matches by substring. A test at
  `routes::lists::tests::sharing::share_audit_logging` no longer matches
  `routes::lists::share_audit_logging`.
- **81 `cargo test` checks across the briefs,** by module:

  | Module | Checks |
  | --- | --- |
  | `routes::lists` | 18 |
  | `repo::list` | 12 |
  | `routes::recipes` | 11 |
  | `repo::product` | 8 |
  | `repo::recipe` | 6 |
  | `repo::category` | 6 |
  | `routes::products` | 5 |
  | `repo::share` | 2 |
  | `repo::name_key` | 2 |
  | `templates` | 1 |
  | `routes::categories` | 1 |

  The rest are crate-level checks.
- **The bypass guard reads one file.**
  `repo::share::list_queries_route_through_central_predicate` (MB-0008,
  RR-0002 constraint 1) uses `include_str!("sqlite.rs")` and
  `include_str!("mod.rs")`. It checks that every list method authorizes first,
  and that only allowed helpers mention the list tables.
- **The feature runbook names `routes/lists.rs`.** Its security trigger
  table has the row "Shared-list authorization in
  `crates/kauppa-web/src/routes/lists.rs`".
- **No test-placement convention is written down.** The briefs imply it
  through their check paths; no Mentorship Pack or Runbook says it.

## Options and trade-offs

### Option A — tests in `<module>/tests/<area>.rs`; rewrite the check paths (owner's choice)

**Layout:**

- **kauppa-web:**
  - `routes/lists.rs` → `routes/lists/mod.rs`, with handler files such as
    `items.rs`, `shop.rs`, `sharing.rs` and `recipes.rs`;
  - tests in `routes/lists/tests/<area>.rs`, declared from `lists/mod.rs` as
    `#[cfg(test)] mod tests;`;
  - test path: `routes::lists::tests::<area>::<test>`;
  - the same for `recipes` and `products`.
- **kauppa-core:**
  - the test-only modules under `repo/` keep their names;
  - one over the size budget becomes a directory (`repo/list/<area>.rs`),
    with test path `repo::list::<area>::<test>`;
  - modules under budget (`repo::recipe`, `repo::category`, `repo::share`,
    …) keep their paths unchanged.

**Check rewrite:** each check whose test moves gets its new full path, in
the same PR as the move. A new `scripts/agentic/check_brief_tests.sh`
proves every check still selects at least one test.

- Pros:
  - idiomatic Rust, and editors and rust-analyzer handle it fully;
  - test files sit next to the code they test, grouped by feature;
  - smaller conflict surfaces for parallel missions;
  - the script stops check drift for good.
- Cons:
  - edits `check:` lines in briefs whose missions are closed. Those are
    historical records, though git keeps the originals;
  - test paths get one or two segments longer.
- Risk: low; a stale path is caught by the script.

### Option B — keep test paths via `include!`

The test code moves to separate files, which are textually included at
module level.

- Pros: no brief edits.
- Cons: unusual Rust; weaker editor support; module-level items from several
  files share one namespace.
- Not chosen.

### Option C — split production code only

- Pros: no test or brief changes.
- Cons: `routes/lists.rs` keeps about 2,300 lines of tests; most of the gain
  is lost.
- Not chosen.

## Sub-questions for the resolver

1. **Closed briefs: rewrite in place, or keep a mapping?**
   - Rewriting in place keeps every brief runnable. Each rewritten brief gets
     a one-line note: "Check paths updated by MB-0019 (RR-0009)".
   - A mapping table in RR-0009 would leave closed briefs untouched but not
     runnable as written.
   - **Recommended: rewrite in place.**
2. **The bypass guard (security).**
   - Re-point it at `repo/sqlite/list.rs` (and `repo/mod.rs`).
   - Add one rule: no other file under `repo/sqlite/` may mention
     `shopping_lists`, `list_items` or `list_shares`.
   - The allowed-helper list and the "authorize first" rule stay as they are.
   - The two hand-made bypasses from MB-0008 are re-run as evidence.
   - **Recommended: approve as described.**
3. **Size budget.** Should 800 lines become a written guideline
   (`actix-web.md`, plus a line in `persistence.md` for the repo), or only be
   MB-0019's one-time target? **Recommended: a written guideline, not
   CI-enforced** (CI enforcement would be a follow-up brief).

## Recommendation and question

Recommended: **Option A** with sub-answers 1 (rewrite in place), 2 (approve
the guard change) and 3 (a written guideline).

Crisp question to answer: **Approve Option A, with closed briefs' check paths
rewritten in place, the bypass guard re-pointed with the "no other repo file
names a list table" rule, and an 800-line guideline written into the
Mentorship Packs?**

The roll-back for the RR to carry:

- `.agentic/runbooks/feature.md` → Escalation triggers: the row becomes
  "Shared-list authorization in `crates/kauppa-web/src/routes/lists/` and
  `crates/kauppa-core/src/repo/sqlite/list.rs`".
- `.agentic/mentorship/actix-web.md` → a Tests section:
  - a module's tests live in `<module>/tests/<area>.rs`;
  - brief checks name the full test path;
  - `scripts/agentic/check_brief_tests.sh` must pass;
  - files stay under about 800 lines.
- `.agentic/mentorship/persistence.md` → one line: one `repo/sqlite/<area>.rs`
  per repo trait implementation.
- `.agentic/missions/TEMPLATE.md` → a note under `acceptance`: checks name
  the full test path (see `actix-web.md` → Tests).
- MB-0019 `resolutions: [RR-0009]`; CRP-0009 → `answered`.

## Blast radius and rollback

- **What can break:**
  - a moved test silently not selected (P1 counts and the P4 script catch
    it);
  - a wiring slip in `routes/mod.rs` (route tests and a curl smoke catch it);
  - the bypass guard reading the wrong file. P5 and the re-run bypass
    experiments catch it: a guard that reads nothing would pass vacuously.
- **Who is affected:** developers and agents. Users aren't, since there is
  no behavior change.
- **Rollback plan:** revert the MB-0019 PRs, which are moves plus doc edits.

## Minimal context

- `.agentic/missions/MB-0019-split-source-files.md`
- `crates/kauppa-core/src/repo/share.rs` (the bypass guard, around
  `SQLITE_RS` / `MOD_RS`)
- `.agentic/runbooks/feature.md` → Escalation triggers
- `.agentic/resolutions/RR-0002-list-sharing-authz.md` → constraint 1

## Evidence bundle

- Line counts and the check-path distribution: measured on `main` at
  729f3c1 (2026-11-07). See MB-0019 → Context and Current state above.
- Baseline test counts on 729f3c1: kauppa-core lib 56, password_compat 3,
  kauppa-web 37.
- Merge history of 2026-11-06: the #23/#25 merges into `recipes.rs` needed
  hand resolution, including a duplicated `is_fragment`/`wants_fragment`
  helper and a duplicated `delete_recipe` test (MB-0013 Continuity Pack).
