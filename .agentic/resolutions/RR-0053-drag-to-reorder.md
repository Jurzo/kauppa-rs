# Resolution Record — Drag to reorder: Sortable 1.15.7 vendored, one first-party script under `assets/app/`, ↑/↓ kept, full-order routes, CSP unchanged

```yaml
---
id: RR-0053
closes: CRP-0053
mission: MB-0066
decided_by: "@Jurzo"
role: platform
risk_tier: medium
decided_at: 2026-11-18t18:12:00z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0066-drag-to-reorder.md
  - .agentic/consults/CRP-0053-drag-to-reorder.md
  - AGENTS.md
  - .agentic/mentorship/templating.md
---
```

## Decision and outcome

Approve CRP-0053 Q1 A, Q2 A, Q3 A, Q4 A and Q5 A.

1. **Sortable (Q1 A, platform).** `Sortable.min.js` from `sortablejs`
   1.15.7 on the npm registry (MIT), byte-for-byte, as
   `assets/vendor/Sortable-1.15.7.min.js`; pinned in `templates/tests.rs`
   by length 45,478, SHA-256
   `bf4241bc73fef7f11c59a283a69fe8051cdd31c6d8ff5a2b9ba219e7831fcf76` and
   SRI `sha384-DgmC6Xe2bSN2WjTDXzWYbUbxyhNP+NNkGDR/g78pCXV7E7rcVTGxVg0uIVCUUcBc`.
2. **The connecting code (Q2 A, platform).**
   - One first-party file, `assets/app/sortable.js`: `htmx.onLoad` creates a
     `Sortable` on each `[data-sortable]` element with
     `handle: ".drag-handle"`; Sortable's `end` event triggers the order
     form.
   - The order form sits beside the list (`hx-trigger="end from:…"`); each
     row carries a hidden `id` input with `form="…"`, so the ids are sent in
     document order.
   - A layout parameter `page_sortable=true` adds both `<script defer>`
     tags, with SRI, on the categories, favorites and recipe pages only.
     Amended by RR-0054: both scripts load on every page, from the layout.
3. **The ↑/↓ buttons (Q3 A, owner).** They stay, with the move routes and
   their tests unchanged. A drag handle ("⠿", accessible name "Drag to
   reorder ‹name›", in both catalogs) sits beside them, hidden without
   JavaScript.
4. **The order routes (Q4 A, owner).**
   - `POST /categories/order`, `POST /favorites/order` (the category's id
     as a field), `POST /recipes/{id}/ingredients/order`; the body is `id`
     repeated, in the new order.
   - Saved only if the ids are exactly the user's current rows of that
     list (none missing, extra, repeated or foreign), in one
     `BEGIN IMMEDIATE` transaction through the repo traits
     (`CategoryRepo::reorder`; one new method each on `ProductRepo` and
     `RecipeRepo`).
   - Otherwise nothing is saved and the section is returned with the
     current order. A recipe or category not the user's is 404. Without
     htmx, a redirect back to the page. The cross-site check covers them.
5. **The CSP (Q5 A, security).** No change: `script-src 'self'; style-src
   'self'`. A CSP violation in the browser suite stops the mission.

Covers:
- MB-0066's implementation as above;
- the roll-back below, including the AGENTS.md and `templating.md` rule.

Does not cover:
- dragging a favorite into another category (the follow-up; its own brief
  and consult, which may extend `assets/app/sortable.js`);
- any other first-party script, or loading these on other pages;
- a CSP or security-header change;
- removing the ↑/↓ buttons or the move routes.

## Rationale and trade-offs

- Sortable handles mouse and touch, needs no `eval` or inline styles, and
  its `group` option serves the follow-up.
- One small first-party file under a narrow, listed rule keeps
  `script-src 'self'` and no inline code; loading it on three pages keeps
  other pages as light as today.
- The buttons keep reordering working by keyboard and without JavaScript.
- A full order with an exact-set check never applies a partial or stale
  order.
- Rejected: our own drag-and-drop (Q1 B); no dragging (Q1 C); loading on
  every page (Q2 B); hiding (Q3 B) or removing (Q3 C) the buttons; one
  step per drop (Q4 B).

## Approvals and authority

@Jurzo as platform, owner and security.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0053-drag-to-reorder.md` (its evidence
  bundle: the npm integrity check, checksums, the CSP scan)
- Mission Brief: `.agentic/missions/MB-0066-drag-to-reorder.md`
- Continuity Pack: `.agentic/continuity/MB-0066.md`
- Related: RR-0006, RR-0015, RR-0017, RR-0019, RR-0029, RR-0049.

## Constraints and follow-on rules (Hardened)

1. **Vendored as published.** The vendored file matches the checksums
   above; upgrading or modifying it is a platform decision (RR-0015
   constraint 1).
2. **First-party scripts.** Only files under `assets/app/` that an RR
   approves; a test lists them, so an unlisted file fails. No inline code,
   no `eval` or `new Function`, no `innerHTML`, no event-handler
   attributes. This record approves `assets/app/sortable.js` only.
3. **Three pages.** The scripts load only where `page_sortable` is set: the
   categories, favorites and recipe pages. Amended by RR-0054: both scripts load on every page, from the layout.
4. **The CSP stays.** The browser suite's CSP and console checks pass on
   every flow, with and without JavaScript.
5. **Tests.** Route tests for each order route (exact set, stale, foreign,
   404); browser flows dragging on each page, without JavaScript and by
   keyboard; the owner's phone check (MB-0066 P7).

## Roll-back into the system (Hardened)

To be applied after this record is signed, with the owner's OK:

- `MB-0066`: `resolutions: [RR-0053]`; plan step 1 marked done; the five
  escalate entries marked decided (RR-0053).
- `CRP-0053`: `state: open` → `answered`.
- `AGENTS.md`, do-not-touch boundaries, after the `assets/vendor/`
  sentence: "First-party scripts may live under `assets/app/` only, one
  file per approving RR (`assets/app/sortable.js`, RR-0053); a test lists
  them. They carry no `eval`, no `innerHTML` and no event-handler
  attributes, and load through `<script defer>` with SRI on the pages that
  need them." The Map's client-interactivity row gains "`assets/app/`
  (RR-0053)".
- `.agentic/mentorship/templating.md`: "`assets/` holds nothing executable
  outside `assets/vendor/`" becomes "outside `assets/vendor/` and the
  approved files under `assets/app/` (RR-0053)".

## Supersession and history

Status: active. Amends nothing; supersedes nothing. Amended by RR-0054
(sub-answer 2's loading, constraint 3). Refines
RR-0049's "first-party JavaScript out of scope" for this one file.

- 2026-11-18: roll-back applied to MB-0066, CRP-0053, AGENTS.md and templating.md (PR #130).
- 2026-11-19: amended by RR-0054 (MB-0066).
