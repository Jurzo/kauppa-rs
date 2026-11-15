# Resolution Record — Collapsible favorite categories: per-group swaps, one collapsed "From recipes" section

```yaml
---
id: RR-0044
closes: CRP-0044
mission: MB-0059
decided_by: "<resolver's GitHub handle, e.g. @Jurzo>"
role: product
risk_tier: low
decided_at: <RFC 3339 UTC time of signing, e.g. 2026-11-15T00:00:00Z>
status: <approved | rejected>
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0059-favorite-categories-collapsible.md
  - .agentic/consults/CRP-0044-favorite-categories-collapsible.md
---
```

## Decision and outcome

Approve CRP-0044 Q1 **B**, Q2 **A** and Q3 as recommended.

1. **Groups (Q1 B).**
   - Each category group on `/favorites` is a `<details>`, closed on a full
     page load, with a stable id per group (by category id; one id for
     Uncategorized).
   - An action whose effect stays inside one group replaces only that
     group, so the groups the user opened stay as they were: ↑/↓, a Save
     that keeps the category, and a delete that leaves products in the
     group. The group is rendered open.
   - An action that moves a product between groups, or adds or removes a
     group, replaces the whole catalog with the touched groups open: a Save
     that changes the category (RR-0043's "No category" included), adding
     a favorite, "Make favorite", and deleting a group's last product.
   - The server decides which applies; with htmx it answers the group
     fragment, or retargets to `#favorites` (`HX-Retarget`). Without htmx,
     every action answers the full page with the touched groups open.
   - An error answer replaces the whole catalog with the error and the
     touched group open, as errors are shown today.
2. **"From recipes" (Q2 A).** One `<details>` section, closed on a full
   page load, its category groups shown inside as headings (not
   collapsible). An action inside it that stays inside it replaces only
   the section, rendered open; one that moves a product into the favorites
   ("Make favorite") replaces the whole catalog with the section and the
   target group open.
3. **Add form and empty categories (Q3).** Adding a favorite opens its
   group (a cross-group action, item 1). Categories with no favorites stay
   hidden, as today.

Covers: MB-0059's implementation and the roll-back below.

Does not cover: remembering open groups across page loads or devices
(CRP-0044 Q1 C, rejected); JavaScript (forbidden by the brief).

## Rationale and trade-offs

- The owner chose per-group swaps (B) over closing every untouched group
  (A): groups the user opened stay open through most actions.
- Trade-off accepted: two answer shapes per action and more fragment
  tests; cross-group actions still close untouched groups.

## Approvals and authority

<resolver: e.g. "@Jurzo as owner (product).">

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0044-favorite-categories-collapsible.md`
- Mission Brief: `.agentic/missions/MB-0059-favorite-categories-collapsible.md`
- Evidence: CRP-0044 → Evidence bundle.

## Constraints and follow-on rules (Hardened)

1. No JavaScript; `HX-Retarget` and `<details open>` are the only
   mechanisms.
2. A group's id doesn't change when the category is renamed.
3. With MB-0058 (RR-0043), a Save's answer shape follows whether its
   category changed.

## Roll-back into the system (Hardened)

To be applied after this record is signed, with the owner's OK:

- `MB-0059`:
  - P2's property: as item 1 (per-group swap within a group; whole catalog
    with touched groups open across groups); its check becomes
    `routes::products::tests::catalog::group_swaps` (record the rename);
  - P3's property: as item 2;
  - the three escalate entries marked decided (RR-0044);
  - the non-goal on remembering groups confirmed;
  - `resolutions: [RR-0044]`;
  - plan step 1 marked done.
- `CRP-0044`: `state: open` → `answered`.

## Supersession and history

Status: active once signed. Supersedes nothing.
