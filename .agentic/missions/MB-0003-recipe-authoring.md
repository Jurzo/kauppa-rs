# Mission Brief — Recipe authoring with categorized ingredients

```yaml
---
id: MB-0003
owner: "@juuso-kalliomaki"
goal: Let a user create and edit recipes whose ingredients reference catalog products (so each ingredient carries a category) with optional quantities.
non_goals:
  - Adding a recipe's ingredients to a shopping list (future build-list brief).
  - The ingredient-quantity multiplier (future, nice-to-have).
  - Product catalog CRUD (MB-0002) and category CRUD (MB-0001).
acceptance:
  - id: P1
    property: Creating or editing a recipe persists its title, instructions, optional servings, and an ordered ingredient list.
    check: cargo test -p kauppa-core repo::recipe::create_with_ingredients
  - id: P2
    property: Adding an ingredient whose product has no category is rejected at the handler layer (category required per RR-0001).
    check: cargo test -p kauppa-web routes::recipes::ingredient_requires_categorized_product
  - id: P3
    property: Adding an ingredient for a not-yet-saved product name creates a catalog product (is_favorite = 0) in the chosen category and reuses it on repeat.
    check: cargo test -p kauppa-core repo::recipe::ingredient_upserts_product
  - id: P4
    property: Ingredient quantity is optional and round-trips unchanged (nice-to-have quantities delivered).
    check: cargo test -p kauppa-core repo::recipe::ingredient_quantity_roundtrip
  - id: P5
    property: A recipe's ingredients are returned in their stored position order.
    check: cargo test -p kauppa-core repo::recipe::ingredients_ordered
  - id: P6
    property: GET /recipes/{id} renders a recipe with its ingredients; create/edit go through htmx; a user cannot read or edit another user's recipe.
    check: cargo test -p kauppa-web routes::recipes::owner_isolation
context:
  - crates/kauppa-core/src/domain/recipe.rs
  - crates/kauppa-core/src/repo/mod.rs
  - crates/kauppa-web/src/routes/recipes.rs
  - migrations/0001_init.sql
  - .agentic/missions/MB-0002-products-favorites.md
  - .agentic/mentorship/persistence.md
  - .agentic/consults/CRP-0001-product-category-data-model.md
autonomy:
  may_decide:
    - internal shape of the RecipeRepo ingredient methods and the recipes route handlers
    - whether ingredient text is denormalized alongside product_id for display
    - handlebars partials for the recipe editor fragments
  escalate:
    - { when: schema change beyond adding ingredients.product_id approved in RR-0001, to: data-owner }
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

The scaffold already has `recipes` and `ingredients` tables and a
`RecipeRepo::list_for_owner`, but no authoring: no create/edit, and ingredients
are free text unconnected to the catalog. The product owner's brief treats a
recipe ingredient as a product — "when you add a new ingredient to a recipe you
choose a category for the product" — so under RR-0001 an ingredient references a
catalog product, which carries its category. Quantities are a nice-to-have the
existing `ingredients.quantity` column already supports, so this brief delivers
them.

Depends on MB-0002 (products catalog). Feeds the later build-list brief, which
copies a recipe's ingredients onto a list.

## Conceptual plan

Add migration `0005_ingredient_product.sql`: `ALTER TABLE ingredients ADD COLUMN
product_id TEXT REFERENCES products(id) ON DELETE SET NULL`. A nullable FK column
is addable in place (no table rebuild) under SQLite's `ADD COLUMN` rules; keep
the existing `text` column as the denormalized display name. Extend `RecipeRepo`
with create/update recipe and add/replace/reorder ingredients; when an ingredient
names a product that doesn't exist yet, upsert it into the catalog
(`is_favorite = 0`) in the chosen category (P3). Enforce "ingredient needs a
categorized product" at the handler (P2). Extend `routes/recipes.rs` and
`templates/recipes/` for the editor.

Checkpoint: repo tests (P1, P3, P4, P5) green before the web slice. Slow-mode
trigger: if the upsert seems to need a schema change beyond `product_id`, stop —
RR-0001 scopes this brief to that one column.

## Validation plan

P1, P3, P4, P5 are `kauppa-core` tests against a temp SQLite pool (recipe +
ingredient round-trip, product upsert/reuse, optional quantity, ordering). P2
and P6 are `kauppa-web` tests: P2 asserts server-side rejection of an
uncategorized-product ingredient; P6 asserts a foreign user gets a not-found /
forbidden on read and edit. Evidence: named test outputs in the scope-to-proof
map.
