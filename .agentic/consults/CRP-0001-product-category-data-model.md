# Consultation Request Pack — Product/category data model

```yaml
---
id: CRP-0001
mission: MB-0001
owner: "@juuso-kalliomaki"
target_role: data-owner
risk_tier: medium
urgency: medium
state: answered
runbook_trigger: ".agentic/runbooks/feature.md escalation trigger `migrations/**` → data-owner; AGENTS.md do-not-touch (schema changes require an approved Resolution Record)"
---
```

## Decision statement

Adopt a product-centric data model — a per-user catalog of `products`, each
classified by a per-user `category`, with recipe ingredients and shopping-list
items referencing products — as the foundation for the whole feature set, in
place of the current free-text `ingredients`/`list_items` in `0001_init.sql`.

## Options and trade-offs

The product owner's brief (`.drafts/kauppalistasovellus.docx`) requires that a category
be chosen **for the product** when saving a favorite or a recipe ingredient, and
that the finished list group "according to the categories the user has formed
for the products." The question is where category and product identity live.

### Option A — Product-centric catalog (recommended)

A per-user `products` catalog; favorites are products (`is_favorite`); recipe
`ingredients` and `list_items` reference a product; categories are a per-user
taxonomy. `list_items` snapshot name/quantity/category at add-time (a shopping
list is a point-in-time artifact and must not reshuffle when the user later
edits their taxonomy).

- Pros: one definition of "a product" and its category; "choose a category for
  the product" and category-grouped lists fall out directly; no cross-table
  drift when a category is renamed; favorites, ingredients, and list items all
  reuse the same catalog.
- Cons: most up-front schema; introduces a join the scaffold doesn't have yet;
  requires migrating the free-text `ingredients`/`list_items` shape.
- Risk: medium — foundational; getting the keys wrong is costly to undo. But
  this is greenfield scaffold with **no production data**, so there is no
  data-migration hazard today.
- Cost / timeline: one migration (`0002`) + repo-trait additions; unblocks
  MB-0001..MB-0004 and MB-0008.

### Option B — Category as a denormalized attribute

No catalog. Add a `category_id` column directly onto favorites, `ingredients`,
and `list_items`; each row is independent text.

- Pros: smallest change; closest to the current scaffold; no joins.
- Cons: "the product" has no single identity — the same product is retyped and
  recategorized in three places; renaming a category or fixing a typo must be
  done everywhere; no natural "pick a saved favorite" source of truth.
- Risk: low to build, high to live with — taxonomy drift is a correctness
  problem the NFR list ("lost data is unacceptable", readable model) penalizes.
- Cost / timeline: one small migration; cheap now, pays interest later.

### Option C — Hybrid (catalog for favorites only)

A `products` catalog backs favorites; `ingredients` and `list_items` keep text +
`category_id` and only *optionally* link a product.

- Pros: less migration than A; favorites get a real home.
- Cons: two code paths for "a product on a list" (linked vs. free text) that
  every grouping/ordering query must handle; the doc treats a recipe ingredient
  as a product too, so this under-models the brief.
- Risk: medium — the optional link tends to stay empty and collapse toward B.
- Cost / timeline: between A and B; most of A's work without all of A's payoff.

## Recommendation and question

Recommended option: **A — product-centric catalog.**
Because: it is the only model under which "assign a category to the product" and
"group the list by the product's category" are the *same* fact stored once. It
matches the brief's own language (a recipe ingredient *is* a product with a
category) and the AGENTS.md goals of a correct, readable model. The extra schema
is paid once, up front, while the repo is still greenfield.

Decided product constraints already folded into the recommended shape (from the
MB-0004 open questions):

1. **Categories are per-user**, not per-list (`categories.owner_id`).
2. **Ad-hoc list items stay uncategorized until a category is provided** —
   `list_items.category_id` is nullable; an "Uncategorized / Muut" bucket renders
   last in the grouped view.
3. **Soft delete** — removing an item sets `list_items.removed_at` rather than
   deleting the row; hidden items are recoverable and excluded from the view.

Proposed schema (new append-only migration `0002_products_categories.sql`;
column/FK details are the implementing brief's to finalize):

```sql
CREATE TABLE categories (
    id         TEXT PRIMARY KEY,
    owner_id   TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    position   INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    UNIQUE (owner_id, name)
) STRICT;

CREATE TABLE products (
    id          TEXT PRIMARY KEY,
    owner_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    category_id TEXT REFERENCES categories(id) ON DELETE SET NULL, -- nullable in DB; app-required for favorites/ingredients
    position    INTEGER NOT NULL DEFAULT 0,                        -- position within category (NTH)
    is_favorite INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    UNIQUE (owner_id, name)
) STRICT;

-- new columns added to existing tables via 0002 (0001 stays untouched):
--   ingredients.product_id  TEXT REFERENCES products(id) ON DELETE SET NULL
--   list_items.product_id   TEXT REFERENCES products(id) ON DELETE SET NULL
--   list_items.category_id  TEXT REFERENCES categories(id) ON DELETE SET NULL  -- snapshot at add-time; NULL = uncategorized
--   list_items.removed_at   TEXT  -- RFC3339; NULL = active (soft delete)
```

Two sub-decisions resolved **inside** Option A, called out for your sign-off:

- **`category_id` is nullable in the DB**, with the "must choose a category when
  saving a favorite/ingredient" rule enforced at the handler layer (an MB-0002 /
  MB-0003 acceptance property). This makes category deletion safe (`SET NULL` →
  the product/item becomes uncategorized) and unifies with constraint #2 above.
- **`list_items` snapshot their category** (seeded from the product at add-time)
  rather than always joining live to the catalog, so an in-progress list does
  not reorder when the user edits their taxonomy mid-trip.

Crisp question to answer: **Do you approve Option A (per-user catalog + per-user
categories, nullable DB category with app-level enforcement, snapshotted list
items, soft delete) as the schema foundation, so MB-0001..MB-0004 can be
scaffolded against it?** If not, which option, and are the two sub-decisions
acceptable?

## Blast radius and rollback

What can break: nothing in production — pre-release scaffold, no live data. The
`0001_init.sql` `ingredients`/`list_items` shape gains columns via `0002`
(append-only; `0001` is never edited). Repo traits (`RecipeRepo`,
`ShoppingListRepo`, plus new `CategoryRepo`/`ProductRepo`) grow new methods.
Who is affected: only the developer; no users yet.
Rollback plan: if rejected before any `0002` lands, no action. If rejected after,
a compensating `0003` migration drops the added tables/columns (SQLite STRICT
supports `DROP TABLE`; column drops via table rebuild per the persistence pack).

## Minimal context

- Brief: `.drafts/kauppalistasovellus.docx` (Finnish) — Products, Grouping, Building a
  list, The list.
- Current schema: `migrations/0001_init.sql` — `ingredients` and `list_items`
  are free-text (`text`, `quantity`), no category, no product identity.
- Governance: `AGENTS.md` (schema changes need an approved RR; `migrations/` is
  append-only), `.agentic/mentorship/persistence.md` (STRICT tables, UUID/RFC3339
  as TEXT, `ON DELETE CASCADE` default — any divergence justified in the
  migration header; repo traits are the only web-facing surface).
- This decision governs the foundation set: MB-0001 (categories), MB-0002
  (products/favorites), MB-0003 (recipes), MB-0004 (assemble list); MB-0008
  (sharing) builds on it. `mission: MB-0001` above names the first brief it
  unblocks.

## Evidence bundle (Hardened)

- Data volume: single self-hosted user scale; per-user catalog is tens-to-low
  hundreds of products — the added join is trivially indexed
  (`categories(owner_id, position)`, `products(owner_id, category_id)`).
- No benchmarks required at this tier; p95 budget (AGENTS.md: <100 ms for
  single-user reads) is not threatened by one indexed join on SQLite.
- Repro / proof will be the migration + repo-trait tests delivered under
  MB-0001/MB-0002 once this is approved.
```
