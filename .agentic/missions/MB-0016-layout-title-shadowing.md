# Mission Brief — Page bodies show the layout's title instead of their own `title` fields

```yaml
---
id: MB-0016
owner: "@juuso-kalliomaki"
goal: Make every full page render its own `title` values (recipe titles, typed drafts) instead of the page title passed to the layout, and pin the behavior with tests so the shadowing can't come back unnoticed.
non_goals:
  - Redesigning the layout or the page-title format (`<title>{page} — kauppa</title>` stays).
  - Upgrading or replacing handlebars (a dependency change).
  - Other template clean-ups seen along the way — file a separate brief.
acceptance:
  - id: P1
    property: On the first (full-page) load of `/lists/{id}`, the "Recipe" picker lists each recipe by its own title, the same as the htmx fragment does; the page title is still the list's name.
    check: cargo test -p kauppa-web routes::lists::recipe_picker_shows_titles
  - id: P2
    property: The `/recipes` index shows each recipe card with its own title; the page title is still "Recipes".
    check: cargo test -p kauppa-web routes::recipes::index_shows_titles
  - id: P3
    property: The new-recipe title field on `/recipes` is empty on a normal load, and after a rejected submission without htmx it holds what the user typed.
    check: cargo test -p kauppa-web routes::recipes::new_recipe_draft
  - id: P4
    property: After a rejected recipe-details edit without htmx, the title field holds the title the user typed, not the stored one; the page heading still shows the stored title.
    check: cargo test -p kauppa-web routes::recipes::details_draft_survives_rejection
  - id: P5
    property: Inside the layout, a body field with the same name as a layout parameter renders its own value, so a future view field can't be shadowed silently.
    check: cargo test -p kauppa-web templates::layout_params_do_not_shadow_body
context:
  - templates/layout.hbs
  - templates/lists/show.hbs
  - templates/lists/_list.hbs
  - templates/recipes/index.hbs
  - templates/recipes/_new.hbs
  - templates/recipes/show.hbs
  - templates/recipes/_details.hbs
  - crates/kauppa-web/src/templates.rs
  - crates/kauppa-web/src/routes/recipes.rs
  - crates/kauppa-web/src/routes/lists.rs
  - AGENTS.md
  - .agentic/mentorship/templating.md
  - .agentic/runbooks/bugfix.md
autonomy:
  may_decide:
    - the new layout parameter name under Option A (e.g. `page_title`), or the renamed view fields under Option B
    - test placement within the named modules and the exact assertions
  escalate:
    - { when: "the fix changes the documented layout convention (`{{#> layout title=…}}` in AGENTS.md → Conventions and templating.md) — Option A", to: owner }
    - { when: the fix seems to need a handlebars upgrade or a custom helper crate, to: platform-owner }
    - { when: any route changes, to: api-owner }
  forbidden:
    - add, remove, or bump dependencies
    - turn off handlebars strict mode
    - lower lint strictness
    - edit AGENTS.md or Mentorship Packs without an approved RR
runbook: .agentic/runbooks/bugfix.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
  - .agentic/mentorship/actix-web.md@v1
---
```

## Context

Reported by the owner (2026-11-01): "On initial loading of a list, the recipe
list … shows the name of the shopping list itself for the recipe items."

**Root cause (confirmed).** Every page is
`{{#> layout title=… signed_in=…}}…{{/layout}}` (AGENTS.md → Conventions).
In handlebars 6.4.4, a partial's hash parameters become **block params** of
the partial's block (`src/partial.rs`: `set_block_param(name, …)` for each
hash entry). Block params are looked up before the context and inherited by
nested blocks such as `{{#each}}`. So inside a page body, `title` and even
`this.title` resolve to the layout's `title`, not to the field of the current
object.

htmx fragments are rendered without the layout, so they show the right values.
That is why the list page looks correct after any in-page action, and why no
test caught it. The existing tests assert on fragments, or on links rather
than titles.

**Repro** (fresh DB, one user, recipes "Pannukakku" and "Puuro", a list
"Viikonloppu"). Expected vs actual, all with a signed-in session:

| # | Request | Expected | Actual |
| --- | --- | --- | --- |
| 1 | `GET /lists/{id}` (no `HX-Request`) | Recipe `<option>`s "Puuro", "Pannukakku" | both say "Viikonloppu" |
| 1′ | same with `HX-Request: true` | "Puuro", "Pannukakku" | correct |
| 2 | `GET /recipes` | card headings "Pannukakku", "Puuro" | both say "Recipes" |
| 3 | `GET /recipes` | new-recipe input `value=""` | `value="Recipes"` |
| 3′ | `POST /recipes` `title=%20` (no htmx) | input keeps the typed value | `value="Recipes"` |
| 4 | `POST /recipes/{id}` `title=Uusi nimi&servings=abc` (no htmx) | input `value="Uusi nimi"` | `value="Pannukakku"` (stored title) |
| 4′ | same with `HX-Request: true` | `value="Uusi nimi"` | correct |

**Blast radius.** The layout is used by ten templates. A body is affected
where it references a field named after a layout parameter: `title` (rows 1–4
above), or `signed_in`. `templates/index.hbs` reads `signed_in` from its own
context in its body (it passes no `signed_in` to the layout), and must keep
working. `recipes/show.hbs`'s `<h1>{{title}}</h1>` is
correct today only because it passes `title=title`.

## Conceptual plan

Red → green (bugfix runbook): commit the P1–P5 tests first, failing, then the
fix in a later commit.

Two fixes, for the owner to choose at acceptance:

- **Option A — rename the layout's parameter (recommended).** For example
  `{{#> layout page_title=…}}`, with `layout.hbs` reading `{{page_title}}`.
  Bodies keep their natural field names, and a domain field is very unlikely
  ever to be called `page_title`. Decide in the same step whether `signed_in`
  gets a namespaced name too. This changes the documented convention in
  AGENTS.md → Conventions and in `templating.md`. Those are Mentorship Pack
  edits, so they need a consult (CRP-0006) and an approved RR before the PR,
  rolled back into both files. The P5 guard then pins "body fields are never
  shadowed".
- **Option B — rename the colliding view fields.** For example
  `RecipeOption.title` → `name`, `RecipeSummary.title` → `name`, and the draft
  fields → `draft_title`. No convention change and no governance step, but the
  trap stays for the next field called `title`. P5 then pins the rule
  differently: a test that fails if any page body uses a field named like a
  layout parameter, plus a note in `templating.md` (also a Mentorship Pack
  edit, so an RR, though a smaller one).

Checkpoints: the failing tests reproduce rows 1–4 before any fix. Slow-mode
trigger: if a body turns out to depend on the leak (reading a layout
parameter as if it were its own field), make that value explicit in its view
rather than keep relying on the leak.

## Decision (owner, 2026-11-01)

**Option A**: rename the layout's parameters. The convention change goes
through CRP-0006 (open; it recommends the `page_` prefix for both `title` and
`signed_in`) and an RR, which must be approved before the fix PR. Option B is
rejected.

## Validation plan

- P1–P4: `kauppa-web` tests through `routes::test_support`, on full-page
  responses without `HX-Request`, asserting the exact titles and `value=`
  attributes in the repro table. Each asserts the page `<title>` too, so the
  fix doesn't break it.
- P5: a unit test in `templates.rs` that renders a small template through the
  real registry. Inside the layout, an `{{#each}}` over objects with a `title`
  field (and a `signed_in` field) must render their own values.
- Evidence: the red commit's test output (or a failing CI run) and the green
  run, linked in the Merge-Readiness Pack.
