# Consultation Request Pack — Amend RR-0044 constraint 1: a held favorite opens a closed group (MB-0071)

```yaml
---
id: CRP-0056
mission: MB-0071
owner: "@juuso-kalliomaki"
target_role: product
risk_tier: low
urgency: medium
state: open
runbook_trigger: "conflict between an approved RR and the owner's decision, found at MB-0071's implementation: RR-0044 constraint 1 ('No JavaScript; `HX-Retarget` and `<details open>` are the only mechanisms') vs the owner's answer on MB-0071's acceptance (2026-11-22: 'open a closed category when hovering over it'); feature.md escalation trigger `.agentic/resolutions/**`"
---
```

## Decision statement

Reconcile RR-0044 constraint 1, under which the favorites groups' open and
closed state uses no JavaScript, with MB-0071: a closed group opens while a
dragged favorite rests on its heading, which only the drag script can do.

## Options and trade-offs

### Option A — Amend RR-0044 constraint 1 for the drag only (recommended)

New wording:

> No JavaScript decides which groups are open: the server's answers open
> them through `HX-Retarget` and `<details open>`. The one exception is a
> drag (MB-0071, RR-0056): while a favorite's row is held,
> `assets/app/sortable.js` may open a closed group whose heading the
> pointer rests on, so that the group can take the drop. It never closes a
> group, and the answer to the drop decides the groups' state as before.

- Pros:
  - Records the owner's decision.
  - Keeps the rule for everything else: groups open and close by tapping
    their heading, and actions' answers open the touched groups.
- Cons: the groups' state now has one script path, during a drag.
- Risk: low. Without JavaScript nothing changes (no drag, no handle).

### Option B — Keep RR-0044; only open groups take a drop

- Pros: RR-0044 unchanged.
- Cons: undoes the owner's decision; to drop into a closed group, the
  user has to open it first, and can't while holding a row.

## Recommendation and question

Recommended option: A.
Because: it records the owner's MB-0071 decision in the rule it changes,
and limits the exception to a held row.

Crisp question to answer: amend RR-0044 constraint 1 as worded in A?

## Blast radius and rollback

- What can break: nothing beyond MB-0071's PR #146, which implements it.
- Who is affected: users dragging favorites on `/favorites`.
- Rollback plan: a later RR restoring the old wording; the drag script
  then stops opening groups, and only open groups take a drop.

## Minimal context

- RR-0044 constraint 1: "No JavaScript; `HX-Retarget` and `<details open>`
  are the only mechanisms."
- MB-0071's escalation on dropping into a closed group, answered by the
  owner on acceptance (2026-11-22): "open a closed category when hovering
  over it".
- PR #146 (stacked on #144): the drag script opens a closed group after the
  held row rests 500 ms on its heading; `tests/browser/flows/regroup.spec.js`
  checks it.

## Evidence bundle (Hardened)

- `assets/app/sortable.js` on `agentic/mb-0071-implementation`: `track` and
  `holding` (only while a row is held; only a group whose list shares the
  held row's Sortable group).
- `.agentic/resolutions/RR-0044-favorite-categories-collapsible.md`,
  Constraints, item 1.

## Owner's answer (2026-11-22)

A, as decided on MB-0071's acceptance ("open a closed category when
hovering over it"); the owner asked for this record ("Draft a short RR for
the amendment to RR-0044"). Recorded in RR-0056 (scaffold; the owner
signs).
