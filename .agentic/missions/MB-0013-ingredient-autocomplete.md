# Mission Brief — Autocomplete for ingredient names

```yaml
---
id: MB-0013
owner: "@juuso-kalliomaki"
goal: When a user types an ingredient name in the recipe editor, suggest the products already in their catalog and show which category an existing product will bring, so recipes reuse products instead of near-duplicates and the category choice is never silently ignored.
non_goals:
  - Fuzzy or typo-tolerant matching; suggestions are the user's own product names, and matching stays the `name_key` rule (RR-0003).
  - Suggestions in other forms (adding a favorite, shopping-list items); they can reuse this later.
  - New JavaScript beyond htmx, or any new dependency.
  - Changing how an existing product's category is chosen (MB-0003 decision 1, one category per product).
acceptance:
  - id: P1
    property: The repo returns the owner's product names for suggestions — favorites and recipe-created products alike — sorted by name ignoring case, and never another user's.
    check: cargo test -p kauppa-core repo::product::suggestion_names
  - id: P2
    property: "The recipe editor's ingredient name input offers those names through a native `<datalist>` (`list=` on the input), so it works on mobile keyboards and without JavaScript; another user's products never appear."
    check: cargo test -p kauppa-web routes::recipes::ingredient_suggestions
  - id: P3
    property: "When the typed name matches an existing product ignoring case, the add form shows that product's category and that the category picker won't apply (\"Uses Kylmä\"); for a new name it asks for a category. The hint updates over htmx; without JavaScript the form still submits and is validated as before."
    check: cargo test -p kauppa-web routes::recipes::ingredient_match_hint
context:
  - crates/kauppa-core/src/repo/mod.rs
  - crates/kauppa-core/src/repo/sqlite.rs
  - crates/kauppa-core/src/repo/name_key.rs
  - crates/kauppa-web/src/routes/recipes.rs
  - templates/recipes/_ingredients.hbs
  - .agentic/missions/MB-0003-recipe-authoring.md
  - .agentic/mentorship/templating.md
autonomy:
  may_decide:
    - the name and shape of the ProductRepo method returning suggestion names
    - the htmx trigger for the hint (e.g. `input changed delay:300ms`) and its markup
    - whether the hint fragment is a separate partial
  escalate:
    - { when: "any route besides GET /recipes/{id}/ingredients/match is added or removed", to: api-owner }
    - { when: a user's catalog makes the datalist too large for the page budget (see plan), to: product }
    - { when: any schema change, to: data-owner }
  forbidden:
    - add dependencies or JavaScript files
    - match names other than through `name_key`
    - lower lint strictness
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/actix-web.md@v1
  - .agentic/mentorship/templating.md@v1
resolutions:
  - RR-0003
---
```

## Context

In MB-0003's editor (PR #13), an ingredient name that matches an existing
product reuses it, and that product keeps its own category (owner decision 1).
Today the user can't see either fact while typing: they don't know which
names exist, and the category picker looks like it applies when it doesn't.
Products created from recipes are hidden from `/favorites`, so they are
invisible everywhere.

Per-user catalogs are tens to low hundreds of products, which a server-rendered
`<datalist>` handles without a search endpoint. AGENTS.md goal 2 (server
rendered, works on a slow phone) favors that over a type-ahead request per key.

Starts after MB-0003 merges.

## Conceptual plan

- **Suggestions (P1, P2):** a `ProductRepo` read of the owner's product names,
  ordered by `name_key`. The editor renders it once as a `<datalist>`, and the
  add form's name input references it.
- **Match hint (P3):** one new route, `GET /recipes/{id}/ingredients/match?name=`,
  owner-scoped like the rest of `/recipes/{id}`. It returns a small fragment
  next to the picker: the matched product's category, a note that the picker
  is ignored, or "new product — choose a category". The name input triggers
  it with `hx-get`, `hx-trigger="input changed delay:300ms"`, and an explicit
  `hx-target`. The lookup is `ProductRepo::find_by_name` (RR-0003).
- Both pieces are display hints: the server-side rule in
  `routes::recipes::check_ingredient` is unchanged, and stays the only
  enforcement.

Checkpoints: P1 green before the web slice. Slow-mode trigger: if a realistic
catalog (say 500 names) pushes the editor's p95 past 100 ms or bloats the page
noticeably on 3G, stop and consult. The alternative is an htmx search endpoint
returning `<option>`s.

## Validation plan

- P1: `kauppa-core` test: favorites and hidden products of two users, mixed
  case and Å/Ä/Ö, asserting order and owner scoping.
- P2: `kauppa-web` test: the editor contains a `<datalist>` with the user's
  names, linked by `list=`, and none of another user's.
- P3: `kauppa-web` test of the match route: an existing product (any case)
  names its category, a new name asks for one, another user's recipe is a 404,
  and the response is a fragment. The editor markup carries the `hx-get` and
  `hx-target` attributes.
