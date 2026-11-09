# Mission Brief — One add field with favorite suggestions

```yaml
---
id: MB-0036
owner: "@juuso-kalliomaki"
goal: "On the list page, adding a favorite and adding a free-form item happen in one field: as the user types, their favorites appear as completion suggestions; submitting a favorite's name adds that favorite (with its category), and any other text is added as a free-form item."
non_goals:
  - "Fuzzy or typo-tolerant matching; a name matches a favorite only by the `name_key` rule (RR-0003)."
  - "Changes to adding a recipe's ingredients to a list."
  - "Creating favorites from the list page."
  - "JavaScript beyond htmx, or new dependencies."
acceptance:
  - id: P1
    property: "The list page has one add form (item text and optional quantity) for users who can edit; the separate favorite `<select>` form is gone."
    check: cargo test -p kauppa-web routes::lists::tests::items::single_add_form
  - id: P2
    property: "The item field offers the user's favorites as suggestions (a `<datalist>` or the approach the RR picks), outside the list fragment so htmx swaps don't resend them."
    check: cargo test -p kauppa-web routes::lists::tests::items::favorite_suggestions
  - id: P3
    property: "Submitting text whose `name_key` equals one of the favorites adds that favorite exactly as the old favorite form did (product link, its name and category copied, quantity kept); any other text adds a free-form, uncategorized item as today."
    check: cargo test -p kauppa-web routes::lists::tests::items::add_resolves_favorite
  - id: P4
    property: "Whose favorites are suggested and matched follows the RR for shared lists (owner's or the editor's own), and a `view` grantee still can't add anything (403)."
    check: cargo test -p kauppa-web routes::lists::tests::sharing_roles::add_follows_roles
  - id: P5
    property: "List access still goes through the central predicate (bypass guard green)."
    check: cargo test -p kauppa-core repo::share::list_queries_route_through_central_predicate
context:
  - templates/lists/_list.hbs
  - templates/lists/show.hbs
  - templates/recipes/show.hbs
  - crates/kauppa-web/src/routes/lists/items.rs
  - crates/kauppa-web/src/routes/lists/mod.rs
  - crates/kauppa-core/src/repo/list/
  - crates/kauppa-core/src/repo/product/
  - crates/kauppa-core/src/repo/name_key.rs
  - .agentic/missions/MB-0013-ingredient-autocomplete.md
  - .agentic/resolutions/RR-0002-list-sharing-authz.md
  - .agentic/resolutions/RR-0003-case-insensitive-names-amendment.md
autonomy:
  may_decide:
    - "reusing MB-0013's datalist pattern and `ProductRepo` queries"
    - "whether `POST /lists/{id}/favorites` stays as a no-JS fallback or is removed, provided P1 holds for the page"
    - "test placement inside `routes::lists::tests`"
  escalate:
    - { when: "on a shared list, whose favorites are suggested and matched: the owner's (today only the owner sees the favorite form) or the signed-in editor's own", to: owner }
    - { when: "whether non-favorite catalog products (e.g. ones created by recipes, MB-0014) also match, or only favorites", to: owner }
    - { when: "matching a favorite whose category was deleted, or a name matching several products", to: owner }
    - { when: "any change to list access checks or `begin_write` (RR-0002)", to: security }
  forbidden:
    - "query list tables outside `repo/sqlite/list.rs` or around the central predicate"
    - "schema changes"
    - "inline scripts"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
  - .agentic/mentorship/actix-web.md@v1
  - .agentic/mentorship/persistence.md@v1
resolutions: []
---
```

## Context

Requested by the owner (2026-11-09). `templates/lists/_list.hbs` has two
forms: "Item" (free text, `POST /lists/{id}/items`, `add_item`) for anyone
who can edit, and "Favorite" (a `<select>` grouped by category,
`POST /lists/{id}/favorites`, `add_favorite`) shown only to the owner.
Free text is stored uncategorized; a favorite brings its product link,
name and category (MB-0004). The two-step choice is slow on a phone.

MB-0013 built the same pattern for recipe ingredients: a `<datalist>` of
the user's product names outside the swapped fragment, and `name_key`
matching. MB-0013's non-goals named shopping-list items as a later reuse.

Owner decisions 2026-11-09 (answering the escalations): only favorites
match, not recipe-created products; a favorite whose category was deleted
follows `add_product` (linked, uncategorized). On a shared list categories
are always the list owner's: only the owner sets them, and editors see the
owner's categories on items ("Only owner can set the category and editor
sees owners categories for the items"). So the owner's typed text matches
the owner's favorites, and an `edit` grantee's text stays a free-form item
with no suggestions, as today; editors' favorite matching is deferred to
CRP-0024 (it would need `add_product` at `edit`, an RR-0002 change).
Acceptance ids are unchanged; P4 checks this split.

## Conceptual plan

1. **Clarify (S1).** Owner questions above (shared lists, which products
   match, edge cases); no RR needed if the answers stay inside existing
   rules, otherwise a CRP.
2. Resolve in the handler (or repo): look up the text's `name_key` among
   the user's favorites; call `add_product` or `add_item`.
3. Template: one form, datalist; remove or keep the old route per P1.

## Validation plan

- P1–P4: route tests; P5: the existing guard.
- Smoke: on a phone-width browser, type part of a favorite, pick it, see
  it under its category; type something new, see it uncategorized.
