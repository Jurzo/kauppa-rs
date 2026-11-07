# Mission Brief — Split the largest source files into manageable modules

```yaml
---
id: MB-0019
owner: "@juuso-kalliomaki"
goal: Break the six source files over 800 lines (up to 3,300) into modules a person or agent can read in one sitting, as a pure move — no behavior, SQL or route change — so the next features land in small files and stop conflicting with each other.
non_goals:
  - Any behavior change, bug fix or new feature found along the way (file a separate brief).
  - Rewriting SQL, renaming public repo trait methods, or changing route paths.
  - Splitting files under 800 lines, templates, or CSS.
  - Enforcing a file-size limit in CI (a possible follow-up).
acceptance:
  - id: P1
    property: "The move preserves behavior: every test that existed before still exists under its new path and passes, and the per-target test counts are unchanged (baseline on 729f3c1: kauppa-core lib 56, password_compat 3, kauppa-web 37)."
    check: cargo test --workspace --all-targets
  - id: P2
    property: No `.rs` file under `crates/` exceeds 800 lines.
    check: "find crates -name '*.rs' | xargs wc -l | awk '$1 > 800 && $2 != \"total\"' — prints nothing"
  - id: P3
    property: "The web helpers duplicated across route modules (`current_user`, `is_hx`, `wants_fragment`) and the duplicated test fixtures (`make_recipe`, `make_list`) are each defined once."
    check: "grep -rnE '^(pub )?(async )?fn (current_user|is_hx|wants_fragment|make_recipe|make_list)\\b' crates/kauppa-web/src — one line per name"
  - id: P4
    property: Every `cargo test` check in every Mission Brief selects at least one test after the move (the briefs' check paths are updated to the RR-0009 test layout), and a script guards this from now on.
    check: scripts/agentic/check_brief_tests.sh
  - id: P5
    property: "The sharing bypass guard still enforces RR-0002 constraint 1 on the split repo: it reads the file that holds the list repo, fails if any other repo file mentions `shopping_lists`, `list_items` or `list_shares`, and still catches the two hand-made bypasses from MB-0008."
    check: cargo test -p kauppa-core repo::share::list_queries_route_through_central_predicate
context:
  - crates/kauppa-core/src/repo/sqlite.rs
  - crates/kauppa-core/src/repo/mod.rs
  - crates/kauppa-core/src/repo/list.rs
  - crates/kauppa-core/src/repo/product.rs
  - crates/kauppa-core/src/repo/share.rs
  - crates/kauppa-web/src/routes/mod.rs
  - crates/kauppa-web/src/routes/lists.rs
  - crates/kauppa-web/src/routes/recipes.rs
  - crates/kauppa-web/src/routes/products.rs
  - crates/kauppa-web/src/routes/test_support.rs
  - .agentic/runbooks/feature.md
  - .agentic/mentorship/actix-web.md
  - .agentic/consults/CRP-0009-source-layout-and-test-paths.md
autonomy:
  may_decide:
    - the module boundaries and file names inside the layout RR-0009 approves
    - where shared private helpers live (`pub(super)` vs a shared module)
    - the order of moves within a PR, as long as each commit builds and passes
  escalate:
    - { when: "a move seems to need a behavior, SQL or signature change", to: owner }
    - { when: "the bypass guard needs more than re-pointing it at the split files", to: security }
    - { when: "a test must change its assertions (not just its location) to survive the move", to: owner }
    - { when: "the test layout or check-path rewrite turns out different from RR-0009", to: owner }
  forbidden:
    - change behavior, SQL text, route paths or public trait signatures
    - delete, weaken or skip any test
    - add dependencies or lower lint strictness
    - edit AGENTS.md, Mentorship Packs, Runbooks or Resolution Records without an approved RR
    - start while feature PRs are open (the move conflicts with everything)
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/actix-web.md@v1
  - .agentic/mentorship/persistence.md@v1
  - .agentic/mentorship/security.md@v1
resolutions:
  - RR-0009
---
```

## Context

After MB-0003 through MB-0015 and MB-0008, most code sits in six files (line
counts on `main` at 729f3c1):

