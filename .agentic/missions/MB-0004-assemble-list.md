# Mission Brief — Assemble a shopping list (free-text + favorites)

```yaml
---
id: MB-0004
owner: "@juuso-kalliomaki"
goal: Let a user assemble a shopping list by typing free-text items and adding saved favorites, with items soft-deletable and free-text items left uncategorized until a category is chosen.
non_goals:
  - Adding a whole recipe to a list (future build-from-recipe brief).
  - Rendering the category-grouped store view and check-off (future list-view brief).
  - Multiple named lists / resume and sharing (future nice-to-have briefs).
acceptance:
  - id: P1
    property: Adding a favorite to a list creates a list item that snapshots the product's name and its category at add-time.
    check: cargo test -p kauppa-core repo::list::add_favorite_snapshots_category
  - id: P2
    property: A free-text item is created with category_id NULL (uncategorized) and can later be assigned a category.
    check: cargo test -p kauppa-core repo::list::freetext_uncategorized_then_assign
  - id: P3
    property: Removing an item sets removed_at (soft delete); default reads exclude removed items and a removed item can be restored.
    check: cargo test -p kauppa-core repo::list::soft_delete_excludes_and_restores
  - id: P4
    property: Changing a product's category in the catalog after it has been added to a list does not change the list item's category — the item keeps its add-time snapshot.
    check: cargo test -p kauppa-core repo::list::snapshot_is_stable
  - id: P5
    property: List items are owner-isolated via their parent list — a user cannot add to or read another user's list.
    check: cargo test -p kauppa-web routes::lists::owner_isolation
  - id: P6
    property: add-item, add-favorite, assign-category, and remove return HTML fragments on HX-Request and full pages otherwise.
    check: cargo test -p kauppa-web routes::lists::htmx_fragments
context:
  - crates/kauppa-core/src/domain/shopping_list.rs
  - crates/kauppa-core/src/repo/mod.rs
  - crates/kauppa-web/src/routes/lists.rs
  - migrations/0001_init.sql
  - .agentic/missions/MB-0002-products-favorites.md
  - .agentic/mentorship/persistence.md
  - .agentic/consults/CRP-0001-product-category-data-model.md
autonomy:
  may_decide:
    - internal shape of the ShoppingListRepo item methods and the lists route handlers
    - whether the category snapshot stores category_id, or id plus denormalized name
    - handlebars partials for the list-builder fragments
  escalate:
    - { when: "schema change beyond adding list_items.product_id, category_id, and removed_at approved in RR-0001", to: data-owner }
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
resolutions:
  - RR-0001
---
```

## Context

The scaffold has `shopping_lists` and `list_items` (free text, quantity,
checked, position) but only a `ShoppingListRepo::list_for_owner` — no way to add
items. The product owner's brief wants a list built from favorites and free-text
entries (recipes come in a later brief). Three product decisions from the
MB-0004 open questions, recorded in RR-0001, shape this: categories are
per-user; ad-hoc free-text items stay uncategorized until a category is given;
removal is a soft delete. A shopping list is a point-in-time artifact, so items
snapshot their category at add-time and do not reshuffle when the catalog is
later edited.

Depends on MB-0002 (favorites to add). The later build-from-recipe and list-view
briefs build directly on the item model established here.

## Conceptual plan

Add migration `0005_list_item_products.sql`: `ALTER TABLE list_items` to
`ADD COLUMN product_id TEXT REFERENCES products(id) ON DELETE SET NULL`,
`ADD COLUMN category_id TEXT REFERENCES categories(id) ON DELETE SET NULL`, and
`ADD COLUMN removed_at TEXT` (RFC3339; NULL = active). All three are nullable, so
`ADD COLUMN` applies in place. The item's own `category_id` is the snapshot: it
is seeded from the product when a favorite is added (P1), left NULL for free text
(P2), and is independent of later catalog edits (P4). Extend `ShoppingListRepo`
with add-free-text / add-favorite / assign-category / soft-delete / restore,
with default reads filtering `removed_at IS NULL`. Extend `routes/lists.rs` and
`templates/lists/` for the builder.

Checkpoint: P4 (snapshot stability) and P3 (soft delete + restore) are the
subtle ones — test them at the repo layer first. Slow-mode trigger: if "snapshot"
seems to require denormalizing more than the category reference, re-read the
RR-0001 snapshot sub-decision before widening the schema.

## Validation plan

P1–P4 are `kauppa-core` tests against a temp SQLite pool (favorite snapshot,
free-text uncategorized-then-assign, soft-delete exclusion + restore, snapshot
stability across catalog edits). P5 and P6 are `kauppa-web` tests: P5 asserts a
foreign user is rejected on read and add; P6 covers HX vs. full-page rendering
for each mutation. Evidence: named test outputs in the scope-to-proof map.
