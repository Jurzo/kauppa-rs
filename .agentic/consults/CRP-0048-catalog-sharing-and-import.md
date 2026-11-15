# Consultation Request Pack — Share favorites and recipes for import: the model, the schema, access and disclosure, and how imports work

```yaml
---
id: CRP-0048
mission: MB-0061
owner: "@juuso-kalliomaki"
target_role: security
risk_tier: medium
urgency: medium
state: answered
runbook_trigger: "MB-0061 autonomy.escalate (sharing model, category mapping, repeat imports, ingredient products, split → owner; schema → data-owner; predicate and disclosure → security); feature.md escalation trigger `migrations/` (RR-0027); a new cross-user read path for products and recipes (RR-0002 constraint 1's pattern, RR-0036 constraint 3)"
---
```

## Decision statement

Decide how a user shares their favorites and recipes with another user,
what is stored, how the recipient's reads are authorized and what they
reveal, and how favorites and recipes are imported as copies through
MB-0060's review step.

## Options and trade-offs

### Q1 — The sharing model (owner)

#### Option A — A standing share per person, with two switches (recommended)

The sharer enters a person's email and ticks "favorites", "recipes" or
both. The recipient then sees the sharer's current favorites and recipes,
live, and imports the ones they choose (Q4). One share per pair of users;
editing it changes the switches; revoking it ends access at once.

- Pros:
  - mirrors list sharing (one row, revocable);
  - the sharer has nothing to curate;
  - the recipient picks what to copy.
- Cons: everything of a kind is visible, not chosen items.

#### Option B — Per-item shares

The sharer picks individual favorites and recipes.

- Cons: a row per item, more pages; and "share my recipes" with a friend
  becomes many clicks.

#### Option C — Both

- Cons: B's cost on top of A.

### Q2 — The schema (data-owner)

#### Option A — One table, two columns (recommended)

Migration `0012_catalog_shares.sql`:

```sql
CREATE TABLE catalog_shares (
    id         TEXT PRIMARY KEY,
    owner_id   TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    grantee_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    favorites  INTEGER NOT NULL CHECK (favorites IN (0, 1)),
    recipes    INTEGER NOT NULL CHECK (recipes IN (0, 1)),
    created_at TEXT NOT NULL,
    UNIQUE (owner_id, grantee_id),
    CHECK (owner_id <> grantee_id)
) STRICT;
CREATE INDEX idx_catalog_shares_grantee ON catalog_shares(grantee_id);

ALTER TABLE recipes ADD COLUMN imported_from TEXT
    REFERENCES recipes(id) ON DELETE SET NULL;
```

- **What deleting does (P7).**
  - Deleting either user deletes the share.
  - Deleting a favorite or recipe just stops showing it; no share points at
    one item.
  - Imported copies are the recipient's own rows and are never touched.
- **`imported_from`.** It records which recipe a copy came from, for Q7.
  It becomes NULL if the original is deleted.
- A share with both switches off is deleted rather than kept.
- Risk: additive only. An older binary ignores the table and the column.

#### Option B — A table per kind (`favorite_shares`, `recipe_shares`)

- Cons: two predicates and two forms for the same relationship.

### Q3 — Access and disclosure (security)

#### Option A — One predicate, list sharing's rules (recommended)

1. **The predicate.** A new `repo/sqlite/catalog_access.rs` holds the only
   query that decides whether user X may read user Y's favorites or
   recipes: a `catalog_shares` row with the matching switch on.
   - It returns a grant value that only it can make, as `ListGrant` does
     (RR-0036 constraint 2).
   - Every cross-user read of products, categories' names and recipes
     takes that grant.
   - `catalog_shares` is named only in that file and the share methods'
     file.
   - The bypass guard is widened to cover it, which RR-0036 constraint 3
     allows only by an RR; this one.
2. **No share means 404.** A missing, revoked or never-made share, or a
   switch that is off, answers like a missing page, as list sharing does
   (RR-0002).
