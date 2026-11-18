# Resolution Record — The review may create a missing category, by the user's choice, in all three flows

```yaml
---
id: RR-0052
closes: CRP-0052
mission: MB-0065
decided_by: "@Jurzo"
role: product
risk_tier: medium
decided_at: 2026-11-18t18:10:00z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0065-create-missing-categories-on-import.md
  - .agentic/consults/CRP-0052-create-missing-categories-on-review.md
  - .agentic/resolutions/RR-0047-list-export-to-favorites.md
  - .agentic/resolutions/RR-0048-catalog-sharing-and-import.md
---
```

## Decision and outcome

Approve CRP-0052 Q1 A, Q2 A, Q3 A, Q4 A and Q5 A.

1. **Flows (Q1 A, owner).** Every review row that has unmatched source
   category names offers "Create ‹name›" for each of them, in all three
   flows: a grantee's list export, favorites import, and recipe import (a
   new product's category, or an existing product without one). Only rows
   reviewed today gain the choice; no row is newly reviewed, and the
   "added automatically" count is unchanged.
2. **The answer (Q2 A, security).** The option's value is `new:<n>`, the
   n-th of the row's unmatched names in the plan. The confirmation reads
   the name from its own recomputed plan; the form never carries a name.
   An index out of range, or `new:` on a row without unmatched names, is a
   tampered answer: `Invalid`, nothing saved.
3. **Placement and name (Q3 A, owner).** Created categories go last in the
   user's order, in the order they first appear on the review, with the
   source's name trimmed.
4. **The transaction and the race (Q4 A, data).**
   - Each chosen name (by `name_key`) is created once, before the products,
     inside the same `BEGIN IMMEDIATE` transaction as the rest of the
     import or export; rows that chose the same name share it.
   - One insert path: `CategoryRepo::create`'s `INSERT` becomes a helper
     taking the transaction, used by "Add category", both imports and the
     export.
   - A category of that name made meanwhile changes the fingerprint, so the
     review is shown again ("changed"); the unique index
     `idx_categories_owner_name_key` stays the last guard, and a violation
     saves nothing.
5. **What this reverses (Q5 A, owner, security).**
   - RR-0047 sub-answer 2 and RR-0048 sub-answer 5: "No category is
     created" becomes "A category is created only when the user chooses
     'Create ‹name›' on the review (RR-0052)".
   - RR-0047 constraint 1: `apply_export` writes the user's `products`,
     and `categories` for names chosen on the review.
   - The user keeps the source category's name only: never its id,
     position or other products (RR-0001 constraint 9).
   - The summary counts created categories; a recipe import's confirmation
     says the same.
   - The choice is "Create ‹name›" in the picker, in an `<optgroup>`
     labelled "New category", below the user's categories and above
     "Skip"; nothing is preselected.

Covers:
- MB-0065's implementation as above;
- the roll-back below.

Does not cover:
- creating categories without the user's choice on the review;
- placement after the source's neighbours, or a normalised spelling;
- a migration;
- dragging favorites between categories (MB-0066's follow-up).

## Rationale and trade-offs

- One rule in `PlanRow::choices` keeps the review the same in every flow
  (RR-0048 constraint 5).
- An index keeps free text out of a form that carries only ids, and the
  fingerprint already covers the unmatched names.
- Creating inside the transaction keeps the import all or nothing; the
  race reuses the stale-plan check.
- Rejected: favorites only (Q1 B) or not recipes (Q1 C); sending the name
  (Q2 B); placing after the source's neighbour (Q3 B); using a category
  made meanwhile silently (Q4 B, RR-0047 constraint 3); a separate
  transaction (Q4 C).

## Approvals and authority

@Jurzo as product, data-owner and security.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0052-create-missing-categories-on-review.md`
  (its evidence bundle)
- Mission Brief: `.agentic/missions/MB-0065-create-missing-categories-on-import.md`
- Continuity Pack: `.agentic/continuity/MB-0065.md`
- Related: RR-0001 (constraint 9), RR-0003, RR-0047, RR-0048, CRP-0048
  (Q5 C, rejected then).

## Constraints and follow-on rules (Hardened)

1. **Only by choice.** A category is created only for a `new:<n>` answer
   the confirmation accepted; never for an unanswered or skipped row.
2. **Names from the plan.** The created name comes from the recomputed
   plan, never from the request.
3. **One transaction, one insert path.** Categories, products and recipes
   are written in one `BEGIN IMMEDIATE` transaction; every category insert
   goes through the shared helper.
4. **Tests.** Route tests cover each flow, a shared name across rows, the
   race (a category made between review and confirmation), a tampered
   index, and that the list owner's own export is unchanged.
5. **Same review everywhere.** As RR-0048 constraint 5: a later change to
   the choice applies to all three flows and needs its own consult.

## Roll-back into the system (Hardened)

To be applied after this record is signed, with the owner's OK:

- `MB-0065`: `resolutions: [RR-0052]`; plan step 1 marked done; the five
  escalate entries marked decided (RR-0052).
- `CRP-0052`: `state: open` → `answered`.
- `RR-0047`: after sub-answer 2 and constraint 1, "Amended by RR-0052: a
  category is created when the user chooses 'Create ‹name›' on the
  review"; a history entry "Amended by RR-0052".
- `RR-0048`: after sub-answer 5, the same note; a history entry "Amended
  by RR-0052".

## Supersession and history

Status: active. Amends RR-0047 (sub-answer 2, constraint 1)
and RR-0048 (sub-answer 5); supersedes nothing.

- 2026-11-18: roll-back applied to MB-0065, CRP-0052, RR-0047 and RR-0048 (PR #130).
