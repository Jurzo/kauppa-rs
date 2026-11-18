# Consultation Request Pack — Creating a missing category from the review: which flows, what the answer carries, placement and name, the transaction and the race

```yaml
---
id: CRP-0052
mission: MB-0065
owner: "@juuso-kalliomaki"
target_role: product
risk_tier: medium
urgency: medium
state: open
runbook_trigger: "MB-0065 autonomy.escalate (reversing 'no category is created', which flows, placement and spelling → owner; categories written in the import and export transactions, the name race → data-owner; what a recipient keeps of the source → security); RR-0048 constraint 5 (a change to the review's behaviour needs its own consult); RR-0047 sub-answer 2 and RR-0048 sub-answer 5 amended"
---
```

## Decision statement

May the shared review step (list export, favorites import, recipe import)
offer "Create ‹name›" for a source category the user doesn't have, and if
so: in which flows (Q1), how the answer is sent and checked (Q2), where the
new category goes and how it is spelled (Q3), how it is written and what a
category made meanwhile does (Q4), and what this reverses (Q5)?

## Current state

- `export::build` (`crates/kauppa-core/src/export.rs`) matches each source
  category by `name_key` to the user's own. With no match, the name is kept
  in `PlanRow::unmatched` and the review already shows it ("on the list in
  ‹name›", `export.on_list_unmatched`, and the import equivalents).
- Today a row with only unmatched names is `Review(Uncategorized)`: a
  `<select>` of the user's categories, plus "Skip" for favorites (an
  ingredient has no skip). A row whose name also matches one of the user's
  categories is planned without asking, and its unmatched names are
  ignored.
- Answers are a stateless form: `fingerprint` plus one value per review row
  (a category id or `skip`). The confirmation recomputes the plan inside
  `BEGIN IMMEDIATE`, compares fingerprints (which cover the user's
  categories, ids and names), and `decide` accepts only offered ids.
- `CategoryRepo::create` inserts at `MAX(position) + 1`; names are unique
  per owner by `idx_categories_owner_name_key` (RR-0003).
- RR-0047 sub-answer 2 and RR-0048 sub-answer 5: "No category is created".
  CRP-0048 Q5 C (this feature as an option) was rejected then, for adding a
  new kind of choice to the shared review.

## Q1 — Which flows (owner)

### Option A — All three: list export by a grantee, favorites import, recipe import (recommended)

The choice appears on every review row that has unmatched names, in every
flow that uses the review:

- a grantee's list export (the list owner's own export never has unmatched
  names: their categories match by id);
- a shared favorite;
- a recipe ingredient whose new product's category the user lacks, or whose
  existing product has no category and the source names one.

- Pros:
  - One rule in one place (`PlanRow::choices`), as RR-0048 constraint 5
    expects: the review behaves the same everywhere.
  - Covers the owner's request ("favorite import from MB-0060 and
    MB-0061").
- Cons:
  - A recipe import can now add categories to the user's catalog, for
    products that start hidden ("From recipes").
- Risk: low.

### Option B — Favorites import only

- Cons: the review would behave differently per flow, which RR-0048
  constraint 5 asked to avoid; a grantee exporting a list has the same
  problem.

### Option C — Favorites import and list export, not recipes

- Pros: no categories created for hidden products.
- Cons: a second rule in `choices`.

**Recommended: A.** No new review rows: only rows that are reviewed today
gain the choice, so the "added automatically" count is unchanged.

## Q2 — What the answer carries (security)

### Option A — An index into the row's unmatched names (recommended)

The option's value is `new:<n>`, the n-th of `row.unmatched` in the plan.
The confirmation reads the name from its own recomputed plan; the client
never sends a name.

- Pros:
  - No free text from the form reaches a write; an index out of range, or
    `new:` on a row without unmatched names, is a tampered answer (as an
    unknown id is today: `Invalid`, nothing saved).
  - The fingerprint already covers `unmatched`, so the index means the same
    name the user saw.
- Cons: none known.

### Option B — The name itself

- Cons: the name must be revalidated against the plan anyway; it adds a
  free-text field to a form that today carries only ids.

**Recommended: A.**

## Q3 — Placement and name (owner)

### Option A — Last in the user's order, in the order they first appear on the review; the source's spelling, trimmed (recommended)

- Pros:
  - As "Add category" does today (`MAX(position) + 1`).
  - The name is what the user saw on the review. The source's name already
    passed the same name rules (`clean_name`).
- Cons: the user may want to move it up afterwards (with ↑/↓, or MB-0066's
  dragging).

### Option B — After its neighbour in the source's order

- Cons: the source's order, and which of its categories the user has, is
  more of the source's catalog than the review shows today (Q5's security
  question); placement logic for little gain.

**Recommended: A.**

## Q4 — The transaction and the race (data)

### Option A — Created inside the existing transaction; a category made meanwhile shows the review again (recommended)

- Every chosen name is created once, before the products, inside the
  same `BEGIN IMMEDIATE` transaction as the rest of the import or export;
  rows that chose the same name (by `name_key`) share it.
