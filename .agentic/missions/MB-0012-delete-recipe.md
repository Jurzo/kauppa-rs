# Mission Brief — Delete a recipe

```yaml
---
id: MB-0012
owner: "@juuso-kalliomaki"
goal: Let a user delete one of their recipes, together with its ingredient list, from the recipe editor, while the catalog products those ingredients used stay in place.
non_goals:
  - Undo, a trash, or soft delete for recipes (see the open question below).
  - Deleting several recipes at once.
  - Cleaning up recipe-created products that no recipe uses any more (MB-0014 makes them visible and deletable).
  - Shopping lists built from the recipe; MB-0005 copies ingredients into list items, which must not depend on the recipe still existing.
acceptance:
  - id: P1
    property: Deleting a recipe removes it and its ingredients; every product its ingredients referenced, favorite or recipe-created, is still in the catalog unchanged.
    check: cargo test -p kauppa-core repo::recipe::delete_keeps_products
  - id: P2
    property: Deleting another user's recipe, or one that doesn't exist, is `NotFound` and changes nothing.
    check: cargo test -p kauppa-core repo::recipe::delete_owner_scoped
  - id: P3
    property: "`POST /recipes/{id}/delete` from the editor asks for confirmation, then lands on /recipes without the recipe: `HX-Redirect` over htmx, a 303 otherwise. Another user gets a 404."
    check: cargo test -p kauppa-web routes::recipes::delete_recipe
context:
  - crates/kauppa-core/src/repo/mod.rs
  - crates/kauppa-core/src/repo/sqlite.rs
  - crates/kauppa-core/src/repo/recipe.rs
  - crates/kauppa-web/src/routes/recipes.rs
  - templates/recipes/show.hbs
  - migrations/0001_init.sql
  - .agentic/missions/MB-0003-recipe-authoring.md
  - .agentic/mentorship/persistence.md
autonomy:
  may_decide:
    - the name and signature of the RecipeRepo delete method
    - where the delete control sits in the editor and its confirmation wording
  escalate:
    - { when: "any route besides POST /recipes/{id}/delete is added or removed", to: api-owner }
    - { when: any schema change, to: data-owner }
  forbidden:
    - edit a committed migration — add a new numbered file (none is expected)
    - delete or modify products when deleting a recipe
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

MB-0003 (PR #13) added recipe authoring but no way to delete a recipe. The
schema already supports it: `ingredients.recipe_id` is
`REFERENCES recipes(id) ON DELETE CASCADE` (`0001_init.sql`), so deleting the
recipe row deletes its ingredients. `ingredients.product_id` points the other
way (`ON DELETE SET NULL`, `0005`), so deleting ingredients never touches
products. RR-0001 constraint 4: deleting user data never deletes catalog rows.

Starts after MB-0003 merges; it extends that brief's `RecipeRepo` and
`routes/recipes.rs`.

## Conceptual plan

Add a `delete` method to `RecipeRepo` that is owner-scoped in its single
statement (`DELETE … WHERE id = ? AND owner_id = ?`) and returns `NotFound` when
no row is affected; the cascade removes the ingredients. Add one route, `POST
/recipes/{id}/delete` (POST, not DELETE, so the no-JS form works), in the
style of `/favorites/{id}/delete`. The editor gets a delete button with
`hx-confirm`; over htmx it answers `HX-Redirect: /recipes`, otherwise 303.

Checkpoints: P1–P2 green at the repo layer before the route. Slow-mode
trigger: if a later brief (MB-0005) has added a reference to `recipes(id)`
from list items, re-read its decision before deleting. A cascade there would
destroy list data.

Open question for the owner at acceptance: is `hx-confirm` enough, or should
deletion be undoable (a soft delete with `deleted_at`, which would be a schema
change and an RR)? This brief assumes `hx-confirm`.

## Validation plan

- P1, P2: `kauppa-core` tests on a temp SQLite pool. Create a recipe whose
  ingredients use one favorite and one recipe-created product, delete it, and
  assert the recipe and ingredient rows are gone while both products are
  unchanged. Also cover a foreign and a missing id.
- P3: `kauppa-web` test through `routes::test_support`: htmx and plain
  requests, the index no longer lists it, another user gets a 404, and the
  page shows the confirmation attribute.
