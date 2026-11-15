# Mission Brief — One Save button when editing a favorite

```yaml
---
id: MB-0058
owner: "@juuso-kalliomaki"
goal: "Editing a favorite (or a product from recipes) has one form with one Save button that saves its name and category together, as MB-0052 did for list items, instead of a Rename button and a separate Change button."
non_goals:
  - "Deleting a product: Delete stays its own control with its confirmation."
  - "Reordering (the ↑/↓ buttons) or the add form."
  - "List items or recipe ingredients (MB-0052 did list items)."
  - "JavaScript, or saving on change without a button."
acceptance:
  - id: P1
    property: "A product's edit section on `/favorites` (favorites and the 'From recipes' section, which share `products/_product_edit.hbs`) has one form with the name, the category picker and one Save button; the picker lists the user's categories and a 'No category' choice and opens with the product's current choice selected (its category, or 'No category'); the only other control in the section is Delete (RR-0043)."
    check: cargo test -p kauppa-web routes::products::tests::edit::one_save_button
  - id: P2
    property: "One Save stores the name and the category in one request and one transaction. If either is rejected (a name another of the user's products already uses, case-insensitively; an empty name; an unknown or foreign category), neither changes and the catalog shows the error as rejections do today."
    check: cargo test -p kauppa-web routes::products::tests::edit::saves_name_and_category_together
  - id: P3
    property: "Save with 'No category' clears the category: a favorite then shows under Uncategorized, a recipe product stays uncategorized; Save with the current choice changes only the name; a request without the category field is rejected and changes nothing (RR-0043 constraint 2); a product's list items and recipe ingredients keep their link to it (as rename and recategorize do today)."
    check: cargo test -p kauppa-web routes::products::tests::edit::no_category_and_unchanged_fields
  - id: P4
    property: "Works without htmx (303 back to `/favorites`, or the full page with the error) and with htmx (`#favorites` swaps); only the owner can edit their products; the cross-site check covers the new or changed route; existing tests change only where they posted to a route this mission changes."
    check: cargo test -p kauppa-web --all-targets
  - id: P5
    property: "In a browser on a phone width and a desktop width, the edit section shows one Save button, and renaming and recategorizing in one Save works."
    check: "manual: browser check on the release binary, recorded in MB-0058's Continuity Pack"
context:
  - templates/products/_product_edit.hbs
  - templates/products/_catalog.hbs
  - crates/kauppa-web/src/routes/products/mod.rs
  - crates/kauppa-web/src/routes/products/tests/
  - crates/kauppa-core/src/repo/sqlite/product.rs
  - crates/kauppa-web/src/csrf/tests/coverage.rs
  - .agentic/missions/MB-0052-one-save-for-list-item-edit.md
  - .agentic/resolutions/RR-0033-one-save-for-list-item-edit.md
  - .agentic/resolutions/RR-0001-product-category-data-model.md
  - .agentic/resolutions/RR-0003-case-insensitive-names-amendment.md
autonomy:
  may_decide:
    - "the route: one `POST /favorites/{id}` (or similar) replacing `/rename` and `/category`, and removing the old routes with their tests updated (they are not trigger paths)"
    - "a repo method that updates both fields in one transaction"
    - "test names inside `routes::products::tests::edit` (record a rename here)"
    - "the form's layout and labels, through catalog keys in both languages"
  escalate:
    - { when: "a product from recipes that has no category: today Rename works without one and Change requires a pick (`parse_category` rejects an empty value); with one Save, may a rename leave it uncategorized, or must Save require a category (decided: RR-0043, an explicit 'No category' choice)", to: owner }
  forbidden:
    - "let a user change another user's product"
    - "JavaScript or inline scripts"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
  - .agentic/mentorship/persistence.md@v1
resolutions: [RR-0043, RR-0046]
---
```

## Context

Requested by the owner (2026-11-15): "Favorites list items should only have
a single save button, like was done for a list item before."

Today `templates/products/_product_edit.hbs` holds three forms inside the
`<details class="product-edit">`:

- **Name** with a **Rename** button → `POST /favorites/{id}/rename`.
- **Category** picker with a **Change** button →
  `POST /favorites/{id}/category`.
- **Delete** (a link-style button with a confirmation).

Changing both means two submissions, and the first one's swap re-renders
`#favorites`, losing a change made in the other form. MB-0052 (RR-0033)
solved the same problem for list items.

Unlike MB-0052, neither `routes/products/` nor `repo/sqlite/product.rs` is a
trigger path (`scripts/agentic/check_triggers.py`), and there is no second
role: only the owner edits their catalog. So the route and transaction are
within this brief's autonomy, and only the owner's product question needs a
consult.

RR-0043's "No category" amends RR-0001 constraint 3 through RR-0046: a
favorite needs a category when created or made a favorite; its owner may
clear it later.

Related: MB-0059 (collapsible categories) edits the same catalog template;
whichever lands second rebases.

## Conceptual plan

1. **Clarify (S1).** The owner question on uncategorized products. Done:
   CRP-0043 answered, RR-0043 approved (an explicit "No category" choice).
2. Tests first (P1–P3); the repo method; the route; the template.
3. Remove the old routes and update their tests.
4. Browser check (P5).

## Validation plan

- P1: template test over the rendered catalog.
- P2, P3: route tests with both fields, each rejection, unchanged fields.
- P4: the full suite, including the cross-site coverage test.
- P5: the owner's browser check.