| File | Lines | Of which tests |
| --- | --- | --- |
| `crates/kauppa-web/src/routes/lists.rs` | 3,323 | ~2,230 |
| `crates/kauppa-web/src/routes/recipes.rs` | 2,356 | ~1,410 |
| `crates/kauppa-core/src/repo/sqlite.rs` | 1,986 | none (all five repo impls) |
| `crates/kauppa-web/src/routes/products.rs` | 1,381 | ~940 |
| `crates/kauppa-core/src/repo/list.rs` | 1,042 | all (test-only module) |
| `crates/kauppa-core/src/repo/product.rs` | 804 | all (test-only module) |

Two costs show up already:

- **Readability.** AGENTS.md goal 4 is "navigate the whole system in under an
  hour", and a 3,300-line file defeats that.
- **Merge conflicts.** The four parallel missions on 2026-11-06 conflicted
  mostly in `recipes.rs`, `show.hbs` and `sqlite.rs`, and needed hand merges.
  One resolution produced a duplicated helper and a duplicated test.

Tests sit at module level because every brief's check names an exact path,
such as `routes::lists::share_audit_logging`. There are 81 `cargo test`
checks across the briefs. Moving tests into submodules changes those paths.
The owner chose to adopt a test layout and update the checks (CRP-0009
Option A, 2026-11-07).

Two governance texts name files that will move:

- the feature runbook's security trigger row, "Shared-list authorization in
  `crates/kauppa-web/src/routes/lists.rs`";
- MB-0008's bypass guard, which `include_str!`s `repo/sqlite.rs`.

Both changes go through RR-0009.

## Conceptual plan

Wait until no feature PRs are open. Then two PRs, one per crate. Each commit
builds and passes the gate, so a reviewer can follow the moves.

1. **kauppa-core:**
   - `repo/sqlite.rs` → `repo/sqlite/`:
     - `mod.rs`: `SqlitePool`, shared row parsers and name helpers;
     - one file per `impl … for SqlitePool`: `user.rs`, `category.rs`,
       `product.rs`, `list.rs`, `recipe.rs`. The list file holds the access
       predicate and all list SQL.
   - The test-only modules `repo/list.rs` and `repo/product.rs` split by area,
     per RR-0009's layout.
   - Re-point the bypass guard (P5).
   - Add `scripts/agentic/check_brief_tests.sh` (P4) and update the core
     check paths in the briefs.
2. **kauppa-web:**
   - shared helpers into one module (P3);
   - `routes/lists.rs`, `routes/recipes.rs` and `routes/products.rs` become
     directories, with handlers grouped by area (e.g. lists: index/CRUD,
     items, shop, sharing, recipes) and re-exported, so `routes/mod.rs`
     registers the same services;
   - their tests move to `tests/<area>.rs`;
   - fixtures go to `test_support`;
   - update the web check paths in the briefs.
3. **Roll-back** (RR-0009):
   - the feature runbook's trigger row;
   - the test-layout convention in `actix-web.md`;
   - a note in the Mission Brief template on how checks name tests.

Checkpoints: per-target test counts compared with the baseline after every
commit (P1). `check_brief_tests.sh` stays green after each check-path
update.

Slow-mode triggers:
- a test count drops;
- a check selects 0 tests;
- `git diff -M` stops reporting a moved block as a rename/move (a sign of
  accidental edits).

## Validation plan

- **P1:** the gate on each commit, plus test counts recorded before and
  after in the Merge-Readiness Pack. Also a curl smoke of every page against
  the built binary, to catch template or route wiring slips that strict mode
  only shows at runtime.
- **P2:** the size command, with its empty output in the MRP.
- **P3:** the grep, one hit per name.
- **P4:** the script runs every `cargo test` check from
  `.agentic/missions/MB-*.md` with `-- --list`, and fails on any that selects
  0 tests. Its output goes in the MRP.
- **P5:**
  - the guard passes on the split code;
  - re-run the two bypass experiments from MB-0008 against the new layout (a
    query before `authorize` in `get`; a `list_items` statement in the
    product repo file), and confirm each fails the guard. Record this in the
    MRP.
