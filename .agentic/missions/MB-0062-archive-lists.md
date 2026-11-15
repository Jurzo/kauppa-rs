# Mission Brief — Archive lists; archived lists in a collapsed section

```yaml
---
id: MB-0062
owner: "@juuso-kalliomaki"
goal: "A list owner can archive a list they no longer use and restore it later; archived lists leave the main lists page and appear under a separate collapsible 'Archived' section, collapsed by default."
non_goals:
  - "Deleting lists (stays as today), or archiving automatically."
  - "Archiving recipes, favorites or categories."
  - "Changing who can see a list: sharing (RR-0002) applies to archived lists as the RR decides, not differently per grantee."
  - "JavaScript: the section is a native `<details>`."
acceptance:
  - id: P1
    property: "On `/lists`, the owner can archive any of their lists; it then leaves the main list and appears in an 'Archived' section below it, a `<details>` closed on a full page load, showing its name and when it was archived; the section is absent when nothing is archived."
    check: cargo test -p kauppa-web routes::lists::tests::archive::archive_moves_to_section
  - id: P2
    property: "The owner can restore (unarchive) an archived list; it returns to the main list in its usual order."
    check: cargo test -p kauppa-web routes::lists::tests::archive::restore
  - id: P3
    property: "An archived list's page and shopping view open read-only with a notice ('Archived on …') and, for the owner, Restore; item and recipe changes (add, edit, check, remove, restore an item, add a recipe) are refused with 409 and the translated error page, and their controls aren't rendered; the owner can still rename it, delete it and manage its sharing (RR-0045)."
    check: cargo test -p kauppa-web routes::lists::tests::archive::archived_list_pages
  - id: P4
    property: "An archived shared list leaves the grantee's 'Shared with you' and appears, read-only, in a collapsed 'Archived' section on the grantee's `/lists`; shares are unchanged; only the owner can archive or restore, and the archive routes refuse grantees and strangers as the list's other owner actions do (RR-0045)."
    check: cargo test -p kauppa-web routes::lists::tests::archive::owner_only
  - id: P5
    property: "After archive or restore with htmx, the `#lists` fragment swaps and the Archived section is open if the action happened inside it; without htmx, 303 back to `/lists`; the cross-site check covers the new routes."
    check: cargo test -p kauppa-web routes::lists::tests::archive::htmx_and_plain
  - id: P6
    property: "Migration `0011_list_archive.sql` adds `shopping_lists.archived_at TEXT` (NULL = active; existing lists stay active; archiving and restoring don't change `updated_at`); the access predicate returns `archived_at`, `begin_write` refuses item and recipe writes on an archived list with a new `Conflict` kind, and every list write states whether it is allowed when archived (RR-0045)."
    check: cargo test -p kauppa-core repo::sqlite::tests
  - id: P7
    property: "Every string in both catalogs; no JavaScript or inline styles; existing tests change only where they asserted the lists page's markup."
    check: cargo test -p kauppa-web --all-targets
  - id: P8
    property: "In a browser on a phone width and desktop, in both themes: archive, find it under Archived, open it, restore it; keyboard and screen reader can open the section."
    check: "manual: browser check on the release binary, recorded in MB-0062's Continuity Pack"
context:
  - templates/lists/index.hbs
  - templates/lists/_index.hbs
  - templates/lists/show.hbs
  - templates/lists/shop.hbs
  - crates/kauppa-web/src/routes/lists/mod.rs
  - crates/kauppa-web/src/routes/lists/tests/
  - crates/kauppa-core/src/repo/sqlite/list.rs
  - crates/kauppa-core/src/repo/sqlite/list_access.rs
  - crates/kauppa-core/src/repo/sqlite/list_rows.rs
  - migrations/0001_init.sql
  - assets/app.css
  - .agentic/resolutions/RR-0002-list-sharing-authz.md
  - .agentic/resolutions/RR-0036-code-cleanup-after-audit.md
  - .agentic/mentorship/security.md
  - .agentic/mentorship/persistence.md
autonomy:
  may_decide:
    - "the routes' paths under `/lists/{id}/` and test names inside `routes::lists::tests::archive` (record renames here)"
    - "markup and styles, inside the design system"
  escalate:
    - { when: "the schema: `shopping_lists.archived_at TEXT NULL` (recommended) or a flag; whether archiving bumps `updated_at` (it orders the lists page); migrations/ is a trigger path (decided: RR-0045)", to: data-owner }
    - { when: "an archived list's pages: read-only with Restore, or fully usable; whether its shopping view stays reachable (decided: RR-0045)", to: owner }
    - { when: "grantees: hide archived shared lists, or show them in the grantee's own Archived section; whether archiving notifies or changes a share (decided: RR-0045)", to: owner }
    - { when: "the access predicate's handling of the archived state for grantees and the owner-only archive routes (routes/lists/ and repo/sqlite/list.rs are trigger paths, RR-0027) (decided: RR-0045)", to: security }
    - { when: "order inside the Archived section (most recently archived first is assumed) (decided: RR-0045)", to: owner }
  forbidden:
    - "a query on the list tables outside the central access predicate"
    - "edit a committed migration"
    - "let a grantee archive or restore a list"
    - "JavaScript or inline scripts/styles"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
  - .agentic/mentorship/security.md@v1
  - .agentic/mentorship/persistence.md@v1
resolutions: [RR-0045]
---
```

## Context

Requested by the owner (2026-11-15): "Lists can be *archived*. An
*archived* list shows under a separate collapsible list that is collapsed by
default."

Today `/lists` (`templates/lists/_index.hbs`) shows the user's lists ordered
by `updated_at` with Edit (rename, delete), then "Shared with you". A list
is either present or deleted; finished lists can only be kept by leaving
them in the main list or lost by deleting them.

Schema: `shopping_lists (id, owner_id, name, created_at, updated_at)`
(migration 0001); sharing in `list_shares` (0007). An archived state needs a
new migration (schema RR). Every list query goes through the access
predicate in `list_access.rs` (RR-0036), and the list routes and repo are
trigger paths (RR-0027), so a security RR is needed too.

The collapsed-by-default section is the same pattern as MB-0059's
favorites groups (native `<details>`, server opens it after an action inside
it).

Related: MB-0057 (live updates) should treat archive and restore as list
changes; whichever lands second covers it.

## Conceptual plan

1. **Clarify (S1).** One CRP: schema (data); archived list pages, grantees,
   order (owner); predicate and routes (security). Done: CRP-0045 answered,
   RR-0045 approved (all as recommended).
2. Migration and repo with core tests (P6).
3. Routes and templates; route tests (P1–P5).
4. Browser check (P8).

## Validation plan

- Core tests for the archived filter through the predicate.
- Route tests for owner, grantees and strangers, with and without htmx.
- The owner's browser check.
