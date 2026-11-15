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
    property: "A list page offers 'Add to favorites' (export) to everyone who can open the list (the owner, and `edit` and `view` grantees), archived lists included; it covers every active item, checked or not, and items removed from the list are never exported; the same name more than once makes one favorite (RR-0047)."
    check: cargo test -p kauppa-web routes::lists::tests::export::offered_to_allowed_roles
  - id: P2
    property: "An item whose name matches none of the user's products (case-insensitively, RR-0003) becomes a favorite in the item's category; an item matching a favorite with the same category changes nothing; an item matching a product from recipes (not a favorite) with the same category makes it a favorite."
    check: cargo test -p kauppa-web routes::lists::tests::export::adds_new_and_skips_identical
  - id: P3
    property: "When any item matches one of the user's products by name but with a different category, nothing is saved yet: a review step lists each conflict (item name, the list's category, the product's category) with explicit choices, none preselected — a favorite: 'Keep ‹its category›' or 'Use ‹the list's category›'; a product from recipes: make it a favorite in its category or the list's, or 'Skip'; copies of one name with different categories: which to use, or skip — and every row must be answered (a missing or unknown answer saves nothing); confirming applies every decision and every non-conflicting item together, in one transaction (RR-0047)."
    check: cargo test -p kauppa-web routes::lists::tests::export::conflicts_need_review
  - id: P4
    property: "The review is a stateless form carrying the answers and a SHA-256 fingerprint of the plan it was computed from; the confirmation recomputes the plan inside the write transaction, and if the user's catalog or the list changed (the fingerprint differs) it saves nothing and shows the review again with the current rows; answers are accepted only for rows in the plan and only with category ids the user owns (RR-0047)."
    check: cargo test -p kauppa-web routes::lists::tests::export::stale_review_is_redone
  - id: P5
    property: "Items without a category (for a grantee, also items whose category name the grantee doesn't have) are listed on the review step with a picker of the user's categories and 'Skip', and each needs a choice; after export the user sees a summary: how many added, already present, made favorite, recategorized, skipped (RR-0047)."
    check: cargo test -p kauppa-web routes::lists::tests::export::uncategorized_and_summary
  - id: P6
    property: "Export writes only the exporting user's own catalog, through a new `ProductRepo::apply_export` (one `BEGIN IMMEDIATE` transaction on `products` only); a stranger gets 404 as for the list's other actions; the list owner's categories are never linked into another user's catalog: a grantee's export maps them by name (`name_key`) to the grantee's own categories, else the item is uncategorized for them, and no category is created; the list is read only through `ShoppingListRepo::items` (the central access predicate, at `view`), and `repo/sqlite/list.rs` doesn't change (RR-0047)."
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
    - { when: "who may export: the owner only, or also `edit` and `view` grantees into their own favorites (routes/lists/ is a trigger path, RR-0002, RR-0027) (decided: RR-0047, any reader into their own favorites)", to: security }
    - { when: "a grantee's export: the list's categories are the owner's; map them to the grantee's categories by name, create missing ones, or leave items uncategorized (decided: RR-0047, by name, else uncategorized)", to: owner }
    - { when: "which items: all current items, only unchecked/checked ones, or a chosen subset (checkboxes on a step before the review) (decided: RR-0047, every active item)", to: owner }
    - { when: "items without a category: skip and report, or require a choice on the review step (favorites need a category today) (decided: RR-0047, a choice on the review step)", to: owner }
    - { when: "conflict choices: 'keep my favorite's category' / 'use the list's category' / 'skip', and the default selection (decided: RR-0047, explicit choices, none preselected)", to: owner }
    - { when: "how the review is carried between steps (a stateless form with item ids and a fingerprint of the state it was computed from, recommended; or server-side state) and its cross-site and tampering checks (decided: RR-0047, stateless with a SHA-256 fingerprint)", to: security }
    - { when: "a new repo method for the transaction in `repo/sqlite/list.rs` or `product.rs` (list.rs is a trigger path) (decided: RR-0047, `ProductRepo::apply_export`)", to: security }
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
resolutions: [RR-0047]
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
   choices (owner). Done: CRP-0047 answered, RR-0047 approved (all as
   recommended).
2. The core function: classify a list's items against a catalog (new,
   identical, promote, conflict, uncategorized), with unit tests.
3. The routes: start export, review, confirm; one transaction; route tests
   (P1–P7).
4. Templates; browser check (P8).

## Validation plan

- Unit tests for the classification.
- Route tests for each acceptance item, including a stale review.
- The owner's browser check.
