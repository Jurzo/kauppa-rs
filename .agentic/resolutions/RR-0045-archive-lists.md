# Resolution Record — Archived lists: `archived_at`, read-only with Restore, visible to grantees, enforced in `begin_write`

```yaml
---
id: RR-0045
closes: CRP-0045
mission: MB-0062
decided_by: "<resolver's GitHub handle, e.g. @Jurzo>"
role: data-owner
risk_tier: medium
decided_at: <RFC 3339 UTC time of signing, e.g. 2026-11-15T00:00:00Z>
status: <approved | rejected>
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0062-archive-lists.md
  - .agentic/consults/CRP-0045-archive-lists.md
---
```

## Decision and outcome

Approve CRP-0045 as recommended: Q1 A, Q2 A, Q3 A, Q4 A, Q5.

1. **Schema (Q1 A, data-owner).**
   - Migration `0011_list_archive.sql`:
     `ALTER TABLE shopping_lists ADD COLUMN archived_at TEXT;` (RFC 3339,
     like the other timestamps).
   - NULL means active, and existing lists stay active.
   - Archiving and restoring don't change `updated_at`.
2. **An archived list (Q2 A, owner).**
   - Its page and shopping view open read-only, with a notice ("Archived on
     …") and, for the owner, **Restore**.
   - Item and recipe changes are refused and their controls not rendered:
     add, edit, check, remove, restore an item, and add a recipe.
   - The owner can still rename it, delete it and manage its sharing.
3. **Grantees (Q3 A, owner).**
   - An archived list leaves "Shared with you" and appears, read-only, in
     a collapsed "Archived" section on the grantee's `/lists`.
   - Shares are unchanged.
   - Only the owner archives or restores.
4. **Enforcement (Q4 A, security).**
   - The access predicate (`ACCESS_SELECT`) also returns `archived_at`.
   - `begin_write` refuses item and recipe writes on an archived list with
     a new `Conflict` kind, rendered as the translated 409 page (RR-0038).
   - Archive, restore, rename, delete, share and revoke stay allowed by
     their existing access requirement (owner, or as today).
   - Archive and restore are owner-level writes through `begin_write`.
5. **Order (Q5, owner).** The Archived section lists the most recently
   archived first.

Covers: MB-0062's implementation (migration, repo, routes, templates) and
the roll-back below.

Does not cover:

- automatic archiving;
- archiving recipes or favorites;
- live updates of archive events (MB-0057 covers them when it lands).

## Rationale and trade-offs

- One nullable column; no backfill, and an older binary ignores it.
- Read-only matches "finished" and prevents stray changes in a shop;
  Restore is one click.
- Grantees keep sight of the list, so nothing disappears without trace.
- Enforcement next to the single access predicate (RR-0002 constraint 1,
  RR-0036) means no route can forget it.

## Approvals and authority

<resolver: e.g. "@Jurzo as data-owner for sub-answer 1; as owner for sub-answers 2, 3 and 5; as security for sub-answer 4.">

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0045-archive-lists.md`
- Mission Brief: `.agentic/missions/MB-0062-archive-lists.md`
- Evidence: CRP-0045 → Evidence bundle (all 15 `ShoppingListRepo` writes
  call `begin_write`).

## Constraints and follow-on rules (Hardened)

1. Any new list write goes through `begin_write` and states whether it is
   allowed on an archived list; the default is refused.
2. Tests cover every write on an archived list: refused for item and
   recipe writes, allowed for the owner actions in sub-answer 2.
3. The migration is append-only and nullable; no data rewrite.

## Roll-back into the system (Hardened)

To be applied after this record is signed, with the owner's OK:

- `MB-0062`:
  - P3: read-only with Restore; rename, delete and sharing allowed;
  - P4: grantees see archived lists in their own Archived section;
  - P6: names `archived_at` and the `begin_write` refusal;
  - the five escalate entries marked decided (RR-0045);
  - `resolutions: [RR-0045]`;
  - plan step 1 marked done.
- `CRP-0045`: `state: open` → `answered`.
- `.agentic/mentorship/persistence.md` is not changed here. If the owner
  wants the "new writes state their archived behaviour" rule (constraint
  1) in the pack, that is a separate consult.

## Supersession and history

Status: active once signed. Supersedes nothing.
