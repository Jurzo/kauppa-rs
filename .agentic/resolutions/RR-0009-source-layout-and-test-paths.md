# Resolution Record — Source layout, test paths and the bypass guard for MB-0019

```yaml
---
id: RR-0009
closes: CRP-0009
mission: MB-0019
decided_by: "@juuso.kalliomaki"
role: platform
risk_tier: medium
decided_at: 2026-11-07T9:32:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/runbooks/feature.md
  - .agentic/mentorship/actix-web.md
  - .agentic/mentorship/persistence.md
  - .agentic/missions/TEMPLATE.md
  - .agentic/missions/MB-0019-split-source-files.md
  - .agentic/consults/CRP-0009-source-layout-and-test-paths.md
---
```

## Decision and outcome

Approve CRP-0009 **Option A**, with these sub-answers:

1. **Test layout.**
   - In kauppa-web, a split module's tests live in `<module>/tests/<area>.rs`,
     test path `routes::<module>::tests::<area>::<test>`.
   - In kauppa-core, a test-only module under `repo/` that exceeds the size
     guideline becomes a directory, test path
     `repo::<module>::<area>::<test>`. Modules under the guideline keep their
     paths.
2. **Closed briefs are rewritten in place.**
   - Every `cargo test` check whose test moves gets its new full path, in the
     same PR as the move.
   - Each rewritten brief gets the note "Check paths updated by MB-0019
     (RR-0009)."
   - `scripts/agentic/check_brief_tests.sh` proves that every check selects
     at least one test.
3. **Bypass guard re-pointed (security).**
   - `repo::share::list_queries_route_through_central_predicate` reads
     `repo/sqlite/list.rs` and `repo/mod.rs`.
   - It gains one rule: no other file under `repo/sqlite/` may mention
     `shopping_lists`, `list_items` or `list_shares`.
   - Its existing rules (authorize first; only allowed helpers touch list
     tables; every trait method checked) are unchanged.
4. **Size guideline.** About 800 lines per `.rs` file, written into the
   Mentorship Packs. It is not enforced in CI.

Covers:

- MB-0019's two PRs (kauppa-core, then kauppa-web);
- the check-path rewrites in all briefs;
- the new script;
- the roll-back edits listed below.

Does not cover:

- any behavior, SQL, route or public-signature change;
- deleting or weakening tests;
- CI enforcement of the size guideline or of `check_brief_tests.sh` (a
  follow-up brief, if wanted);
- splitting templates or CSS.

## Rationale and trade-offs

- **A — chosen by the owner.**
  - Idiomatic Rust: tests sit next to the code they cover, grouped by
    feature.
  - Parallel missions get small, separate files to change. The four
    missions of 2026-11-06 conflicted mostly in three large files.
- **Rewrite in place.** It keeps every brief runnable as written. A mapping
  table would leave closed briefs pointing at paths that select nothing.
  Git keeps the originals, and the note marks the edit.
- **Guard rule added.** Splitting the repo creates files the guard didn't
  read. Without the new rule, list SQL placed in, say, `sqlite/product.rs`
  would escape it. Re-running MB-0008's two bypass experiments shows the
  guard doesn't pass vacuously.
- **B (`include!`) and C (production-only split) — not chosen.**
  - B keeps paths at the cost of unusual code and weaker editor support.
  - C leaves about 2,300 lines of tests in `routes/lists.rs`.

## Approvals and authority

Approved by platform-owner @juuso.kalliomaki; the bypass-guard change
(sub-answer 3) approved by security-owner @juuso.kalliomaki

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0009-source-layout-and-test-paths.md`
- Mission Brief: `.agentic/missions/MB-0019-split-source-files.md`
- Evidence links:
  - CRP-0009 → Current state: line counts and the distribution of the 81
    check paths on 729f3c1;
  - the baseline test counts on 729f3c1 (core 56, password 3, web 37);
  - the MB-0013 Continuity Pack, for the 2026-11-06 merge resolutions.

## Constraints and follow-on rules (Hardened)

1. **Pure move.** MB-0019 changes no behavior, SQL text, route path or
   public trait signature. Per-target test counts match the baseline, and no
   test's assertions change.
2. **Every brief check selects a test.** `check_brief_tests.sh` passes in
   MB-0019's PRs. From then on it is part of the Merge-Readiness evidence of
   any PR that moves or renames tests.
3. **List SQL lives in one file.** `repo/sqlite/list.rs` is the only
   repository file that may name the list tables, and the bypass guard
   enforces it (this extends RR-0002 constraint 1 to the split layout). A
   change to the guard itself is a security escalation.
4. **New code follows the layout.**
   - New route tests go in `<module>/tests/<area>.rs`.
   - New brief checks name the full test path.
   - New files aim to stay under about 800 lines.

## Roll-back into the system (Hardened)

To be applied in MB-0019's PRs (the first PR carries the doc edits):

- `.agentic/runbooks/feature.md` → Escalation triggers: the row
  "Shared-list authorization in `crates/kauppa-web/src/routes/lists.rs`"
  becomes "Shared-list authorization in `crates/kauppa-web/src/routes/lists/`
  and `crates/kauppa-core/src/repo/sqlite/list.rs`".
- `.agentic/mentorship/actix-web.md` → a new **Tests** section:
  - a module's tests live in `<module>/tests/<area>.rs`;
  - brief checks name the full test path, and
    `scripts/agentic/check_brief_tests.sh` must pass;
  - source files stay under about 800 lines; split by feature area when they
    grow past it (RR-0009).
- `.agentic/mentorship/persistence.md`:
  - one `repo/sqlite/<area>.rs` per repo trait implementation;
  - list tables are named only in `repo/sqlite/list.rs` (RR-0009,
    constraint 3).
- `.agentic/missions/TEMPLATE.md` → under `acceptance`: checks name the full
  test path (see `actix-web.md` → Tests).
- `MB-0019`: `resolutions: [RR-0009]`.
- `CRP-0009`: `state: open` → `answered`.

## Supersession and history

Status: active. Supersedes nothing. Extends RR-0002 constraint 1's
enforcement to the split repository layout; doesn't change the constraint.
