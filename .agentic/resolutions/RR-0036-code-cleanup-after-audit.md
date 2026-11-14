# Resolution Record — Code cleanup: `sqlx` to dev-dependencies, the unused favorites route removed, `list.rs` split into `list_access.rs` under the guard, error bodies stay English

```yaml
---
id: RR-0036
closes: CRP-0036
mission: MB-0048
decided_by: "@Jurzo"
role: security
risk_tier: medium
decided_at: 2026-11-14T09:00:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0048-code-cleanup-after-audit.md
  - .agentic/consults/CRP-0036-code-cleanup-after-audit.md
  - .agentic/resolutions/RR-0009-source-layout-and-test-paths.md
  - .agentic/resolutions/RR-0024-editor-favorite-matching.md
  - .agentic/mentorship/persistence.md
  - .agentic/mentorship/templating.md
  - .agentic/runbooks/feature.md
  - scripts/agentic/check_triggers.py
  - .github/CODEOWNERS
---
```

## Decision and outcome

Approve CRP-0036's questions 1–9 as answered by the owner (@Jurzo, 2026-11-14: "All as recommended").

Sub-answers:

1. **`sqlx` in `kauppa-web` (Q1 A, platform).**
   `sqlx = { workspace = true }` moves from `[dependencies]` to
   `[dev-dependencies]` in `crates/kauppa-web/Cargo.toml`. Same version and
   features, no other manifest change, `Cargo.lock` unchanged. Done under
   `dependency-bump.md`: this RR is approved before the PR opens, and
   `cargo deny check` passes. The compiler then enforces
   `persistence.md`'s "the web layer never imports `sqlx`".
2. **`POST /lists/{id}/favorites` (Q2 A, owner).** Removed: its
   handler, its form type, its registration in `routes/mod.rs` and the test
   cases that post to it. The old URL answers 404. This amends RR-0024
   sub-answer 3 (a note is added there).
3. **Splitting `list.rs` (Q3 A, security).** A second file,
   `crates/kauppa-core/src/repo/sqlite/list_access.rs`, takes
   `ACCESS_SELECT`, `Accessible`, `accessible_lists`, `ListGrant`,
   `authorize`, `begin_read`, `begin_write`, `ITEM_SELECT`, `fetch_items`,
   `fetch_item` and `touch_list`.
   - `list.rs` keeps `STORE_VIEW_SELECT`, `SHARE_SELECT` and the
     `impl ShoppingListRepo` block, unchanged.
   - No SQL text, method body or signature changes. Moved items `list.rs`
     calls become `pub(super)`. `ListGrant`'s fields stay private and are
     read through accessors, so only `authorize` can make a grant.
4. **The guard (Q4, security).** In `repo/share.rs`'s bypass
   guard, exactly:
   - one constant added, `LIST_ACCESS_RS = include_str!("sqlite/list_access.rs")`;
   - rules 1 and 2 untouched;
   - rule 3 runs over both files with the same seven allowed headers, a
     leading `pub(super) ` dropped before comparing, and the `&ListGrant`
     requirement unchanged;
   - rule 4 skips `list.rs` and `list_access.rs`, and scans every other
     file in `repo/sqlite/`, present or future, for the same four names.
   - Nothing is removed and no rule is loosened. If `share.rs` would pass
     800 lines, the implementation stops and asks.
   - RR-0009 constraint 3 and sub-answer 3 change from "one file" to these
     two named files (a note is added to RR-0009).
5. **Trigger paths (Q5, security).** `list_access.rs` joins
   the "Shared-list authorization" row in `.agentic/runbooks/feature.md`,
   `scripts/agentic/check_triggers.py` and `.github/CODEOWNERS`
   (`test_check_triggers.sh` keeps the three in step), and
   `persistence.md`'s list-tables paragraph names both files.
6. **Leftovers (Q6, Q4 A, owner).** Removed:
   `InvitationState::as_str`; `PasswordLengthError`'s `Display` and `Error`
   impls with the one unit test of their text (RR-0022 sub-answer 4 still
   holds); `Entity::Favorite` and its four keys in each catalog;
   `Invalid::ChooseFavorite` and `errors.choose_favorite` (follows from
   sub-answer 2). The other 11 unreachable entity keys stay, because the
   types allow those pairs (RR-0023 sub-answer 6).
7. **Plain error bodies (Q7, Q5 A, owner).** They stay English
   in every language. No change to `error.rs` or `csrf/`. The reason goes
   into `templating.md` → Translations, after "Messages built in Rust", as
   CRP-0036 Q5 A words it, citing RR-0023 sub-answer 10 and this record.
8. **An error-page brief (Q8, owner).** A separate brief for a
   translated HTML error page (layout, navigation, sign-in on 401), after
   MB-0055. *Not part of MB-0048 either way.*