3. **What the recipient sees of the sharer.** Their display name only,
   never their email (list grantees see the owner's display name only).
4. **The share form.** It gives the same notice whether or not the email
   matches an account (RR-0002 constraint 3). The email is matched
   case-insensitively to exactly one other account (RR-0028). The sharer's
   own list of shares shows the matched people by name and email, as a
   list's sharing page does; that reveals no more than list sharing
   already does.
5. **No rate limit**, as for list sharing. The form is behind sign-in, and
   it reveals nothing new.
6. **Logging.** Shares and revokes are logged at INFO with ids, never
   emails (RR-0002 constraint 5).

- Risk: the first cross-user read of private catalogs. The single
  predicate plus the guard is the control.

#### Option B — Show the sharer's email to the recipient too

- Cons: discloses an email the sharer may not have meant to give out.

### Q4 — Which favorites are imported (owner)

#### Option A — Choose with checkboxes, or import all (recommended)

The shared favorites are shown grouped by the sharer's categories, with a
checkbox each, none ticked. "Import selected" imports the ticked ones;
"Import all" imports everything. Either way the result goes through
MB-0060's plan and review.

#### Option B — Import all only

- Cons: a friend's 200 favorites when you wanted five.

### Q5 — Category mapping on import (owner)

#### Option A — As MB-0060 (recommended)

The sharer's category is matched by name, ignoring case, to the
recipient's own. With no match, the review step asks: one of the
recipient's categories, or skip (RR-0047 sub-answers 2 and 4). The
sharer's category ids never enter the recipient's catalog.

#### Option B — Create the missing categories automatically

- Cons: a catalog import can add many categories at once.

#### Option C — As A, and the review's picker also offers "New category: ‹their name›"

The category is created only if that choice is made.

- Pros: one click per unmatched category.
- Cons: a new kind of choice in the shared review step, which MB-0060's
  page would gain too.

### Q6 — A recipe's ingredients in the recipient's catalog (owner)

Today an ingredient links to a product of its owner's (RR-0001
constraint 1). A product created for a recipe is hidden ("From recipes"),
and an ingredient needs a categorized product (RR-0001 constraint 3,
unchanged by RR-0046).

#### Option A — Link or create, as the recipe editor does (recommended)

- **An existing product** in the recipient's catalog, matched by name
  ignoring case, is linked as it is. Its category and favorite flag are
  unchanged and there is no conflict review, because a recipe doesn't
  decide a product's category.
- **If that product has no category,** the review asks for one (a picker,
  no skip), since the ingredient needs one.
- **A new product** is created hidden ("From recipes"). Its category is
  the sharer's, mapped as in Q5. With no match, the review asks for one (a
  picker, no skip).
- **The copy itself.** Title, instructions, servings, and the ingredients
  with their quantities and order, are created with the products in one
  transaction.

#### Option B — Ingredient products go through the favorites' conflict review

- Cons: questions about categories the recipe doesn't use.

#### Option C — New ingredient products become favorites

- Cons: the recipient's favorites fill up with every ingredient of every
  imported recipe.

### Q7 — Importing a recipe again (owner)

#### Option A — Refuse, with a link to the copy (recommended)

If the recipient already has a recipe with `imported_from` = this recipe,
the import is refused with "You already have this recipe" and a link to
it. If they deleted their copy, importing again is allowed. Recipe titles
aren't unique, so the title isn't the test.

#### Option B — A second copy, the title suffixed "(2)"

- Cons: duplicates on purpose.

#### Option C — Offer to update the earlier copy

- Cons: overwrites the recipient's own edits. It needs a merge design.

### Q8 — Splitting the mission (owner)

#### Option A — One mission, two implementation PRs (recommended)

1. The share, the predicate, browsing, and importing favorites (P1–P3, P6,
   P7).
2. Importing recipes (P4, P5).

The schema (Q2) lands with the first PR.

