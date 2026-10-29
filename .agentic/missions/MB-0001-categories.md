# Mission Brief — Per-user product categories

```yaml
---
id: MB-0001
owner: "@juuso-kalliomaki"
goal: Let a user create, rename, reorder, and delete per-user product categories that favorites, recipe ingredients, and shopping-list items are grouped by.
non_goals:
  - Assigning a category to any product or list item (MB-0002, MB-0003, MB-0004).
  - Per-list categories — categories are owner-scoped only, per RR-0001.
  - Rendering or grouping a shopping list by category (future list-view brief).
acceptance:
  - id: P1
    property: Categories are owner-scoped — a repo query for one owner never returns another owner's categories.
    check: cargo test -p kauppa-core repo::category::owner_isolation
  - id: P2
    property: list_for_owner returns categories ordered by position ascending, with name as a stable tiebreak.
    check: cargo test -p kauppa-core repo::category::list_ordered
  - id: P3
    property: Reordering a set of categories persists the new position values atomically.
    check: cargo test -p kauppa-core repo::category::reorder_persists
  - id: P4
    property: Creating a category with an empty name or an owner-duplicate name is rejected with a CoreError.
    check: cargo test -p kauppa-core repo::category::unique_nonempty_name
  - id: P5
    property: A category can be renamed and deleted; after deletion it is absent from list_for_owner.
    check: cargo test -p kauppa-core repo::category::rename_and_delete
  - id: P6
    property: GET /categories renders the owner's categories; add/rename/reorder/delete return HTML fragments on HX-Request and full pages otherwise; unauthenticated requests are rejected.
    check: cargo test -p kauppa-web routes::categories::crud_and_auth
context:
  - crates/kauppa-core/src/repo/mod.rs
  - crates/kauppa-core/src/domain/mod.rs
  - crates/kauppa-web/src/routes/mod.rs
  - migrations/0001_init.sql
  - .agentic/mentorship/persistence.md
  - .agentic/consults/CRP-0001-product-category-data-model.md
autonomy:
  may_decide:
    - internal shape of CategoryRepo methods and the categories route module
    - handlebars partials used for the category-management fragments
    - whether position values are dense or sparse integers
  escalate:
    - { when: schema change beyond the categories table approved in RR-0001, to: data-owner }
    - { when: public route added or removed, to: api-owner }
  forbidden:
    - edit a committed migration (migrations/0001_init.sql) — add a new numbered file
    - add dependencies
    - lower lint strictness
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/persistence.md@v1
  - .agentic/mentorship/actix-web.md@v1
  - .agentic/mentorship/templating.md@v1
resolutions:
  - RR-0001
---
```

## Context

RR-0001 approved the product-centric data model (Option A in CRP-0001). This
brief lays its first stone: the per-user category taxonomy. The product owner's
brief (`.drafts/kauppalistasovellus.docx`) requires user-defined categories (produce,
cold, baking, frozen, spices…) that products are grouped by, and lets the user
control their order. Nothing in the current scaffold models categories —
`0001_init.sql` has no such table.

Downstream briefs depend on this one: MB-0002 products reference a category,
MB-0003 ingredients inherit one via their product, MB-0004 list items snapshot
one. Keep the taxonomy owner-scoped and ordered; everything else grouping-related
is later work.

## Conceptual plan

Add migration `0002_categories.sql`: a `STRICT` `categories` table with
`owner_id` (`REFERENCES users(id) ON DELETE CASCADE`), `name`, `position`,
`created_at` (RFC3339 TEXT), and `UNIQUE(owner_id, name)`; index
`(owner_id, position)`. Add a `Category` domain type and a `CategoryRepo` trait
(create / list_for_owner / rename / reorder / delete) with a sqlite impl, using
a transaction for the multi-row reorder. Add `routes/categories.rs` (thin:
parse → repo → render) mounted in `routes/mod.rs`, plus `templates/categories/`
following the layout + htmx-fragment pattern.

Checkpoints: repo tests green before wiring the web layer; `fast-gate.sh` green
before packaging. Slow-mode triggers: if deletion appears to require touching a
not-yet-existing `products` table, stop — that cross-table `ON DELETE SET NULL`
behaviour is MB-0002's to prove. If ordering seems to need cross-entity scope,
re-read RR-0001 before diverging.

## Validation plan

P1–P5 are `kauppa-core` integration tests against a temp SQLite pool (owner
isolation, ordering, atomic reorder, uniqueness/empty-name rejection, rename +
delete). P6 is a `kauppa-web` test exercising the route both with and without
`HX-Request` and asserting an unauthenticated request is rejected. Evidence: the
named test outputs in the Merge-Readiness Pack scope-to-proof map.
