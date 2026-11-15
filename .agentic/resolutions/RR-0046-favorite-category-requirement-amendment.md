# Resolution Record — Amend RR-0001 constraint 3: a favorite needs a category when created; its owner may clear it later

```yaml
---
id: RR-0046
closes: CRP-0046
mission: MB-0058
decided_by: "@Jurzo"
role: data-owner
risk_tier: low
decided_at: 2026-11-15T09:22:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/resolutions/RR-0001-product-category-data-model.md
  - .agentic/missions/MB-0058-favorite-one-save.md
  - .agentic/consults/CRP-0046-favorite-category-requirement-amendment.md
---
```

## Decision and outcome

Approve CRP-0046 option A. This amends RR-0001 constraint 3, which now
reads:

> **`category_id` stays nullable in the schema** on every table that has
> one. A favorite needs a category when it is created or made a favorite
> (the add form, "Make favorite"); its owner may later clear it explicitly
> ("No category", RR-0043), and it then shows under Uncategorized. These
> rules are enforced at the handler layer. Do not add `NOT NULL`.

Covers:

- MB-0058's implementation of RR-0043;
- the roll-back below.

Does not cover:

- any schema change;
- list items, which keep their own rules (RR-0001 constraint 5);
- recipe ingredients, whose rule is unchanged by this record.

## Rationale and trade-offs

- It records in the data-model rule the owner's CRP-0043 choice (C).
- Uncategorized favorites already exist when a category is deleted (RR-0001
  constraints 4 and 5), so this adds no new state.
- Rejected: narrowing RR-0043 (B), which would undo the owner's choice.

## Approvals and authority

@Jurzo as data-owner.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0046-favorite-category-requirement-amendment.md`
- Amends: `.agentic/resolutions/RR-0001-product-category-data-model.md`
  (constraint 3)
- Related: `.agentic/resolutions/RR-0043-favorite-one-save.md`
- Mission Brief: `.agentic/missions/MB-0058-favorite-one-save.md`

## Constraints and follow-on rules (Hardened)

1. Creating a favorite and "Make favorite" still require a category
   (`parse_category`; `can_favorite` only with a category).
2. Clearing happens only through RR-0043's explicit choice (RR-0043
   constraints 1 and 2).

## Roll-back into the system (Hardened)

To be applied after this record is signed, with the owner's OK:

- `RR-0001`: an amendment note under Supersession and history (as RR-0003's
  is), quoting the new constraint 3. RR-0001's own text stays as signed.
- `MB-0058`: `resolutions: [RR-0043, RR-0046]`; the Context notes the
  amendment.
- `CRP-0046`: `state: open` → `answered`.

## Supersession and history

Status: active once signed. Supersedes nothing; amends RR-0001
constraint 3.
