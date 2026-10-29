# Mission Brief — Product catalog / favorites

```yaml
---
id: MB-0002
owner: "@juuso-kalliomaki"
goal: Let a user save favorite products, each assigned to a category, and browse them grouped by category — the per-user product catalog that recipes and lists draw from.
non_goals:
  - Adding a product to a shopping list (MB-0004) or a recipe (MB-0003).
  - Category CRUD and ordering (MB-0001).
  - Recipe-specific quantities (MB-0003).
acceptance:
  - id: P1
    property: Creating a favorite product requires an existing category owned by the same user; a missing or foreign category id is rejected at the handler layer (category required per RR-0001).
    check: cargo test -p kauppa-web routes::products::create_requires_owned_category
  - id: P2
    property: Product names are unique per owner and non-empty; duplicates and empty names are rejected.
    check: cargo test -p kauppa-core repo::product::unique_nonempty_name
  - id: P3
    property: list_for_owner returns products grouped by category and ordered by position within each category (nice-to-have position delivered).
    check: cargo test -p kauppa-core repo::product::grouped_ordered
  - id: P4
    property: Deleting a category sets its products' category_id to NULL (uncategorized) and never deletes the product row.
    check: cargo test -p kauppa-core repo::product::category_delete_sets_null
  - id: P5
    property: A product can be renamed, recategorized, repositioned within a category, and deleted.
    check: cargo test -p kauppa-core repo::product::mutations
  - id: P6
    property: GET /favorites renders the catalog grouped by category; create/edit/delete return HTML fragments on HX-Request and full pages otherwise; unauthenticated requests are rejected.
    check: cargo test -p kauppa-web routes::products::crud_and_auth
context:
  - crates/kauppa-core/src/domain/mod.rs
  - crates/kauppa-core/src/repo/mod.rs
  - crates/kauppa-web/src/routes/mod.rs
  - migrations/0001_init.sql
  - .agentic/missions/MB-0001-categories.md
  - .agentic/mentorship/persistence.md
  - .agentic/consults/CRP-0001-product-category-data-model.md
autonomy:
  may_decide:
    - internal shape of ProductRepo methods and the products/favorites route module
    - whether is_favorite is a column or inferred, within the RR-0001 shape
    - handlebars partials used for the catalog fragments
  escalate:
    - { when: schema change beyond the products table approved in RR-0001, to: data-owner }
    - { when: public route added or removed, to: api-owner }
  forbidden:
    - edit a committed migration (migrations/0001_init.sql, 0002_categories.sql) — add a new numbered file
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

Per RR-0001, "favorite products" are the per-user product catalog: a `products`
table whose rows carry a category and are reused by recipes (MB-0003) and lists
(MB-0004). The product owner's brief requires that saving a favorite forces the
user to choose a category for the product; RR-0001 resolved that `category_id`
is nullable in the DB (so category deletion is a safe `SET NULL`) with the
"must choose a category" rule enforced at the handler layer. This brief owns
both that enforcement and the `SET NULL`-on-category-delete behaviour.

Depends on MB-0001 (categories must exist to assign one). Blocks MB-0003 and
MB-0004.

## Conceptual plan

Add migration `0003_products.sql`: a `STRICT` `products` table —
`owner_id REFERENCES users(id) ON DELETE CASCADE`,
`category_id REFERENCES categories(id) ON DELETE SET NULL` (nullable),
`name`, `position` (within category), `is_favorite` (default 1), timestamps,
`UNIQUE(owner_id, name)`; index `(owner_id, category_id, position)`. The
`ON DELETE SET NULL` divergence from the default CASCADE is justified in the
migration header (deleting a taxonomy entry must not destroy products).

Add a `Product` domain type and `ProductRepo` (create / list_for_owner grouped /
rename / recategorize / reorder / delete). Enforce "category required" in the
handler, not the schema (P1), returning a `WebError` on a missing/foreign
category. Add `routes/products.rs` (`/favorites`) + `templates/products/`.

Checkpoint: P4 is the key cross-table test — delete a category, assert member
products survive with `category_id = NULL`. Slow-mode trigger: if enforcing
"category required" seems to want a NOT NULL column, stop — that contradicts
RR-0001; keep it handler-level.

## Validation plan

P2–P5 are `kauppa-core` tests against a temp SQLite pool (uniqueness, grouped
ordering, `SET NULL` on category delete, mutations). P1 and P6 are `kauppa-web`
tests: P1 asserts the server rejects a create with no/foreign category even if a
client skips the UI control; P6 covers HX vs. full-page rendering and the
unauthenticated path. Evidence: named test outputs in the scope-to-proof map.
