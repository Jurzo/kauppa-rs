# Resolution Record — Export a list's items to favorites: any reader into their own catalog, explicit review, stateless and rechecked

```yaml
---
id: RR-0047
closes: CRP-0047
mission: MB-0060
decided_by: "@Jurzo"
role: security
risk_tier: medium
decided_at: 2026-11-15t14:30:00z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0060-list-export-to-favorites.md
  - .agentic/consults/CRP-0047-list-export-to-favorites.md
---
```

## Decision and outcome

Approve CRP-0047 as recommended: Q1–Q7 A, and Q8.

1. **Who (Q1 A, security).** Anyone who can open a list may export it: the
   owner, and `edit` and `view` grantees. Each export goes into the
   exporting user's own favorites. This RR is the record RR-0024
   constraint 6 asks for. A grantee's export reads the list's item names
   and their categories' names, which the list page already shows them,
   and nothing else of the owner's catalog.
2. **A grantee's categories (Q2 A, owner).** Each list category is matched
   by name, ignoring case (`name_key`, RR-0003), to the exporting user's
   own category. With no match, the item counts as uncategorized for that
   user (item 4). No category is created.
   Amended by RR-0052: a category is created when the user chooses
   'Create ‹name›' on the review.
3. **Which items (Q3 A, owner).**
   - Every active item, checked or not; removed items never.
   - The same name more than once makes one favorite. If the copies'
     categories differ, the review asks which to use, or to skip it.
   - Quantities aren't exported.
4. **Items without a category (Q4 A, owner).** They are listed on the
   review step with a picker of the user's categories and "Skip"; each
   needs a choice.
5. **Conflicts (Q5 A, owner).** A conflict is an item whose name matches
   one of the user's products with a different category.
   - **A favorite:** "Keep ‹its category›" or "Use ‹the list's category›".
   - **A product from recipes:** make it a favorite in its category, or in
     the list's category, or "Skip".
   - Nothing is preselected. Every row must be answered, and the server
     refuses a missing or unknown answer.
6. **The review's state (Q6 A, security).**
   - **The form.** The review is a stateless form carrying the answers
     (per row: the product's id or the item's name, and a category id or
     "skip") and a SHA-256 fingerprint of the plan it was computed from:
     each item's name and category, and each matched product's id, name,
     category and favorite flag.
   - **The confirmation.** It recomputes the plan inside the write
     transaction.
     - If the fingerprint differs, it saves nothing and shows the review
       again with the current rows.
     - Otherwise it applies the answers. It accepts answers only for rows
       in that plan, and only category ids the user owns.
7. **Where (Q7 A, security).**
   - **Read:** the route reads the list through
     `ShoppingListRepo::items(user, list, false)`, i.e. the access
     predicate at `view`.
   - **Plan:** a pure kauppa-core function classifies the items against the
     user's catalog: new, identical, promote, conflict, uncategorized.
   - **Write:** a new `ProductRepo::apply_export` runs one `BEGIN IMMEDIATE`
     transaction on `products` only, with the fingerprint check inside.
   - **List files:** `repo/sqlite/list.rs` and `list_access.rs` don't
     change.
8. **Archived lists (Q8, owner).** Export is offered on archived lists
   too. It only reads the list, and RR-0045 refuses only item and recipe
   writes.

Covers:
- MB-0060's implementation: the plan function, `ProductRepo::apply_export`,
  the routes `/lists/{id}/export` and `/lists/{id}/export/confirm`, the
  review and summary templates;
- the roll-back below.

Does not cover:
- creating categories;
- exporting quantities;
- MB-0061's import, which reuses the review step under its own consult;
- any change to the list tables or the access predicate.

## Rationale and trade-offs

- Any reader may keep a list's items, but nobody's catalog other than
  their own is ever written, and the owner's category ids never cross over
  (RR-0001 constraint 9).
- Every change to an existing favorite is a decision the user made
  (forbidden otherwise by the brief).
- A stateless review rechecked in the write transaction can't apply a
  stale decision, and needs no stored state.
- Rejected:
  - owner-only export (Q1 B), which shuts out grantees;
  - creating categories (Q2 B);
  - preselected answers (Q5 B);
  - server-side review state (Q6 B);
  - a list-repo method that writes products (Q7 B).

## Approvals and authority

@Jurzo as security for sub-answers 1, 6 and 7; as owner for sub-answers 2, 3, 4, 5 and 8.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0047-list-export-to-favorites.md`
- Mission Brief: `.agentic/missions/MB-0060-list-export-to-favorites.md`
- Related: RR-0001 (constraints 2, 3 as amended by RR-0046, 6, 9), RR-0002
  constraint 1, RR-0003 constraint 3, RR-0024 constraint 6, RR-0045.

## Constraints and follow-on rules (Hardened)

1. **Own catalog only.** `apply_export` writes the acting user's
   `products` and nothing else. Every `category_id` it sets is checked as
   the user's in the same transaction.
   Amended by RR-0052: a category is created when the user chooses
   'Create ‹name›' on the review.
2. **The list through the predicate.** The export reads list items only
   through `ShoppingListRepo` (at `view`). A query on the list tables
   anywhere else is a review failure (RR-0002 constraint 1).
3. **No silent decisions.** A conflict or uncategorized row without an
   explicit answer saves nothing. A fingerprint mismatch saves nothing.
4. **Tests.** Route tests cover every role (owner, `edit`, `view`,
   stranger), a grantee's category mapping, a stale review (the list and
   the catalog changed), a tampered answer (a foreign category id, or a
   row not in the plan), and an archived list.
5. **Reuse.** MB-0061 may reuse the plan and review for its imports. Who
   may import, and from what, is its own consult.

## Roll-back into the system (Hardened)

To be applied after this record is signed, with the owner's OK:

- `MB-0060`:
  - P1: offered to the owner and `edit` and `view` grantees, archived lists
    included; every active item;
  - P3: the conflict choices of item 5, none preselected;
  - P4: the fingerprint, rechecked in the write transaction;
  - P5: uncategorized items chosen on the review step;
  - P6: a grantee's categories mapped by name, else uncategorized;
    `ProductRepo::apply_export` writes products only;
  - the seven escalate entries marked decided (RR-0047);
  - `resolutions: [RR-0047]`;
  - plan step 1 marked done.
- `CRP-0047`: `state: open` → `answered`.

## Supersession and history

Status: active. Supersedes nothing. Amended by RR-0052 (sub-answer 2,
constraint 1).

- 2026-11-18: amended by RR-0052 (MB-0065).