9. **Order and the brief (Q9, owner).** *Proposed.* MB-0048's
   implementation starts from `main` after MB-0055 (PR #88) merges. The
   brief takes CRP-0036 Q7's amendments (Roll-back, below).

Covers:

- MB-0048's implementation, P1–P6, one commit per item;
- the `Cargo.toml` line move, the route removal, the split and the guard
  changes exactly as listed, the removals in sub-answer 6;
- moving the navigation-panel test code from `templates/tests.rs` to
  `templates/tests/navigation.rs` with one-line wrappers (within the
  brief's `may_decide`; no check path changes);
- the roll-backs below.

Does not cover:

- any change to list SQL, a method body or signature in the `impl`, or the
  guard's rules 1 and 2;
- a version change of any dependency, or `cargo update`;
- narrowing the error types (CRP-0036 Q4 B);
- translating error bodies, an error-page middleware, or any change to
  `error.rs`, `csrf/` or `main.rs`;
- a status code change.

## Rationale and trade-offs

- **Move, don't remove, `sqlx`.** The compiler enforces the layering rule
  for free; removing it would widen core's public surface for tests only.
- **Remove the route.** Nothing posts to it since MB-0036; a URL nobody
  uses is surface the CSRF and authorization tests must keep covering.
- **`list_access.rs` (A) over a directory (B) or splitting the `impl`
  (C).** A keeps the guard's rules 1 and 2 as they are and makes the grant
  compiler-enforced. B would make the guard read a whole directory; C would
  make rule 1 pass on nothing. A-minimal leaves 75 lines of room.
- **Whole unused variants only.** Narrowing the types is a refactor beyond
  the brief.
- **English error bodies, written down.** Translating needs a
  request-aware middleware in trigger paths (`main.rs`, `csrf/`,
  `i18n/request.rs`); the visible gap is the bare page, not the wording.

## Approvals and authority

@Jurzo as platform for sub-answer 1; owner for 2, 6, 7, 8 and 9; security for 3, 4 and 5.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0036-code-cleanup-after-audit.md`
- Mission Brief: `.agentic/missions/MB-0048-code-cleanup-after-audit.md`
- Related: RR-0002 (central list predicate), RR-0009 (constraint 3,
  amended), RR-0022 (sub-answer 4), RR-0023 (sub-answers 6 and 10), RR-0024
  (sub-answer 3, amended), RR-0035 (MB-0055, which merges first).
- Evidence: CRP-0036 → Evidence bundle (the audit re-checked on `main`
  `b800583`; the `sqlx` move built, linted and `cargo deny`-checked on a
  scratch copy; line counts).

## Constraints and follow-on rules (Hardened)

1. **The list tables are named in exactly two files**, `list.rs` and
   `list_access.rs` (replaces RR-0009 constraint 3's one file). Any other
   file in `repo/sqlite/` naming them fails the guard.
2. **Only `authorize` makes a `ListGrant`.** Its fields stay private to
   `list_access.rs`.
3. **The guard is only ever widened by a Resolution Record**, as here: one
   constant, one more file under rule 3, one more name skipped by rule 4.
4. **`kauppa-web` doesn't depend on `sqlx` outside tests.**
5. **Plain error bodies are English by decision**, until a brief for a
   translated error page says otherwise.

## Roll-back into the system (Hardened)

To be applied after this record is signed, in the consult's PR (the
`Cargo.toml` change itself waits for the implementation PR):

- `MB-0048`: CRP-0036 Q7's amendments: Context to today's facts; the
  `context` list extended; P1's check becomes
  `cargo test -p kauppa-web state::tests::sqlx_is_a_dev_dependency`; P2
  and P5 reworded to the decisions (P5 adds `csrf::tests::cross_site`); P3
  names both files over 800 and `list_access.rs`; P4 reworded to
  sub-answer 6 with the check
  `cargo test -p kauppa-web i18n::tests::catalog_has_no_unused_keys`;
  `escalate` settled; `resolutions: [RR-0036]`.
- `CRP-0036`: `state: open` → `answered`.
- `RR-0009`: an amendment note under constraint 3 and sub-answer 3: "two
  files, `list.rs` and `list_access.rs` (RR-0036)".
- `RR-0024`: an amendment note under sub-answer 3 ("id-based add stays owner-only"): the route is removed
  (RR-0036 sub-answer 2).
- `.agentic/mentorship/persistence.md`: the list-tables paragraph names
  both files.
- `.agentic/mentorship/templating.md`: the English-error-bodies paragraph
  (sub-answer 7).
- `.agentic/runbooks/feature.md`, `scripts/agentic/check_triggers.py`,
  `.github/CODEOWNERS`: `list_access.rs` in the "Shared-list
  authorization" row (sub-answer 5).

## Supersession and history

Status: active. Superseded by: none.
