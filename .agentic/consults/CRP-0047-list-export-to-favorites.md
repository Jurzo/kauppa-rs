# Consultation Request Pack — Export a list's items to favorites: who may export, which items, categories, conflict choices, and the review's state

```yaml
---
id: CRP-0047
mission: MB-0060
owner: "@juuso-kalliomaki"
target_role: security
risk_tier: medium
urgency: medium
state: open
runbook_trigger: "MB-0060 autonomy.escalate (who may export, the review's state, where the write lives → security; grantee categories, which items, uncategorized items, conflict choices → owner); feature.md escalation trigger `crates/kauppa-web/src/routes/lists/` (RR-0027)"
---
```

## Decision statement

Decide who may add a list's items to their own favorites, which items and
categories that covers, how category conflicts are offered for review, and
how the review is carried safely from the review page to the confirmation.

## Options and trade-offs

### Q1 — Who may export (security)

#### Option A — Anyone who can open the list, into their own favorites (recommended)

The owner, and `edit` and `view` grantees. Each export writes only the
exporting user's catalog.

- Pros: a grantee can keep a friend's list as favorites. It reads nothing
  the list page doesn't already show them: item names and the categories'
  names.
- Risk: low. Writes stay in the user's own catalog, and the list is read
  through the access predicate at `view`. RR-0024 constraint 6 ("any other
  read of the owner's catalog at `edit` needs an RR") is met by this RR. No
  owner product or category id ever reaches the grantee's catalog (see Q2).

#### Option B — Owner only

- Cons: grantees, the most likely users of "save this list's items", can't.

### Q2 — A grantee's categories (owner)

The list's categories belong to its owner (RR-0001 constraints 2 and 9), so
they can't be linked into a grantee's catalog.

#### Option A — Map by name to the grantee's own categories (recommended)

The match ignores case (`name_key`, RR-0003). An item whose category name
the grantee doesn't have counts as uncategorized for them (Q4).

- Pros: no categories appear in the grantee's catalog by surprise.

#### Option B — As A, and create the missing categories

Missing categories are appended to the end of the grantee's category order.

- Cons: an export can add many categories at once.

#### Option C — Every item uncategorized for a grantee

- Cons: a grantee picks every category by hand.

### Q3 — Which items (owner)

Removed items are never exported (brief P1).

#### Option A — Every active item, checked or not (recommended)

One "Add to favorites" button, and nothing more unless there's something to
review.

#### Option B — Only unchecked items

- Cons: after shopping, everything is checked, and that is when a list is
  most likely to be saved.

#### Option C — A chosen subset

A step before the review shows the items with checkboxes, all ticked by
default.

- Pros: full control.
- Cons: one more page on every export.

The same name twice on a list makes one favorite (names are unique per user,
RR-0003). If the copies' categories differ, the review asks which to use,
or to skip it.

### Q4 — Items without a category (owner)

A new favorite needs a category (RR-0001 constraint 3 as amended by
RR-0046).

#### Option A — Choose on the review step (recommended)

Such an item is listed with a picker of the user's categories and "Skip".
Each needs a choice before saving.

- Pros: works for grantees too (Q2 A), who can't categorize the list's
  items.

#### Option B — Skip and report

The summary counts them, and the owner categorizes the items first.

- Cons: a grantee has no way to add them.

### Q5 — Conflict choices and the default (owner)

A conflict is an item whose name matches one of the user's products with a
different category.

#### Option A — Explicit choices, none preselected (recommended)

- **A favorite:** "Keep ‹its category›" or "Use ‹the list's category›".
- **A product from recipes (not a favorite):** "Make it a favorite in ‹its
  category›", "… in ‹the list's category›", or "Skip".

Every row must be answered. The radios are `required`, and the server
refuses a missing answer. This keeps to the brief's "never apply a decision
the user didn't make".

#### Option B — Preselect "Keep"

- Pros: one click for "change nothing".
- Cons: a decision the user may not have looked at.

### Q6 — Carrying the review (security)

#### Option A — A stateless form with a fingerprint (recommended)

- **What the review page sends.** One field per review row: the conflicting
  product's id, or the item's name, and the chosen category id or "skip".
  Plus a SHA-256 fingerprint of the whole plan the page was computed from:
  each item's name and category, and each matched product's id, name,
  category and favorite flag.
- **What the confirmation does.** It recomputes the plan inside the write
  transaction.
  - If the fingerprint differs, it saves nothing and shows the review again
    with the current conflicts (P4).
  - Otherwise it applies the answers. It accepts answers only for rows in
    that plan, and only category ids the user owns (RR-0001 constraint 9).
- **Hashing.** `sha2` is already a dependency of both crates, so no new
  package is needed.
- Pros:
  - no server-side state;
  - tampering can only change the user's own catalog, and only within the
    choices the plan offers;
  - the cross-site check covers the POSTs.

#### Option B — Server-side review state (a table or the session)

- Cons:
  - a migration, or a larger cookie;
  - state to expire;
  - it gains nothing over A, which recomputes anyway.

### Q7 — Where the write lives (security)

#### Option A — Read through the list repo, write through the product repo (recommended)

- **Reading.** The route reads the list's active items through the
  existing `ShoppingListRepo::items` (access predicate, `view`).
- **Planning.** A pure kauppa-core function classifies the items against
  the user's catalog as new, identical, promote, conflict or uncategorized,
  with unit tests.
- **Writing.** A new `ProductRepo::apply_export(user, plan, answers)` runs
  one `BEGIN IMMEDIATE` transaction on `products` only. Inside it, it
  re-reads the user's catalog for the fingerprint check (Q6) and writes.
- **No list-file changes.** No list table is written and
  `repo/sqlite/list.rs` doesn't change, so the bypass guard is untouched.
  The route under `routes/lists/` is still a trigger path, which is what
  this consult covers.
- Pros: the list side stays as it is, and the catalog write is atomic.
- Risk: the list can change between reading and writing. Only the list's
  part of the fingerprint catches that, from the confirmation's own read;
  the product transaction can't lock a list, and doesn't need to.

#### Option B — A new `ShoppingListRepo` method that writes products

- Cons: a list-file method writing another table; it widens a trigger path
  for no gain.

### Q8 — Archived lists (owner)

Export only reads the list, so it is offered on archived lists too
(recommended). RR-0045 refuses only item and recipe writes to the list.

## Recommendation and question

Recommended: Q1 A, Q2 A, Q3 A, Q4 A, Q5 A, Q6 A, Q7 A, Q8 as above.
Because:
- any reader of a list can keep its items, without touching anyone else's
  catalog;
- every decision is the user's own;
- the review is stateless and rechecked when confirmed;
- the list side of the code doesn't change.

Crisp questions to answer:
1. (security) May `edit` and `view` grantees export into their own
   favorites (A), or the owner only (B)?
2. (owner) A grantee's categories: match by name (A), also create the
   missing ones (B), or none (C)?
3. (owner) Every active item (A), unchecked only (B), or a chosen subset
   (C)?
4. (owner) Items without a category: choose on the review step (A), or
   skip and report (B)?
5. (owner) Conflict choices as listed, none preselected (A), or "Keep"
   preselected (B)?
6. (security) A stateless review with a SHA-256 fingerprint, rechecked in
   the write transaction (A)?
7. (security) Read through `ShoppingListRepo::items` and write through a
   new `ProductRepo::apply_export` (A)?
8. (owner) Export offered on archived lists too?

## Blast radius and rollback

What can break: users' favorites. An export adds favorites, promotes
products from recipes, and recategorizes favorites the user chose to.
Who is affected: anyone who exports; list owners and their other grantees
are never touched.
Rollback plan:
- Revert the implementation PR. There is no schema change.
- Favorites added by an export stay. They are ordinary favorites the user
  can edit or delete.

## Minimal context

- **Catalog.** A user's catalog is `products` (`owner_id`, `name`,
  `name_key`, `category_id`, `is_favorite`, `position`).
  - Names are unique per user, ignoring case (RR-0003).
  - `ProductRepo::create` already promotes a product from recipes of the
    same name instead of failing.
