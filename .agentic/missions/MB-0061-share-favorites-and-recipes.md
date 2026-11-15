# Mission Brief — Share your favorites and recipes so other users can import them

```yaml
---
id: MB-0061
owner: "@juuso-kalliomaki"
goal: "A user can share their favorites and their recipes with another user of the app, who can browse what was shared and import copies of chosen favorites and recipes into their own catalog, resolving category conflicts case by case exactly as list export does (MB-0060)."
non_goals:
  - "Live, shared editing of a favorite or recipe: an import is a copy; later changes on either side don't propagate."
  - "Public links or sharing with people who have no account (registration stays invite-only, RR-0012)."
  - "Sharing categories on their own, or shopping lists (lists already have sharing, RR-0002)."
  - "Comments, ratings or a public recipe directory."
  - "JavaScript."
acceptance:
  - id: P1
    property: "A user can share with another existing user, as the RR decides (their favorites, their recipes, or both; all of them or chosen ones), see who they share with, and revoke a share; revoking stops further imports and never removes copies already imported."
    check: cargo test -p kauppa-web routes::sharing::tests::grants::share_and_revoke
  - id: P2
    property: "A recipient sees what has been shared with them (by whom, what) and nothing that wasn't; every read of another user's favorites or recipes goes through one access predicate, and a revoked or never-made share answers like a missing page (404), as list sharing does."
    check: cargo test -p kauppa-web routes::sharing::tests::access::only_shared_content
  - id: P3
    property: "Importing favorites adds them to the recipient's catalog with the same rules as MB-0060 (new, identical, promote, conflict, uncategorized), using the same review step for conflicts; categories are mapped as the RR decides (the sharer's category ids never enter the recipient's catalog)."
    check: cargo test -p kauppa-web routes::sharing::tests::import::favorites_use_export_review
  - id: P4
    property: "Importing a recipe creates a copy owned by the recipient (title, instructions, servings, ingredients with quantities and order); each ingredient's product is matched or created in the recipient's catalog by the same rules and the same review, in one transaction with the recipe."
    check: cargo test -p kauppa-web routes::sharing::tests::import::recipe_copy_with_products
  - id: P5
    property: "Importing the same recipe twice does what the RR decides (refused, a second copy, or an update offer) and never duplicates silently; a stale review is redone as in MB-0060 P4."
    check: cargo test -p kauppa-web routes::sharing::tests::import::repeat_and_stale
  - id: P6
    property: "Sharing doesn't reveal whether an email has an account beyond what list sharing already reveals (RR-0002, RR-0012); the sharer's email is shown to the recipient only as the RR decides; the share routes are rate-limited or not as the RR decides."
    check: cargo test -p kauppa-web routes::sharing::tests::grants::no_new_account_disclosure
  - id: P7
    property: "Schema changes are a new migration only (append-only), with the RR; deleting a user, a recipe or a favorite removes shares that point at them and leaves recipients' imported copies intact."
    check: cargo test -p kauppa-core repo::sqlite::tests
  - id: P8
    property: "Works without htmx and with htmx; the cross-site check covers every new route; every string in both catalogs; existing tests unchanged."
    check: cargo test -p kauppa-web --all-targets
  - id: P9
    property: "Two accounts in a browser: share, browse, import favorites with a conflict, import a recipe, revoke."
    check: "manual: browser check on the release binary, recorded in MB-0061's Continuity Pack"
context:
  - templates/products/
  - templates/recipes/
  - templates/lists/sharing.hbs
  - crates/kauppa-web/src/routes/products/mod.rs
  - crates/kauppa-web/src/routes/recipes/
  - crates/kauppa-web/src/routes/lists/shares.rs
  - crates/kauppa-core/src/repo/sqlite/product.rs
  - crates/kauppa-core/src/repo/sqlite/recipe.rs
  - crates/kauppa-core/src/repo/sqlite/list_access.rs
  - migrations/
  - .agentic/missions/MB-0060-list-export-to-favorites.md
  - .agentic/resolutions/RR-0001-product-category-data-model.md
  - .agentic/resolutions/RR-0002-list-sharing-authz.md
  - .agentic/resolutions/RR-0012-invite-only-registration.md
  - .agentic/resolutions/RR-0036-code-cleanup-after-audit.md
  - .agentic/mentorship/security.md
  - .agentic/mentorship/persistence.md
autonomy:
  may_decide:
    - "the new route module's name (`routes/sharing/` is assumed above; record a rename here) and test names inside it"
    - "page layout and wording, through catalog keys in both languages"
  escalate:
    - { when: "the sharing model: a standing grant per recipient (like list shares) for 'all my favorites' / 'all my recipes', per-item shares, or both; whether favorites and recipes are shared separately", to: owner }
    - { when: "the schema: new tables (for example `catalog_shares`), their cascades, and a new access predicate for cross-user reads of products and recipes (migrations/ is a trigger path)", to: data-owner }
    - { when: "authorization and disclosure: the predicate, 404 vs 403, what the recipient sees of the sharer (name, email), account enumeration through the share form, rate limits", to: security }
    - { when: "category mapping on import: by name to the recipient's categories, create missing ones, or leave uncategorized / ask on the review step", to: owner }
    - { when: "importing a recipe again: refuse, second copy, or offer to update the earlier copy", to: owner }
    - { when: "whether a recipe's ingredient products become favorites or stay hidden ('From recipes') in the recipient's catalog", to: owner }
    - { when: "splitting this mission: favorites first and recipes second, if S1 shows the recipe import is large", to: owner }
  forbidden:
    - "read another user's favorites or recipes outside the new access predicate"
    - "write to the sharer's catalog, or link the recipient's data to the sharer's category or product ids"
    - "edit a committed migration"
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

Requested by the owner (2026-11-15): "Ability to share your favorites and
recipes so that other users can import them. Conflicts resolution should be
done the same as on `4`." (`4` is MB-0060, list export to favorites.)

What exists:

- **Catalogs are private.** Products (favorites and hidden 'From recipes'
  products), categories and recipes are each owned by one user
  (`owner_id`); no route reads another user's.
- **List sharing** (MB-0008, RR-0002): `list_shares` grants an existing user
  (found by email) `view` or `edit` on one list, revocable; every list
  query goes through one predicate (`list_access.rs`, RR-0036). It is the
  pattern to copy for the new cross-user reads, with its own predicate.
- **Recipes** (`recipes`, `ingredients`): ingredients have text, quantity,
  position and an optional `product_id` into the owner's catalog
  (migration 0005). A copy for another user must re-link each ingredient
  to the recipient's catalog, which is where MB-0060's matching and
  conflict review apply.
- **Categories** are per user with case-insensitive names (migration 0004,
  RR-0003), so a sharer's "Dairy" and a recipient's "dairy" are the same
  category by name but different rows.

This mission depends on MB-0060's review step and should start after it.
It needs a schema RR (new tables) and a security RR (cross-user reads).

## Conceptual plan

1. **Clarify (S1).** One CRP: the sharing model, category mapping, repeat
   imports and ingredient products (owner); schema (data); predicate and
   disclosure (security). Decide whether to split.
2. Migration, repo and access predicate, with core tests (P7, P2).
3. Share management pages (P1, P6).
4. Browse and import favorites through MB-0060's review (P3).
5. Recipe import (P4, P5).
6. Browser check (P9).

## Validation plan

- Core tests for the predicate and cascades.
- Route tests for each acceptance item, including revoked and never-made
  shares.
- The owner's two-account browser check.
