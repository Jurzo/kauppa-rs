# Consultation Request Pack — Amend RR-0001 constraint 3: a favorite's category may be cleared by its owner (RR-0043)

```yaml
---
id: CRP-0046
mission: MB-0058
owner: "@juuso-kalliomaki"
target_role: data-owner
risk_tier: low
urgency: medium
state: answered
runbook_trigger: "conflict between approved RRs found at MB-0058's implementation start: RR-0001 constraint 3 ('A favorite or recipe ingredient must have a category', handler rule) vs RR-0043 (Save with 'No category' clears a favorite's category); feature.md escalation trigger `.agentic/resolutions/**`"
---
```

## Decision statement

Decide how to reconcile RR-0001 constraint 3, which requires a favorite to
have a category, with RR-0043, which lets the owner clear it with "No
category".

## Options and trade-offs

### Option A — Amend RR-0001 constraint 3 (recommended)

New wording: "A favorite needs a category when it is created or made a
favorite (the add form, 'Make favorite'); its owner may later clear it
explicitly ('No category', RR-0043), and it then shows under
Uncategorized. A recipe ingredient's product needs no category."

- Pros: matches the owner's RR-0043 choice; Uncategorized is already a
  real state for favorites (RR-0001 constraint 5: a deleted category
  leaves them there).
- Cons: the rule becomes "required at creation" rather than "always".
- Risk: low; the schema is unchanged (the column stays nullable,
  constraint 3's "Do not add `NOT NULL`" stands).

### Option B — Narrow RR-0043 to fit RR-0001

"No category" is offered only on products that have none now; a favorite
with a category can be moved but not cleared.

- Pros: RR-0001 unchanged.
- Cons: not what the owner chose in CRP-0043 (C).

## Recommendation and question

Recommended option: A.
Because: it records the owner's CRP-0043 decision in the data-model rule
it changes.

Crisp question to answer: Amend RR-0001 constraint 3 as worded in A?

## Blast radius and rollback

What can break: nothing in code yet; MB-0058's implementation follows the
amended rule.
Who is affected: users editing favorites.
Rollback plan: a later RR restoring the old wording; uncategorized
favorites would then be prompted for a category on their next Save.

## Minimal context

- RR-0001 constraint 3: "`category_id` stays nullable in the schema on
  every table that has one. 'A favorite or recipe ingredient must have a
  category' is enforced at the handler layer. Do not add `NOT NULL`."
- RR-0001 constraint 5: "Uncategorized is a real state."
- RR-0043 (approved 2026-11-15): "Saving with 'No category' clears the
  product's category. A favorite then shows under Uncategorized."
- `persistence.md` says only that requiredness is a handler rule; it needs
  no change.
- Found when starting MB-0058's implementation; CRP-0043 did not cite
  RR-0001.

## Evidence bundle (Hardened)

- `crates/kauppa-web/src/routes/products/mod.rs` module comment and
  `parse_category` ("Enforce 'a favorite must have a category'
  (RR-0001)").

## Owner's answer (2026-11-15)

A. Recorded in RR-0046 (scaffold; the owner signs).
