# Mission Brief — One Save button when editing a list item

```yaml
---
id: MB-0052
owner: "@juuso-kalliomaki"
goal: "Editing an item on an open list has one form with one Save button that saves the category and the quantity together, instead of a separate Set button for the category and a Save button for the quantity."
non_goals:
  - "Changing who may change what: the category stays owner-only and the quantity stays editable by the owner and `edit` grantees (RR-0002, RR-0024)."
  - "Renaming items, or editing the item text."
  - "The same change for favorites' edit controls (rename and category are separate there) or recipe ingredients; a later brief if wanted."
  - "JavaScript, or saving on change without a button."
acceptance:
  - id: P1
    property: "An item's edit section has one form and one submit button (Save): for the owner it holds the category picker (when the owner has categories) and the quantity; for an `edit` grantee only the quantity. There is no Set button, and no other form inside the section except Remove."
    check: cargo test -p kauppa-web routes::lists::tests::item_edit::one_save_button
  - id: P2
    property: "One Save by the owner stores both fields in one request. A rejected field (an unknown or foreign category, for example) changes neither field, and the list shows the error as rejections do today."
    check: cargo test -p kauppa-web routes::lists::tests::item_edit::saves_category_and_quantity_together
  - id: P3
    property: "An `edit` grantee's Save changes only the quantity. A category field sent by a grantee never changes the item's category; the RR decides whether it is refused or ignored. A `view` grantee is refused as today."
    check: cargo test -p kauppa-web routes::lists::tests::item_edit::grantee_saves_quantity_only
  - id: P4
    property: "Works without htmx (303 back to the list, or the full page with the error) and with htmx (the list fragment swaps), like today's item forms; the cross-site check covers the new or changed route; existing tests change only where they posted to a route this mission changes."
    check: cargo test -p kauppa-web --all-targets
  - id: P5
    property: "In a browser on a phone width and a desktop width, the edit section shows one Save button, saving a category and a quantity together works for the owner, and a grantee sees only the quantity."
    check: "manual: browser check on the release binary, recorded in MB-0052's Continuity Pack"
context:
  - templates/lists/_list.hbs
  - crates/kauppa-web/src/routes/lists/items.rs
  - crates/kauppa-web/src/routes/lists/mod.rs
  - crates/kauppa-web/src/routes/mod.rs
  - crates/kauppa-core/src/repo/mod.rs
  - crates/kauppa-core/src/repo/sqlite/list.rs
  - crates/kauppa-web/src/routes/lists/tests/
  - crates/kauppa-web/src/csrf/tests/coverage.rs
  - .agentic/resolutions/RR-0002-list-sharing-authz.md
  - .agentic/resolutions/RR-0024-editor-favorite-matching.md
  - .agentic/mentorship/security.md
autonomy:
  may_decide:
    - "test names inside `routes::lists::tests::item_edit` (record a rename here)"
    - "the form's layout and labels, through catalog keys in both languages"
  escalate:
    - { when: "the route: one new `POST /lists/{id}/items/{item}` (or similar) replacing the category and quantity routes, or keeping the old routes; `routes/lists/` is a trigger path (RR-0027)", to: security }
    - { when: "atomicity: a new repo method that sets both fields in one transaction behind the central access predicate (`repo/sqlite/list.rs` is a trigger path, RR-0002), or two calls", to: security }
    - { when: "a grantee's request that carries a category: refuse it (403/400) or ignore the field", to: security }
    - { when: "what Save does with a field the user didn't touch (e.g. an uncategorized item saved with only a quantity)", to: owner }
  forbidden:
    - "let an `edit` grantee change a category"
    - "a query on the list tables outside the central access predicate"
    - "JavaScript or inline scripts"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
  - .agentic/mentorship/security.md@v1
  - .agentic/mentorship/persistence.md@v1
resolutions: []
---
```

## Context

Requested by the owner (2026-11-12): "Editing an item in a list should not
have separate save functions 'set' and 'save' for category and quantity. A
single save button should suffice."

Today an item's `<details class="product-edit">` in `templates/lists/_list.hbs`
holds three forms:

- **Category** (owner only, when the owner has categories): the picker and a
  **Set** button, posting to `POST /lists/{id}/items/{item}/category`
  (`assign_category`, owner-only in the repo).
- **Quantity**: a text field and a **Save** button, posting to
  `POST /lists/{id}/items/{item}/quantity` (`set_item_quantity`, owner or
  `edit` grantee).
- **Remove**: a link-style button.

Changing both means two submissions, and the second form's swap reloads the
list, so a category picked but not Set is lost when the quantity is saved.

The two writes have different permissions (RR-0002: the category is the
owner's catalog; RR-0024 kept categories owner-only for editors), so a
combined save must keep that split. Both routes and the list repo are trigger
paths since RR-0027, so the change needs an RR.

## Conceptual plan

1. **Clarify (S1).** CRP to security (route, atomicity, grantee requests)
   with the owner question on untouched fields.
2. Tests first (P1–P3), then the repo method if the RR wants one
   transaction, the route, and the template.
3. Remove or keep the old routes as the RR decides; update the tests that
   post to them.
4. Browser check (P5).

## Validation plan

- P1: template test over the rendered list for the owner and a grantee.
- P2, P3: route tests with both fields, a rejected category, and grantee
  requests with and without a category.
- P4: the full suite, including the cross-site coverage test.
- P5: the owner's browser check.
