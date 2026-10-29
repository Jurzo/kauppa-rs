# Mission Brief — Shopping list store view (grouped, check-off, remove)

```yaml
---
id: MB-0006
owner: "@juuso-kalliomaki"
goal: Render a shopping list grouped by category and ordered within each category, and let a user check items off as found and remove unneeded items while shopping.
non_goals:
  - Building the list / adding items (MB-0004, MB-0005).
  - Category and product management (MB-0001, MB-0002).
  - Multiple named lists / resume and sharing (future briefs).
  - Any schema change — checked, position, category_id, and removed_at already exist.
acceptance:
  - id: P1
    property: The view groups active (non-removed) items by their snapshotted category; items with no category render in a single trailing "Muut" bucket.
    check: cargo test -p kauppa-web routes::lists::view_groups_by_category
  - id: P2
    property: Within a category, items are ordered by the user-defined product position with alphabetical name as a deterministic fallback.
    check: cargo test -p kauppa-core repo::list::view_order_within_category
  - id: P3
    property: Toggling an item as found flips and persists its checked state; the toggle returns an HTML fragment on HX-Request.
    check: cargo test -p kauppa-web routes::lists::toggle_found
  - id: P4
    property: Removing an item from the store view soft-deletes it (sets removed_at) and it no longer appears in the grouped view, while remaining recoverable.
    check: cargo test -p kauppa-web routes::lists::remove_from_view
  - id: P5
    property: Checked and removed state are owner-isolated — a user cannot toggle or remove an item on another user's list.
    check: cargo test -p kauppa-web routes::lists::view_owner_isolation
  - id: P6
    property: GET of the store view returns the full page on a normal request and fragments for toggle/remove on HX-Request; unauthenticated requests are rejected.
    check: cargo test -p kauppa-web routes::lists::view_htmx_and_auth
context:
  - crates/kauppa-core/src/domain/shopping_list.rs
  - crates/kauppa-core/src/repo/mod.rs
  - crates/kauppa-web/src/routes/lists.rs
  - crates/kauppa-web/templates
  - .agentic/missions/MB-0004-assemble-list.md
  - .agentic/missions/MB-0001-categories.md
autonomy:
  may_decide:
    - the grouped read query shape and whether ordering is done in SQL or in the handler
    - the "Muut" bucket label and placement, and the store-view fragment markup
    - whether check-off uses optimistic htmx swap or a server round-trip
  escalate:
    - { when: a schema change turns out to be needed, to: data-owner }
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

This is the view a user takes to the store — the product owner's final section:
the list is organized by category, items within a category follow the
user-defined order (or alphabetical), items can be checked off as found, and
unneeded items (already at home) can be removed from view. It consumes the
item model from MB-0004 and the category positions from MB-0001/MB-0002.

No schema change is needed — `list_items` already has `checked` and `position`
(from `0001`) and `category_id` + `removed_at` (from MB-0004's `0006`), so this
brief carries no Resolution Record. The soft-delete mechanism is MB-0004's; this
brief surfaces it in the store view alongside check-off.

## Conceptual plan

Add a `ShoppingListRepo` read that returns a list's active items grouped and
ordered for display (group by snapshotted `category_id`, uncategorized last;
within a group order by the product's position-in-category with name as a
deterministic tiebreak). Reuse MB-0004's soft-delete for the remove control and
add a check-off toggle that flips `checked`. Render the grouped view in
`templates/lists/` and return htmx fragments for toggle/remove so a shopper on a
slow phone gets minimal round-trips (per the AGENTS.md 3G goal).

Checkpoints: the ordering read (P2) is unit-tested at the repo layer first.
Slow-mode triggers: if grouping seems to need live joins back to a mutable
catalog, re-read MB-0004's snapshot decision — the view groups by the item's
*snapshotted* category, not the product's current one; if ordering needs a new
column, stop and escalate.

## Validation plan

P2 is a `kauppa-core` test over a temp SQLite pool (deterministic in-category
ordering with alphabetical fallback). P1, P3, P4, P5, P6 are `kauppa-web` tests:
grouping incl. the trailing uncategorized bucket, check-off persistence,
remove-hides-from-view, cross-owner rejection, and HX vs. full-page rendering
with the unauthenticated path. Evidence: named test outputs in the scope-to-proof
map.
