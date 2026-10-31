# Mission Brief — See and manage recipe-created products

```yaml
---
id: MB-0014
owner: "@juuso-kalliomaki"
goal: Let a user see the products created from recipe ingredients (hidden from favorites), with their category and how many recipes use them, and manage them — make one a favorite, set or change its category, rename it, or delete it.
non_goals:
  - Turning a favorite back into a hidden product (un-favoriting); see the open question below.
  - Merging two products into one.
  - Automatically deleting products that no recipe uses any more.
  - Any schema change; `is_favorite` and the product columns already exist.
acceptance:
  - id: P1
    property: The repo lists the owner's recipe-created (non-favorite) products grouped by category like favorites (uncategorized last), each with the number of the owner's recipes that use it; never another user's.
    check: cargo test -p kauppa-core repo::product::list_hidden_with_usage
  - id: P2
    property: Making a recipe-created product a favorite keeps its id, name, and category, and appends it at the end of that category's favorites; one without a category is refused (`Invalid`, RR-0001 — a favorite needs a category) until a category is set.
    check: cargo test -p kauppa-core repo::product::make_favorite
  - id: P3
    property: /favorites shows these products in their own section below the favorites, with category and usage count, and every action (make favorite, rename, change category, delete) works over htmx and without it. Another user's products never appear.
    check: cargo test -p kauppa-web routes::products::hidden_products_section
  - id: P4
    property: A recipe-created product left uncategorized by a category deletion can be given a category from this section, after which it is accepted as a recipe ingredient again (closes the MB-0003 P2 dead end).
    check: cargo test -p kauppa-web routes::products::hidden_uncategorized_fixable
  - id: P5
    property: Deleting a product that recipes use asks for confirmation naming the usage count; afterwards those ingredients keep their name and quantity, unlinked (RR-0001 constraint 4).
    check: cargo test -p kauppa-web routes::products::delete_used_product_warns
context:
  - crates/kauppa-core/src/repo/mod.rs
  - crates/kauppa-core/src/repo/sqlite.rs
  - crates/kauppa-core/src/repo/product.rs
  - crates/kauppa-web/src/routes/products.rs
  - templates/products/_catalog.hbs
  - .agentic/missions/MB-0002-products-favorites.md
  - .agentic/missions/MB-0003-recipe-authoring.md
  - .agentic/resolutions/RR-0001-product-category-data-model.md
  - .agentic/mentorship/persistence.md
autonomy:
  may_decide:
    - the names and shapes of the new ProductRepo methods
    - the section's heading and markup, and whether it is collapsed by default
    - ordering of hidden products within a category (position, or name)
  escalate:
    - { when: "any route besides POST /favorites/{id}/favorite is added or removed", to: api-owner }
    - { when: any schema change, to: data-owner }
    - { when: the section seems to belong on a separate page rather than /favorites, to: product }
  forbidden:
    - edit a committed migration
    - delete recipe ingredients when deleting a product
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

MB-0003 (PR #13) creates a product with `is_favorite = 0` when an ingredient
names a new product, and `/favorites` lists only favorites (owner decision 2).
Those products are reused by name but can't be seen or edited anywhere. One
case is a dead end: if such a product's category is deleted, MB-0003 P2
rejects it as an ingredient ("has no category"), and the only fix today is to
add it as a favorite, which promotes it (decision 3).

The `/favorites/{id}/rename`, `/category`, and `/delete` handlers already
work on any product the user owns, favorite or not, through the same
owner-scoped `ProductRepo` methods. Only listing and an explicit "make
favorite" are missing.

Starts after MB-0003 merges.

## Conceptual plan

- Add a `ProductRepo` read for non-favorite products, grouped like
  `list_for_owner`, with a usage count from `ingredients` joined to `recipes`
  on the owner (RR-0001 constraint 9).
- Add a `make_favorite` method: same row, `is_favorite = 1`, appended to its
  category, refused when uncategorized. Its route is
  `POST /favorites/{id}/favorite`.
- Render a second section in `products/_catalog.hbs` (within the `#favorites`
  fragment, so every existing action's htmx response re-renders both lists).
  Reuse the existing row actions and the category picker. The delete
  confirmation names the usage count.

Checkpoints: P1–P2 green at the repo layer before the web slice. Slow-mode
triggers:

- the existing `/favorites/{id}/…` handlers turn out to need a favorites-only
  guard;
- moving hidden products within a category needs its own ordering rules
  (MB-0003 made favorites step past them).

In either case, stop and re-plan.

Open question for the owner at acceptance: should a favorite be demotable back
to a recipe-only product, instead of deleting it when it's still used in
recipes? That would be one more route and is listed as a non-goal here.

## Validation plan

- P1, P2: `kauppa-core` tests on a temp SQLite pool. Hidden products in two
  categories plus an uncategorized one, used by different numbers of recipes,
  a second owner as control; `make_favorite` on categorized and uncategorized
  products.
- P3–P5: `kauppa-web` tests through `routes::test_support`:
  - P3: the section's contents and each action, htmx and plain.
  - P4: recategorize, then add the product as an ingredient → accepted.
  - P5: the confirmation text shows the count; after deletion the recipe still
    lists the ingredient by name.
