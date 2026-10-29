# Resolution Record — Product/category data model

```yaml
---
id: RR-0001
closes: CRP-0001
mission: MB-0001
decided_by: "@juuso-kalliomaki"
role: data-owner
risk_tier: medium
decided_at: 2026-10-28T00:00:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/mentorship/persistence.md
  - .agentic/runbooks/migration.md
---
```

## Decision and outcome

Approved option A.

## Rationale and trade-offs

Avoids data duplication and cost of change is still low.

## Approvals and authority

Approved by @juuso.kalliomaki

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0001-product-category-data-model.md`
- Mission Brief:
  - `.agentic/missions/MB-0001-categories.md`
  - `.agentic/missions/MB-0002-products-favorites.md`
  - `.agentic/missions/MB-0003-recipe-authoring.md`
  - `.agentic/missions/MB-0004-assemble-list.md`
- Migrations under this decision (one append-only file per Mission Brief, as
  the briefs split it; CRP-0001 had sketched a single
  `0002_products_categories.sql`):
  - `migrations/0002_categories.sql` — MB-0001 (merged)
  - `migrations/0003_products.sql` — MB-0002 (merged)
  - `migrations/0004_ingredient_product.sql` — MB-0003 (planned)
  - `migrations/0005_list_item_products.sql` — MB-0004 (planned)
- Evidence links:
  - MB-0001: PR #2 (merged into `agentic/scaffold-mission-briefs`; reached
    `main` via PR #4)
  - MB-0002: PR #4

## Constraints and follow-on rules (Hardened)

1. **One product, one row.** A product exists once per user, in `products`.
   Favorites are products with `is_favorite = 1`. Recipe ingredients (MB-0003)
   and list items (MB-0004) reference `products(id)` instead of retyping the
   product. *(CRP-0001 Option A)*
2. **Categories are per user** (`categories.owner_id`), never per list.
   *(CRP-0001 constraint 1)*
3. **`category_id` stays nullable in the schema** on every table that has one.
   "A favorite or recipe ingredient must have a category" is enforced at the
   handler layer. Do not add `NOT NULL`. *(CRP-0001 sub-decision 1)*
4. **Deleting catalog or taxonomy rows never deletes user data.** References
   to `categories(id)` and `products(id)` use `ON DELETE SET NULL`; the rows
   that pointed at them become uncategorized or unlinked. Owner references
   (`owner_id → users(id)`) keep the default `CASCADE`. *(CRP-0001 schema)*
5. **Uncategorized is a real state.** Ad-hoc list items stay uncategorized
   until a category is chosen, and uncategorized groups always render last.
   *(CRP-0001 constraint 2)*
6. **List items snapshot at add-time.** Name, quantity, and category are copied
   when an item is added; later catalog edits don't change existing items.
   *(CRP-0001 sub-decision 2)*
7. **Removing a list item is a soft delete** (`removed_at`). Default reads hide
   removed items, and a removed item can be restored. *(CRP-0001 constraint 3)*
8. **Schema history.** `0001_init.sql` is never edited. Each brief adds exactly
   the migration listed under *Linked artifacts*. A brief that needs a
   migration not listed there amends this RR through a new consult first.
   *(Proposed: the migration runbook plus the per-brief split.)*
9. **Same-owner references are an application invariant.** The foreign keys
   only check that the referenced row exists, not that it belongs to the same
   user — `products.category_id` would accept another user's category. Every
   write that sets a `category_id` or `product_id` verifies ownership in the
   same statement or transaction, and cross-table reads also join on
   `owner_id`. *(Proposed: what MB-0002 does in `ProductRepo::create`,
   `recategorize`, and `list_for_owner`.)*

Open questions for the resolver:

- **What does the snapshot freeze?** MB-0004 lets a list item store just
  `category_id`. In that case only the item's category *membership* is frozen:
  renaming or reordering a category still shows up live on an in-progress
  list. CRP-0001's wording ("must not reshuffle when the user later edits their
  taxonomy") could be read as freezing the group order too. Settle this
  before MB-0004.
  - Only *membership* shoudl be frozen.
- **Name uniqueness is case-sensitive.** `UNIQUE(owner_id, name)` uses SQLite's
  binary collation, so "Maito" and "maito" can coexist. Keep that, or match
  case-insensitively (a schema or app change)?
  - Names should match case-insensitively.

## Roll-back into the system (Hardened)

- `.agentic/mentorship/persistence.md`
  - *Schema:* after "Foreign keys use `ON DELETE CASCADE` by default", add the
    RR-0001 exception: references to catalog and taxonomy rows (`category_id`,
    `product_id`) use `ON DELETE SET NULL` and stay nullable; requiredness is
    a handler rule (constraints 3–4).
  - *Queries:* add the same-owner rule (constraint 9). Foreign keys don't prove
    ownership, so verify it in the writing statement
    (`INSERT … SELECT … WHERE owner_id = ?`) or transaction, and join on
    `owner_id` across tables.
  - *Queries:* add `CategoryRepo` and `ProductRepo` to the repository traits
    the web layer may use.
  - *New "Point-in-time data" note:* list items copy product fields at
    add-time and are soft-deleted (constraints 6–7); default reads filter on
    `removed_at IS NULL`.
- `.agentic/runbooks/migration.md`
  - *Entry gate, item 3:* when one RR covers several briefs, it lists each
    brief's migration filename. Creating a migration that isn't listed needs
    an RR amendment through a consult first (constraint 8).
  - *S2 — Author:* a foreign key that diverges from `CASCADE` names the RR that
    allows it in the migration's comment header, as `0003_products.sql` does.

## Supersession and history

Status: active

- 2026-10-29: clerical update. Linked-artifact paths corrected, migration
  filenames listed (migration runbook entry gate, item 3), evidence links
  added. Decision unchanged.
