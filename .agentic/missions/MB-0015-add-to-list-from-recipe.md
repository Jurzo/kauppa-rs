# Mission Brief — Add a recipe to a list from the recipe page

```yaml
---
id: MB-0015
owner: "@juuso-kalliomaki"
goal: Let a user add a recipe's ingredients to one of their shopping lists straight from the recipe page, choosing the list and a multiplier, with the same snapshot and scaling rules as adding it from the list page.
non_goals:
  - Creating a new list from the recipe page; the control links to /lists when the user has none.
  - Adding several recipes at once.
  - Any change to how items are copied or quantities scaled (MB-0005's `ShoppingListRepo::add_recipe` and `domain::quantity::scale` are reused as they are).
  - Any schema change.
acceptance:
  - id: P1
    property: The recipe page offers the user's own lists, most recently updated first, and a "Times" field defaulting to 1; with no lists it links to /lists instead. Another user's lists never appear.
    check: cargo test -p kauppa-web routes::recipes::add_to_list_form
  - id: P2
    property: "Submitting adds the recipe's ingredients to the chosen list exactly as `POST /lists/{id}/recipes` does (snapshot, multiplier). Over htmx it answers with a confirmation fragment naming the list and the number of items added, with a link to the list; without htmx, a 303 to the list."
    check: cargo test -p kauppa-web routes::recipes::add_to_list
  - id: P3
    property: Another user's list or recipe is a 404 and adds nothing; a missing list choice, a bad multiplier, or a recipe without ingredients shows a message, over htmx and without it.
    check: cargo test -p kauppa-web routes::recipes::add_to_list_rejections
context:
  - crates/kauppa-web/src/routes/recipes.rs
  - crates/kauppa-web/src/routes/lists.rs
  - templates/recipes/show.hbs
  - crates/kauppa-core/src/repo/mod.rs
  - .agentic/missions/MB-0005-add-recipe-to-list.md
  - .agentic/mentorship/actix-web.md
  - .agentic/mentorship/templating.md
autonomy:
  may_decide:
    - the confirmation fragment's markup and where the control sits on the recipe page
    - whether multiplier parsing is shared with routes::lists (one helper) or duplicated
  escalate:
    - { when: "any route besides POST /recipes/{id}/add-to-list is added or removed", to: api-owner }
    - { when: the control seems to need new repo methods beyond reading lists and `add_recipe`, to: data-owner }
    - { when: any schema change, to: data-owner }
  forbidden:
    - change `ShoppingListRepo::add_recipe` semantics or `domain::quantity::scale`
    - add dependencies
    - lower lint strictness
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/actix-web.md@v1
  - .agentic/mentorship/templating.md@v1
---
```

## Context

MB-0005 lets a user add a recipe from the list page (`POST /lists/{id}/recipes`,
recipe picker plus "Times"). The other direction is missing: while looking at
a recipe, there is no way to put it on a list without navigating to the list
first. Drafted as an MB-0005 follow-up at the owner's request (2026-10-31).

Starts after MB-0005 merges.

## Conceptual plan

On the recipe editor (`templates/recipes/show.hbs`), add a small "Add to a
list" form with:

- a `<select>` of the user's lists (`ShoppingListRepo::list_for_owner`, most
  recent first);
- a "Times" field;
- a submit button.

It posts to one new route, `POST /recipes/{id}/add-to-list`, with `list_id`
and `multiplier` fields. The handler parses them the way `routes::lists` does
(ideally one shared helper) and calls `ShoppingListRepo::add_recipe`. Over
htmx it swaps a confirmation fragment into the form's place ("Added 5 items
to Viikonloppu — open list"). Plain requests get a 303 to `/lists/{list}`.

The form posts to a fixed recipe URL with the list as a field, so it works
without JavaScript. Posting to `/lists/{id}/recipes` would need the action to
change with the selection.

Checkpoints: P1 before P2/P3. Slow-mode trigger: if sharing the parsing
helper means moving more than the two small parse functions, keep them
duplicated and note it.

## Validation plan

- P1: `kauppa-web` test of the recipe page: two users' lists, asserting the
  order and the owner scoping, plus the no-lists link.
- P2: `kauppa-web` test: htmx and plain submissions, asserting the items (with
  a multiplier) on the chosen list, the fragment's count and link, and the 303
  target.
- P3: `kauppa-web` test: foreign list, foreign recipe, missing list, bad
  multiplier, and an empty recipe, over htmx and without it; nothing added.
