# Resolution Record — Amend RR-0044 constraint 1: a held favorite opens a closed group (MB-0071)

```yaml
---
id: RR-0056
closes: CRP-0056
mission: MB-0071
decided_by: "<resolver's GitHub handle, e.g. @Jurzo>"
role: product
risk_tier: low
decided_at: <RFC 3339 UTC time of signing, e.g. 2026-11-22T00:00:00Z>
status: <approved | rejected>
superseded_by: null
rolls_back_into:
  - .agentic/resolutions/RR-0044-favorite-categories-collapsible.md
  - .agentic/missions/MB-0071-drag-favorite-between-categories.md
  - .agentic/consults/CRP-0056-open-group-while-dragging.md
---
```

## Decision and outcome

Approve CRP-0056 option A. This amends RR-0044 constraint 1, which now
reads:

> No JavaScript decides which groups are open: the server's answers open
> them through `HX-Retarget` and `<details open>`. The one exception is a
> drag (MB-0071, RR-0056): while a favorite's row is held,
> `assets/app/sortable.js` may open a closed group whose heading the
> pointer rests on, so that the group can take the drop. It never closes a
> group, and the answer to the drop decides the groups' state as before.

Covers:

- MB-0071's implementation (PR #146): the hover opening, after the held
  row rests 500 ms on a closed group's heading;
- the roll-back below.

Does not cover:

- any other script behaviour on `/favorites`: groups still open and close
  only by their heading, and actions' answers still open the touched groups
  (RR-0044 items 1–3);
- the `moved` field on `POST /favorites/order` and the drag script's move
  between lists, which MB-0071 records (the owner's answers on
  acceptance, 2026-11-22);
- empty categories, which stay hidden (RR-0044 item 3).

## Rationale and trade-offs

- It records the owner's MB-0071 decision ("open a closed category when
  hovering over it") in the rule it changes.
- The exception is limited to a held row, so without JavaScript, or when
  no row is held, groups behave exactly as RR-0044 decided.
- Rejected: only open groups take a drop (B), which undoes the owner's
  decision.

## Approvals and authority

<resolver: e.g. "@Jurzo as owner (product).">

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0056-open-group-while-dragging.md`
- Amends: `.agentic/resolutions/RR-0044-favorite-categories-collapsible.md`
  (constraint 1)
- Mission Brief: `.agentic/missions/MB-0071-drag-favorite-between-categories.md`
- Continuity Pack: `.agentic/continuity/MB-0071.md`
- Evidence: PR #146; `tests/browser/flows/regroup.spec.js` (the closed
  group opens on hover and takes the drop; a mutation that never opens it
  fails the flow).
- Related: RR-0053 (the drag script, whose constraint 2 still holds).

## Constraints and follow-on rules (Hardened)

1. The script opens a group only while a row is held, only a closed one,
   and only one whose list shares the held row's Sortable group.
2. The script never closes a group or changes which groups an answer
   opens.

## Roll-back into the system (Hardened)

To be applied after this record is signed, with the owner's OK:

- `RR-0044`: an amendment note under Supersession and history (as
  RR-0001's for RR-0046), quoting the new constraint 1. RR-0044's own text
  stays as signed.
- `MB-0071`: `resolutions: [RR-0056]`; the closed-group escalation gets
  "(RR-0056)"; plan step 1's note on RR-0044 says the amendment is
  recorded.
- `CRP-0056`: `state: open` → `answered`.

## Supersession and history

Status: active once signed. Supersedes nothing; amends RR-0044
constraint 1.
