# Mission Brief — Add a recipe to a shopping list

```yaml
---
id: MB-0005
owner: "@juuso-kalliomaki"
goal: Let a user pour a saved recipe's ingredients onto a shopping list in one action, snapshotting each ingredient's product, category, and quantity, with an optional whole-recipe quantity multiplier.
non_goals:
  - Free-text and single-favorite add flows (MB-0004).
  - Rendering the grouped store view and check-off (MB-0006).
  - Editing recipes or the product catalog (MB-0003, MB-0002).
  - Any schema change — the list-item columns from MB-0004 are sufficient.
acceptance:
  - id: P1
    property: Adding a recipe to a list appends one list item per recipe ingredient, each snapshotting the ingredient's product, category, and quantity at add-time.
    check: cargo test -p kauppa-core repo::list::add_recipe_snapshots_ingredients
  - id: P2
    property: Adding a recipe never mutates the recipe or the catalog — it only inserts list items, so re-adding or editing the taxonomy later leaves existing items unchanged.
    check: cargo test -p kauppa-core repo::list::add_recipe_is_nondestructive
  - id: P3
    property: With multiplier N, an ingredient quantity that has a parseable leading number is scaled by N; a quantity with no parseable number is carried through unchanged (nice-to-have multiplier delivered).
    check: cargo test -p kauppa-core repo::list::add_recipe_quantity_multiplier
  - id: P4
    property: An ingredient whose product is uncategorized adds as an uncategorized list item (category_id NULL) rather than being dropped.
    check: cargo test -p kauppa-core repo::list::add_recipe_handles_uncategorized
  - id: P5
    property: A user cannot add from a recipe they do not own, nor onto a list they do not own.
    check: cargo test -p kauppa-web routes::lists::add_recipe_owner_isolation
  - id: P6
    property: POST add-recipe returns an HTML fragment of the updated list on HX-Request and a full page otherwise; unauthenticated requests are rejected.
    check: cargo test -p kauppa-web routes::lists::add_recipe_htmx
context:
  - crates/kauppa-core/src/domain/recipe.rs
  - crates/kauppa-core/src/domain/shopping_list.rs
  - crates/kauppa-core/src/repo/mod.rs
  - crates/kauppa-web/src/routes/lists.rs
  - crates/kauppa-web/src/routes/recipes.rs
  - .agentic/missions/MB-0003-recipe-authoring.md
  - .agentic/missions/MB-0004-assemble-list.md
autonomy:
  may_decide:
    - internal shape of the add-recipe repo method and whether it runs in one transaction
    - the quantity-multiplier parsing rule (leading-number scaling; non-numeric passthrough)
    - whether the multiplier UI is a per-add input or a stepper, and the fragment markup
  escalate:
    - { when: a schema change turns out to be needed (e.g. a new column), to: data-owner }
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

With recipes authored (MB-0003) and the list-item model in place (MB-0004), this
brief delivers the product owner's headline convenience: "select previously
saved recipes into the list, so the list gets the recipe's ingredients and their
quantities." The nice-to-have "the recipe's ingredient list can be multiplied"
is included as the multiplier (P3).

No schema change is needed — `list_items` already carries `product_id`,
`category_id`, `quantity`, and `removed_at` from MB-0004's `0005` migration, so
this brief carries no Resolution Record. Adding a recipe is a read of the
recipe's ingredients followed by a batch insert of snapshotted list items.

## Conceptual plan

Extend `ShoppingListRepo` with `add_recipe(list_id, recipe_id, multiplier)`:
load the recipe's ingredients (owner-checked), and for each insert a `list_item`
that snapshots the ingredient's product, the product's category, and the
quantity — mirroring the snapshot semantics MB-0004 established for favorites.
Do the inserts in one transaction so a partial add can't leave a half-populated
list. Add a POST handler on the lists route and an htmx fragment that re-renders
the affected list.

Quantity is free-form TEXT ("2 dl", "a pinch"), so the multiplier parses a
leading numeric prefix and scales only that, leaving the unit and any
non-numeric quantity untouched (P3) — document the rule next to the parser.

Checkpoints: repo tests (P1–P4) green before the web slice. Slow-mode triggers:
if snapshotting seems to need a new column, stop and escalate to data-owner
(this brief asserts none is needed); if the multiplier tempts a numeric
quantity schema, that is a separate future decision, not this brief.

## Validation plan

P1–P4 are `kauppa-core` tests against a temp SQLite pool (ingredient snapshot,
non-destructiveness, multiplier scaling incl. non-numeric passthrough,
uncategorized handling). P5 and P6 are `kauppa-web` tests: P5 asserts
cross-owner recipe/list rejection; P6 covers HX fragment vs. full page and the
unauthenticated path. Evidence: named test outputs in the scope-to-proof map.
