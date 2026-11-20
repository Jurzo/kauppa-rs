# Resolution Record — Compact rows: 44 × 30 px stacked arrows, hand-drawn inline SVG icons, delete on the row, the categories rename panel, and the design-system docs

```yaml
---
id: RR-0055
closes: CRP-0055
mission: MB-0068
decided_by: "@Jurzo"
role: product
risk_tier: low
decided_at: 2026-11-20t10:39:00z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0068-compact-reorder-rows.md
  - .agentic/consults/CRP-0055-compact-reorder-rows.md
---
```

## Decision and outcome

Approve CRP-0055's recommended set, **Q1 B, Q2 A, Q3 A, Q4 C, Q5 A**
(owner, 2026-11-20: "All as recommended").

1. **Stacked arrows (Q1 B, owner).** On the categories, favorites and
   ingredients rows, ↑ sits above ↓, each button 44 px wide and 30 px
   tall; a row is about 68 px. The handle, pencil and trash stay at least
   44 × 44 px.
2. **Icons (Q2 A, platform).** A pencil and a trash can, drawn by hand as
   inline SVG, one partial per icon under `templates/icons/`:
   `viewBox="0 0 24 24"`, `aria-hidden="true" focusable="false"`, a 2 px
   stroke in `currentColor` with round caps and joins, no fill
   (`design-system/README.md`, Iconography). The arrows stay the ↑/↓
   characters. No file under `assets/`, no third-party source.
3. **Delete on the row (Q3 A, owner).** In all three lists the trash
   button sits on the row and posts today's delete form (same route and
   fields).
   - Favorites keep today's confirmation text, including the
     "used in N recipes" warning; the visible warning stays in the edit
     panel.
   - Ingredients gain a confirmation, "Remove ‹name›?" (a new catalog
     string in each locale).
   - The edit panels keep only their edit forms.
4. **Categories (Q4 C, owner).** The category's name is shown as text;
   the pencil is the summary of a `<details>` panel below the row that
   holds today's rename form, unchanged (same route and field), as on the
   favorites and ingredients rows.
5. **Design-system docs (Q5 A, owner).**
   - `design-system/components/Button.md` gains an **Icon** variant:
     44 × 44 px, no visible label, an `aria-label` naming the action and
     its object, the icon `aria-hidden`; `brand` for edit, `danger` for
     delete; and the exception that a stacked ↑/↓ pair may be
     44 × 30 px each.
   - `design-system/components/ListItem.md` gains a **Reorderable row**
     paragraph: handle, name, ↑ above ↓, pencil, trash; the handle hidden
     without JavaScript; the edit panel opens below the row.

Covers:
- the five decisions above, in MB-0068's implementation PR;
- the roll-back below.

Does not cover:
- other lists (shopping list, lists index, recipes index, hidden
  products) or the add forms;
- an icon set, an icon font, or any file under `assets/`;
- a route, form-field or handler change, JavaScript, or a CSP change.

## Rationale and trade-offs

- At 360 px a row has 302 px for its contents; with the handle, the
  stacked arrows, the pencil, the trash and their gaps, about 102 px are
  left for the name (CRP-0055, width budget). Stacking the arrows is what
  makes one line fit.
- 30 px tall arrows keep rows near the mock-up's proportions and stay
  above WCAG 2.2's minimum target size (2.5.8, AA, 24 × 24 px). The
  project's 44 px rule is the stricter 2.5.5 (AAA); this is a documented
  exception for one pair, whose mis-tap costs one move, undone by the
  other arrow.
- Two hand-drawn icons need no licence, pin or request, and match the
  README's line.
- Delete on the row is one tap closer; the ingredient confirmation offsets
  how easy the trash is to hit beside the pencil.
- A text name with a pencil that opens a panel makes the pencil mean
  "edit" on all three lists, and long names wrap instead of hiding in a
  short field; renaming costs one more tap.
- Rejected: 44 × 44 stacked (Q1 A, rows about 96 px), side by side (Q1 C,
  doesn't fit); an icon set (Q2 B, process for two icons); no ingredient
  confirmation (Q3 B) or delete left in the panels (Q3 C); a pencil or a
  tick that saves an always-editable field (Q4 A, B); no docs change
  (Q5 B).

## Approvals and authority

@Jurzo as owner (product: Q1, Q3, Q4, Q5) and platform (Q2).

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0055-compact-reorder-rows.md` (its
  evidence bundle: the width budget, the 44 px rule, WCAG 2.5.5/2.5.8)
- Mission Brief: `.agentic/missions/MB-0068-compact-reorder-rows.md`
- Continuity Pack: `.agentic/continuity/MB-0068.md`
- Related: RR-0034 (design system, the ↑/↓ glyphs in the font subset),
  RR-0053 (drag handles and order forms, unchanged).

## Constraints and follow-on rules (Hardened)

1. **Same forms.** Every action posts the same route with the same fields
   as before; moving a form into or out of a `<details>` panel is the only
   structural change.
2. **Accessible names.** Each icon button's accessible name is the action
   and the row's name ("Delete Voi", "Edit Voi", "Move Voi up"); the SVG
   is `aria-hidden="true" focusable="false"`; no icon-only control without
   one.
3. **CSP unchanged.** No `style` attribute and no inline script; the
   icons' strokes and colours come from `app.css`.
4. **Only the stacked pair** may be under 44 px tall; every other button
   keeps the 44 × 44 px minimum.
5. **No confirmation is removed**; the ingredient one is added.

## Roll-back into the system (Hardened)

To be applied after this record is signed, with the owner's OK:

- `MB-0068`: `resolutions: [RR-0055]`;
  - P1: "the name field, ↑ above ↓, a pencil button that saves the name
    (today's 'Rename')" becomes "the name, ↑ above ↓ (each 44 × 30 px), a
    pencil button that opens the rename panel below the row";
  - P2: "(moved out of the edit panel, as the consult decides) with a
    confirmation" becomes "(moved out of the edit panel) with a
    confirmation: today's for favorites, a new 'Remove ‹name›?' for
    ingredients";
  - P3: "at least the size the consult decides" becomes "at least
    44 × 44 px, except each stacked arrow, at least 44 × 30 px";
  - the escalations RR-0055 answered get "(RR-0055)" appended.
- `CRP-0055`: `state: open` → `answered`.

## Supersession and history

Status: active. Supersedes nothing.

- 2026-11-20: roll-back applied to MB-0068 and CRP-0055 (PR #138).
