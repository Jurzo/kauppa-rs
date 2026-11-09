# Mission Brief — Recipe editor: ask for a category only for new products

```yaml
---
id: MB-0039
owner: "@juuso-kalliomaki"
goal: "When adding an ingredient that is already a product (e.g. a favorite), the recipe editor doesn't offer a category choice it would ignore; a category is asked for only when the ingredient is a new product, either in the form or right after submitting."
non_goals:
  - "Changing a product's category from the recipe editor (one category per product, MB-0003)."
  - "Changing `name_key` matching (RR-0003) or the suggestions (MB-0013)."
  - "JavaScript beyond htmx."
acceptance:
  - id: P2
    property: "If the owner chooses the ask-after-submit flow instead: submitting a new product's name without a category answers with a category prompt for that ingredient (keeping name and quantity) rather than an error, and choosing a category there adds it; an existing product is added in one step."
    check: cargo test -p kauppa-web routes::recipes::tests::editor::new_product_asks_category
  - id: P3
    property: "An existing product without a category is handled as the owner decides (today: an error pointing to Favorites), and no product is ever created without a category from the recipe editor (RR-0001)."
    check: cargo test -p kauppa-web routes::recipes::tests::editor::uncategorized_product
  - id: P4
    property: "The existing recipe editor and suggestion tests pass unchanged, or the brief delta lists the ones that change."
    check: cargo test -p kauppa-web routes::recipes::tests
context:
  - templates/recipes/_ingredients.hbs
  - templates/recipes/_match_hint.hbs
  - templates/products/_category_picker.hbs
  - crates/kauppa-web/src/routes/recipes/ingredients.rs
  - crates/kauppa-web/src/routes/recipes/mod.rs
  - .agentic/missions/MB-0013-ingredient-autocomplete.md
  - .agentic/missions/MB-0014-recipe-created-products.md
  - .agentic/resolutions/RR-0001-product-category-data-model.md
autonomy:
  may_decide:
    - "how the match response updates the picker (e.g. an out-of-band swap from `ingredient_match`), within htmx"
    - "test names inside `routes::recipes::tests::editor`"
  escalate:
    - { when: "choosing between P1 (picker follows the match as you type) and P2 (ask after submitting), or both", to: owner }
    - { when: "letting the user choose a category for an existing uncategorized product from the recipe editor (changes MB-0013's rule)", to: owner }
    - { when: "any change to how products are created or categorised in core (RR-0001)", to: data-owner }
  forbidden:
    - "create products without a category"
    - "inline scripts"
    - "schema changes"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
  - .agentic/mentorship/actix-web.md@v1
resolutions: []
---
```

## Context

Requested by the owner (2026-11-09): "Category field should be disabled (or
not visible) when adding an existing favorite to a recipe (possibly ask for
a category after submitting and checking the ingredient)."

Today (`_ingredients.hbs`) the add form always shows "Category (for a new
product)". As the name is typed, `GET /recipes/{id}/ingredients/match`
swaps `#ingredient-match` with a hint: "Uses <category> — the category
picker doesn't apply to an existing product", "<name> has no category —
choose one for it on Favorites first", or "New product — choose a category
for it" (`match_hint`, mirroring `check_ingredient`). The picker itself
never changes, so users fill a field that is ignored.

Two designs fit the request: P1 keeps one form and makes the picker follow
the hint (an out-of-band swap of the picker from the match response);
P2 drops the picker from the add form and asks for a category only after
the server finds the name is new. P2 also works without htmx and on a
slow phone where the hint arrives late. The owner chooses at acceptance;
the unchosen property is dropped in a brief delta.

Owner chose P2 on 2026-11-09 ("P2, ask after submit"; brief accepted the
same day). P1 (picker follows typing) is dropped from the acceptance list;
the remaining ids keep their numbers. P3: the existing-but-uncategorized
case keeps today's behaviour (the error pointing to Favorites). The match
hint stays as an informational hint, reworded so it no longer mentions a
picker.

## Conceptual plan

1. Owner picks P1, P2 or both; decide the uncategorized-product case.
2. Tests first, then template and handler changes.

## Validation plan

- Route tests over the match fragment and the add flow, with and without
  `HX-Request`.
- Smoke: add an existing favorite and a new product on a phone-width
  browser.
