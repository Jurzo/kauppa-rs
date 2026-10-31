# Mission Brief — Named lists: save and resume (initial version)

```yaml
---
id: MB-0007
owner: "@juuso-kalliomaki"
goal: Let a user keep several named shopping lists, see them most-recent-first, and reopen one to continue it later.
non_goals:
  - Adding items or building a list (MB-0004, MB-0005).
  - The grouped store view and check-off (MB-0006).
  - Sharing a list (MB-0008).
  - A distinct archived / completed state — deferred (see open questions).
acceptance:
  - id: P1
    property: A user can create multiple named lists; each list is owner-scoped and never visible to another user.
    check: cargo test -p kauppa-core repo::list::create_named_owner_scoped
  - id: P2
    property: The list index returns the user's lists ordered most-recently-updated first.
    check: cargo test -p kauppa-core repo::list::index_ordered_by_updated
  - id: P3
    property: Opening a list by id loads the list so it can be continued; opening a list the user does not own is rejected.
    check: cargo test -p kauppa-web routes::lists::open_resume_owner_isolation
  - id: P4
    property: A list can be renamed and deleted; deleting a list removes its items via the existing ON DELETE CASCADE.
    check: cargo test -p kauppa-core repo::list::rename_and_delete_cascades
  - id: P5
    property: Create / open / rename / delete return HTML fragments on HX-Request and full pages otherwise; unauthenticated requests are rejected.
    check: cargo test -p kauppa-web routes::lists::lifecycle_htmx_and_auth
context:
  - crates/kauppa-core/src/domain/shopping_list.rs
  - crates/kauppa-core/src/repo/mod.rs
  - crates/kauppa-web/src/routes/lists.rs
  - migrations/0001_init.sql
  - .agentic/missions/MB-0004-assemble-list.md
autonomy:
  may_decide:
    - internal shape of the ShoppingListRepo lifecycle methods and the lists route handlers
    - the list-index and rename UI and its fragment markup
    - whether a new list gets a default name or requires one
  escalate:
    - { when: an archived / completed state or any other schema change is adopted, to: data-owner }
    - { when: public route added or removed, to: api-owner }
  forbidden:
    - edit a committed migration (migrations/0001_init.sql and earlier 000N files) — add a new numbered file
    - add dependencies
    - lower lint strictness
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/persistence.md@v1
  - .agentic/mentorship/actix-web.md@v1
  - .agentic/mentorship/templating.md@v1
---
```

## Context

Initial version. The product owner's nice-to-have is "the shopping list can be
saved and continued later." The scaffold already persists `shopping_lists`
(owner, name, created_at, updated_at) with an `(owner_id, updated_at DESC)` index
and `list_items` cascading on delete, and `routes/lists.rs::index` already lists
them — but `routes/lists.rs::create` is a `NotImplemented` TODO. So "save and
resume" is largely a lifecycle-wiring job on an existing schema, needing no new
migration and therefore no Resolution Record.

## Conceptual plan

Wire up `ShoppingListRepo` create / open / rename / delete (owner-checked) and
the matching handlers, replacing the `create` TODO. Reuse the existing index
ordering for the "most recent first" list. Resumption is just reopening a
persisted list — items come from MB-0004's model — so this brief keeps that
read thin and defers item rendering to MB-0004 / MB-0006.

Checkpoints: owner isolation (P1, P3) tested before UI. Slow-mode trigger: if a
clean active-vs-finished separation turns out to be needed, stop and open a
consult rather than adding a column here (see open questions).

## Open questions (initial version)

1. **Sequencing with MB-0004.** Assembling a list (MB-0004) needs a list to
   exist. If MB-0004 lands first with an implicit single/default list, this
   brief generalizes it to many named lists; if this brief lands first, it
   provides creation for MB-0004. Pick an order before implementing.
2. **Finished lists.** "Continue later" may want an explicit
   archived/completed state so finished trips drop out of the active index —
   that is a `shopping_lists` schema change and would need a data-owner consult.
   Deferred out of this initial version.

## Decisions (owner, 2026-10-31)

1. **Sequencing (open question 1): this brief lands first**, and MB-0004 adds
   items on `/lists/{id}` in a PR stacked on it. There is no implicit default
   list.
2. Open question 2 (an archived/completed state) stays deferred.

Implementation notes (within `may_decide`):

- A new list needs a name; names needn't be unique. Renaming counts as an
  update, so it moves the list to the top of the index.
- Routes: `POST /lists`, `GET /lists/{id}`, `POST /lists/{id}/rename`, and
  `POST /lists/{id}/delete`.
- `GET /lists/{id}` returns the `lists/_list` fragment only for a non-boosted
  htmx request. Boosted navigation gets the full page.

## Validation plan

P1, P2, P4 are `kauppa-core` tests over a temp SQLite pool (owner-scoped create,
index ordering, rename + cascading delete). P3 and P5 are `kauppa-web` tests:
cross-owner open rejection and HX vs. full-page rendering with the
unauthenticated path. Evidence: named test outputs in the scope-to-proof map.