- One insert path: `CategoryRepo::create`'s `INSERT` becomes a helper that
  takes the transaction, used by "Add category" and by both imports and the
  export. The `products` and `recipes` writes then also set the new
  category ids, which are the user's by construction (RR-0001
  constraint 9).
- **The race.** If the user (in another tab) creates a category of that
  name between the review and the confirmation, the recomputed plan
  matches it, the fingerprint changes, and the review is shown again
  ("changed", MB-0060 P4), now with the name matched. No new code path.
  The unique index stays the last guard: a violation rolls back and saves
  nothing.

- Pros:
  - All or nothing, as today; no orphan categories if anything fails.
  - The race reuses the stale-plan mechanism the review already has.
- Cons:
  - `apply_export` and `import_recipe` now write `categories` too
    (RR-0047's constraint 1 says "`products` and nothing else"; amended in
    Q5).
- Risk: low.

### Option B — Use the existing category silently

- Cons: the user chose "Create"; silently linking to a category they made
  meanwhile is a decision not shown on the review (RR-0047 constraint 3,
  "No silent decisions").

### Option C — Create categories first, in their own transaction

- Cons: a failed import leaves categories behind.

**Recommended: A.**

## Q5 — What this reverses, and what the recipient keeps (owner, security)

### Option A — Amend RR-0047 sub-answer 2, its constraint 1, and RR-0048 sub-answer 5 (recommended)

- RR-0047 sub-answer 2 and RR-0048 sub-answer 5: "No category is created"
  becomes "A category is created only when the user chooses 'Create
  ‹name›' on the review (RR-<this>)".
- RR-0047 constraint 1: the transaction writes `products`, and `categories`
  for names chosen on the review.
- The recipient keeps the name only: never the source's category id,
  position, or its other products (RR-0001 constraint 9). They already see
  the name on the review, so nothing new is disclosed.
- The summary counts created categories ("N categories created"); a recipe
  import's confirmation says the same.
- The new choice's label: "Create ‹name›" inside the picker, in an
  `<optgroup>` labelled "New category", below the user's categories and
  above "Skip"; nothing is preselected.

**Recommended: A.**

## Recommendation and question

Recommended: **Q1 A, Q2 A, Q3 A, Q4 A, Q5 A.**

Because: it adds one opt-in choice to the review everywhere at once, the
client sends only an index, and every new write stays inside the
transaction and the stale-plan check the review already has.

Crisp questions to answer:
- **Q1:** all three flows (A), favorites import only (B), or not recipes
  (C)?
- **Q2:** an index (A) or the name (B)?
- **Q3:** last, the source's spelling (A), or after its neighbour (B)?
- **Q4:** inside the transaction, a race shows the review again (A); use
  the existing one silently (B); or a separate transaction (C)?
- **Q5:** amend RR-0047 sub-answer 2 and constraint 1, and RR-0048
  sub-answer 5, as above (A)?

## Blast radius and rollback

What can break: list export, favorites import and recipe import (the
review's choices, the apply transaction); "Add category" (the shared
insert helper).
Who is affected: users who export a shared list or import from a share.
Rollback plan: revert the PR. Categories already created stay as ordinary
categories the user can rename or delete; no schema change.

## Minimal context

- MB-0065 (accepted 2026-11-18), its acceptance P1–P6.
- `crates/kauppa-core/src/export.rs`: `build`, `classify`,
  `classify_ingredient`, `PlanRow::choices`, `fingerprint`, `decide`,
  `writes`, `ingredient_products`.
- `crates/kauppa-core/src/repo/sqlite/product.rs` (`apply_export`),
  `recipe.rs` (`import_recipe`), `category.rs` (`create`).
- `templates/review/page.hbs` (the picker), `crates/kauppa-web/src/routes/review.rs`.
- RR-0001 constraint 9; RR-0003; RR-0047 sub-answers 2, 4 and constraints
  1, 3; RR-0048 sub-answers 5, 6 and constraint 5; CRP-0048 Q5 C.

## Evidence bundle (Hardened)

- **Which rows gain the choice** (from `classify` on `main`, `f651836`):
  - no matched category and no favorite of that name →
    `Review(Uncategorized)`: gains "Create" for each unmatched name;
  - copies in several categories, some unmatched → `Review(Copies)` if two
    or more match; if one matches, `Add` without review (unchanged, the
    unmatched names are ignored as today);
  - an ingredient with no matched category → `Review(Uncategorized)` (no
    skip): gains "Create".
- **The race is already caught:** the fingerprint hashes every one of the
  user's categories (`"c", id, name`), so a new category changes it, and
  `build` would then match the name.
- **No new disclosure:** `unmatched` names are already rendered on the
  review (`export.on_list_unmatched`, `import.in_theirs_unmatched`,
  `import.in_recipe_unmatched`).
- No migration; `idx_categories_owner_name_key` (0004) guards duplicates.

## Owner's answer (2026-11-18)

"as recommended": Q1 A, Q2 A, Q3 A, Q4 A, Q5 A.

Recorded in RR-0052 (scaffold; the owner signs).
