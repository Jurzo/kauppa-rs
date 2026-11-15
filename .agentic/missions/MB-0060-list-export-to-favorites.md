# Mission Brief — Export a list's items to your favorites, with a review of category conflicts

```yaml
---
id: MB-0060
owner: "@juuso-kalliomaki"
goal: "From a shopping list, a user can add its items to their own favorites in one action; items that already match a favorite by name but carry a different category are shown on a review step, where the user decides each conflict before anything is saved."
non_goals:
  - "Changing the list: exporting never edits, removes or reorders its items."
  - "Exporting recipes, or importing from another user (MB-0061 reuses this mission's review step)."
  - "Merging two favorites, or renaming a favorite during export."
  - "JavaScript."
acceptance:
  - id: P1
    property: "A list page offers 'Add to favorites' (export) to the users the RR allows. It covers the list's current items as the RR decides (all, or a chosen subset), and items removed from the list are never exported."
    check: cargo test -p kauppa-web routes::lists::tests::export::offered_to_allowed_roles
  - id: P2
    property: "An item whose name matches none of the user's products (case-insensitively, RR-0003) becomes a favorite in the item's category; an item matching a favorite with the same category changes nothing; an item matching a product from recipes (not a favorite) with the same category makes it a favorite."
    check: cargo test -p kauppa-web routes::lists::tests::export::adds_new_and_skips_identical
  - id: P3
    property: "When any item matches one of the user's products by name but with a different category, nothing is saved yet: a review step lists each conflict (item name, the list's category, the favorite's category) and asks, per conflict, which category to keep, or to skip it; confirming applies every decision and every non-conflicting item together, in one transaction."
    check: cargo test -p kauppa-web routes::lists::tests::export::conflicts_need_review
  - id: P4
    property: "If the user's favorites or the list change between the review and the confirmation so that a decision no longer applies, the confirmation saves nothing and shows the review again with the current conflicts (no stale decision is applied)."
    check: cargo test -p kauppa-web routes::lists::tests::export::stale_review_is_redone
  - id: P5
    property: "Items without a category are handled as the RR decides (skipped and reported, or a category chosen on the review step); after export the user sees a summary: how many added, already present, recategorized, skipped."
    check: cargo test -p kauppa-web routes::lists::tests::export::uncategorized_and_summary
  - id: P6
    property: "Export writes only the exporting user's own catalog; it is refused (as the list's other actions are) to anyone the RR doesn't allow; categories that belong to the list's owner are never linked into another user's catalog (a grantee's export maps them as the RR decides); every list query goes through the central access predicate."
    check: cargo test -p kauppa-web routes::lists::tests::export::writes_only_own_catalog
  - id: P7
    property: "Works without htmx (full pages, 303 after the final save) and with htmx; the cross-site check covers the new routes; every string in both catalogs; existing tests unchanged."
    check: cargo test -p kauppa-web --all-targets
  - id: P8
    property: "In a browser on a phone width: exporting a list with no conflicts, and one with two conflicts decided differently, gives the expected favorites."
    check: "manual: browser check on the release binary, recorded in MB-0060's Continuity Pack"
context:
  - templates/lists/show.hbs
  - templates/lists/_list.hbs
  - templates/products/_catalog.hbs
  - crates/kauppa-web/src/routes/lists/
  - crates/kauppa-web/src/routes/products/mod.rs
  - crates/kauppa-core/src/repo/sqlite/list.rs
  - crates/kauppa-core/src/repo/sqlite/list_access.rs
  - crates/kauppa-core/src/repo/sqlite/product.rs
  - crates/kauppa-core/src/repo/sqlite/category.rs
  - migrations/0003_products.sql
  - migrations/0006_list_item_products.sql
  - .agentic/resolutions/RR-0001-product-category-data-model.md
  - .agentic/resolutions/RR-0002-list-sharing-authz.md
  - .agentic/resolutions/RR-0003-case-insensitive-names-amendment.md
  - .agentic/resolutions/RR-0024-editor-favorite-matching.md
  - .agentic/mentorship/security.md
  - .agentic/mentorship/persistence.md
autonomy:
  may_decide:
    - "the review page's layout and wording, through catalog keys in both languages"
    - "test names inside `routes::lists::tests::export` (record renames here)"
  escalate:
    - { when: "who may export: the owner only, or also `edit` and `view` grantees into their own favorites (routes/lists/ is a trigger path, RR-0002, RR-0027)", to: security }
    - { when: "a grantee's export: the list's categories are the owner's; map them to the grantee's categories by name, create missing ones, or leave items uncategorized", to: owner }
    - { when: "which items: all current items, only unchecked/checked ones, or a chosen subset (checkboxes on a step before the review)", to: owner }
    - { when: "items without a category: skip and report, or require a choice on the review step (favorites need a category today)", to: owner }
    - { when: "conflict choices: 'keep my favorite's category' / 'use the list's category' / 'skip', and the default selection", to: owner }
    - { when: "how the review is carried between steps (a stateless form with item ids and a fingerprint of the state it was computed from, recommended; or server-side state) and its cross-site and tampering checks", to: security }
    - { when: "a new repo method for the transaction in `repo/sqlite/list.rs` or `product.rs` (list.rs is a trigger path)", to: security }
  forbidden:
    - "write to another user's catalog or categories"
    - "a query on the list tables outside the central access predicate"
    - "apply a conflict decision the user didn't make"
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

Requested by the owner (2026-11-15): "A functionality on a list to 'export'
the items to your favorites. If an item matches your existing favorite but
has a different category, a separate confirmation should be shown to the
user of the conflicts and a case-by-case confirmation should be made."

Data model (RR-0001, migrations 0003 and 0006):

- A user's catalog is `products` (`owner_id`, `name`, `category_id`,
  `is_favorite`): favorites have `is_favorite = 1`; products created from
  recipe ingredients are hidden (`is_favorite = 0`) and listed under 'From
  recipes'. Names are unique per owner, case-insensitively (RR-0003).
- A list item has `text`, `quantity`, and optionally `product_id` and
  `category_id`, both pointing at the list **owner's** catalog. A grantee's
  export therefore can't reuse those ids; that is the owner question about
  mapping categories.
- On `/favorites`, adding requires a category ("products.needs_category"),
  so an item without one can't become a favorite as-is.

The conflict review is the part MB-0061 (importing shared favorites and
recipes) reuses: "Conflicts resolution should be done the same as on 4."
Building it as a reusable step (input: candidate products with names and
categories; output: decisions) keeps the two missions consistent.

`routes/lists/` and `repo/sqlite/list.rs` are trigger paths (RR-0027), so
the route and any list-side repo change need a security RR.

## Conceptual plan

1. **Clarify (S1).** One CRP: roles and review-state handling (security);
   grantee categories, which items, uncategorized items and conflict
   choices (owner).
2. The core function: classify a list's items against a catalog (new,
   identical, promote, conflict, uncategorized), with unit tests.
3. The routes: start export, review, confirm; one transaction; route tests
   (P1–P7).
4. Templates; browser check (P8).

## Validation plan

- Unit tests for the classification.
- Route tests for each acceptance item, including a stale review.
- The owner's browser check.
