# Resolution Record — Sortable and its connecting script load on every page

```yaml
---
id: RR-0054
closes: CRP-0054
mission: MB-0066
decided_by: "@Jurzo"
role: platform
risk_tier: low
decided_at: 2026-11-18t18:57:00z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0066-drag-to-reorder.md
  - .agentic/consults/CRP-0054-sortable-under-boosted-navigation.md
  - .agentic/resolutions/RR-0053-drag-to-reorder.md
  - AGENTS.md
---
```

## Decision and outcome

Approve CRP-0054 Q1 A.

1. **Every page (Q1 A, platform).** `layout.hbs` loads
   `/assets/vendor/Sortable-1.15.7.min.js` and then `/assets/app/sortable.js`,
   each `<script defer>` with its SRI, after htmx and its two extensions, on
   every page. The `page_sortable` parameter and the
   `templates/sortable/_scripts.hbs` partial go; the layout's pinned script
   list (`LAYOUT_SCRIPTS`) gains the two.
2. **The script is unchanged in what it does:** it marks `<html>` with
   `sortable-ready` and connects each `[data-sortable]` list htmx loads;
   pages without one get no handles and no Sortable instance.
3. **A browser flow** reaches a drag page by an in-app (boosted) click and
   drags there, so the case that failed under RR-0053 is covered.

Covers:
- the loading change above in MB-0066's PR;
- the roll-back below.

Does not cover:
- loading Sortable on demand (CRP-0054 Q1 B), opting out of boost (C);
- any other script, a CSP change, or response compression.

## Rationale and trade-offs

- Under `hx-boost`, htmx keeps only the `<title>` of a response's `<head>`,
  so per-page scripts in `<head>` load only on a full page load; measured
  in CRP-0054. Loading from the layout makes dragging work however the page
  is reached, with the least code.
- The cost is 45,478 bytes on a browser's first visit, then cached for a
  year (`immutable`, RR-0029), and parsing it on each page load.
- Rejected: on-demand loading (Q1 B, more first-party code); no boost into
  the drag pages (Q1 C, fragile); leaving it (Q1 D, dragging rarely
  available); htmx's head-support extension (needs `allowScriptTags`).

## Approvals and authority

@Jurzo as platform.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0054-sortable-under-boosted-navigation.md`
  (its evidence bundle: the boosted-navigation probe)
- Mission Brief: `.agentic/missions/MB-0066-drag-to-reorder.md`
- Continuity Pack: `.agentic/continuity/MB-0066.md`
- Related: RR-0053 (amended), RR-0019, RR-0029.

## Constraints and follow-on rules (Hardened)

1. **Layout only.** The two scripts are loaded by `layout.hbs` and nowhere
   else; `navigation_needs_no_script`'s rule (only the layout loads
   scripts) holds again without an exception.
2. **Order.** htmx, response-targets, SSE, Sortable, `sortable.js`, all
   `defer`, each with SRI pinned by the test suite.
3. **Inert elsewhere.** `sortable.js` creates Sortable instances only for
   `[data-sortable]` lists.

## Roll-back into the system (Hardened)

To be applied after this record is signed, with the owner's OK:

- `MB-0066`: `resolutions: [RR-0053, RR-0054]`; a note in the plan that
  RR-0054 changed where the scripts load.
- `CRP-0054`: `state: open` → `answered`.
- `RR-0053`: after sub-answer 2's `page_sortable` item and constraint 3, "Amended
  by RR-0054: both scripts load on every page, from the layout"; a history
  entry "Amended by RR-0054".
- `AGENTS.md`, do-not-touch boundaries: "load through `<script defer>` with
  SRI on the pages that need them" becomes "load through `<script defer>`
  with SRI from the layout (RR-0054)".

## Supersession and history

Status: active once signed. Amends RR-0053 (sub-answer 2's loading,
constraint 3); supersedes nothing.