#### Option B — Split recipes into a new brief

- Cons: a second brief and consult for the same share.

## Recommendation and question

Recommended: Q1 A, Q2 A, Q3 A, Q4 A, Q5 A, Q6 A, Q7 A, Q8 A.
Because:
- one revocable share per person, like lists;
- one predicate for every cross-user read;
- imports are copies decided in MB-0060's review;
- recipes link to the recipient's own products without surprises;
- repeats are caught by origin, not by title.

Crisp questions to answer:
1. (owner) A standing share per person with two switches (A), per-item
   (B), or both (C)?
2. (data-owner) `catalog_shares` and `recipes.imported_from` in
   `0012_catalog_shares.sql`, as written?
3. (security) One predicate in `catalog_access.rs` with the guard widened;
   404 without a share; the recipient sees the display name only; the
   share form's notice as for lists; no rate limit?
4. (owner) Import chosen favorites with checkboxes or all (A), or all only
   (B)?
5. (owner) Categories: as MB-0060 (A), create the missing ones (B), or
   offer "New category" on the review (C)?
6. (owner) Ingredients: link existing products as they are, create new
   ones hidden, ask only for missing categories (A)?
7. (owner) A repeat recipe import: refused with a link (A), a second copy
   (B), or an update offer (C)?
8. (owner) One mission, two PRs (A), or a new brief for recipes (B)?

## Blast radius and rollback

What can break:
- privacy of catalogs, through the new cross-user reads;
- the recipient's catalog, through imports.

Who is affected: users who share and their recipients.

Rollback plan:
- Revert the implementation PRs. The migration is additive, so an older
  binary ignores the table and the column.
- Imported copies stay. They are the recipient's own data.

## Minimal context

- **Today catalogs are private.** `products`, `categories` and `recipes`
  are each read only with the caller's own `owner_id`.
- **List sharing (RR-0002) is the pattern.**
  - `list_shares` holds the grants.
  - One predicate (`ACCESS_SELECT`) decides access, and only it can make a
    `ListGrant` (RR-0036 constraint 2).
  - A share answers the same whether or not the email matches.
  - A missing share is a 404.
  - A guard test confines the list tables to two files.
- **Recipes.** `recipes (id, owner_id, title, instructions, servings, …)`
  and `ingredients (id, recipe_id, text, quantity, position, product_id)`.
  `RecipeRepo::add_ingredient` matches the owner's product by name or
  creates a hidden one in a given category. Titles aren't unique.
- **MB-0060's review.** `kauppa-core/src/export.rs` plans by name against
  the user's catalog and maps foreign categories by name. Its review and
  fingerprint are reused (RR-0047 constraint 5); its input generalizes
  from list items to candidates (a name and the sharer's category).

## Evidence bundle (Hardened)

- **Constraints checked:**
  - RR-0001 constraints 1, 3 (as amended by RR-0046 for favorites only),
    4 and 9;
  - RR-0002 constraints 1, 3 and 5;
  - RR-0003 constraint 3;
  - RR-0012 (invite-only; no new account disclosure);
  - RR-0013 (limits only on sign-in and registration);
  - RR-0028 (email matching `NOCASE`);
  - RR-0036 constraints 2 and 3;
  - RR-0047 constraint 5.
- **Code:**
  - `crates/kauppa-core/src/repo/sqlite/list_access.rs`;
  - `crates/kauppa-core/src/repo/share.rs` (the bypass guard);
  - `crates/kauppa-web/src/routes/lists/shares.rs`;
  - `crates/kauppa-core/src/repo/mod.rs` (`RecipeRepo::add_ingredient`);
  - `migrations/` ends at `0011_list_archive.sql`.

## Owner's answer (2026-11-15)

As recommended: Q1 A, Q2 A, Q3 A, Q4 A, Q5 A, Q6 A, Q7 A, Q8 A.

Recorded in RR-0048 (scaffold; the owner signs).