- **List items.** A list item has `text`, `quantity` and optionally
  `product_id` and `category_id`, both in the list owner's catalog
  (RR-0001 constraint 9). `ShoppingListRepo::items(user, list, false)`
  returns active items with the category's name, at `view`.
- **Quantities aren't exported.** Favorites have no quantity.
- **Reuse by MB-0061.** Importing shared favorites and recipes uses the
  same review step. The plan and review take candidate names and
  categories, not list items, so MB-0061 can feed them too.
- **The flow.**
  1. "Add to favorites" on the list page posts to `/lists/{id}/export`.
  2. With nothing to review, it saves and shows a summary (added, already
     there, made favorite, recategorized, skipped).
  3. Otherwise it shows the review page, whose form posts to
     `/lists/{id}/export/confirm`.
  4. The final save is a 303 to the summary without htmx.

## Evidence bundle (Hardened)

- **Constraints checked:**
  - RR-0001 constraints 2, 3 (as amended by RR-0046), 6 and 9;
  - RR-0002 constraint 1;
  - RR-0003 constraint 3;
  - RR-0024 constraint 6;
  - RR-0045 (archived lists refuse item and recipe writes only).
- **Code:**
  - `crates/kauppa-core/src/repo/mod.rs` (`ProductRepo::create`, which
    promotes; `ShoppingListRepo::items`);
  - `crates/kauppa-core/src/repo/sqlite/list_access.rs` (`ITEM_SELECT`
    joins the owner's categories).
- **`sha2`:** in `crates/kauppa-core/Cargo.toml` and
  `crates/kauppa-web/Cargo.toml`.

## Owner's answer (2026-11-15)

As recommended: Q1 A, Q2 A, Q3 A, Q4 A, Q5 A, Q6 A, Q7 A, and Q8 (export
offered on archived lists).

Recorded in RR-0047 (scaffold; the owner signs).
