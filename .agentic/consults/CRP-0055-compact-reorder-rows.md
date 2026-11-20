# Consultation Request Pack — Compact rows: arrow size, icon source, delete on the row, the categories pencil, and the design-system docs

```yaml
---
id: CRP-0055
mission: MB-0068
owner: "@juuso-kalliomaki"
target_role: product
risk_tier: low
urgency: low
state: answered
runbook_trigger: "MB-0068 autonomy.escalate (the stacked arrows' target size, delete placement and ingredient confirmation, the categories pencil, design-system/ changes → owner; the icon source → platform-owner)"
---
```

## Decision statement

How does a row on the categories page, in a favorites group and in a recipe's
ingredients fit one line at 360 px: how large are the stacked ↑/↓ buttons
(Q1), where do the pencil and trash icons come from (Q2), does delete move
onto the row and does an ingredient's delete get a confirmation (Q3), what
does the categories page's pencil do (Q4), and which design-system docs
change (Q5)?

## Current state

- **Rows today** (`templates/categories/_section.hbs`,
  `templates/products/_group.hbs` + `_product_edit.hbs`,
  `templates/recipes/_ingredients.hbs`), in a flex row that wraps
  (`app.css`, `.category-row, .product-row`):
  - categories: handle, a rename form (name field + "Rename"), a move form
    (↑, ↓), a delete form ("Delete", with `hx-confirm`);
  - favorites: handle, name, move form, `<details>` "Edit" holding the
    name-and-category form and "Delete" (with `hx-confirm` and, when
    recipes use the product, a visible warning);
  - ingredients: handle, quantity and name, move form, `<details>` "Edit"
    holding the quantity form and "Remove" (no confirmation).
- **Sizes:** every button is at least 44 × 44 px (`Button.md`: "Height at
  least 44px"; `app.css` sets `min-width`/`min-height: 2.75rem` on the row
  buttons and the handle). A list row is at least 48 px (`ListItem.md`).
- **Icons:** the design system has no icon set. `design-system/README.md`
  (Iconography) describes the line: "rounded caps and joins, about 2px
  stroke at 24px, no fills except small dots … Always pair icon-only
  buttons with an `aria-label`." The only icons in templates are the
  theme switch's three inline SVGs in `layout.hbs` (MB-0055), drawn by
  hand.
- **Confirmations** are `hx-confirm`, so they exist only with JavaScript;
  without it every delete is immediate today.

### The width budget at 360 px

The viewport minus `main`'s padding at phone width (12 px each side), the
list's border (1 px each side) and the row's padding (16 px each side)
leaves **302 px** for a row's contents.

| Item | Width |
|---|---|
| Drag handle (44 px with a −8 px margin) | 36 px |
| ↑/↓ column | 44 px |
| Pencil | 44 px |
| Trash | 44 px |
| Four gaps of 8 px | 32 px |
| **Left for the name** | **102 px** (118 px if the row padding drops to 8 px) |

Stacking the arrows saves 44 px of width over today's side by side;
it's what makes the line fit. The arrows' height decides the row's height
(Q1). Without JavaScript the handle is hidden and the name gets 36 px
more.

## Q1 — The stacked ↑/↓ buttons' size (owner)

### Option A — 44 × 44 px each, as today; rows about 96 px

- Pros: no rule changes; the most forgiving targets.
- Cons: each row is about twice today's single-line height, so the lists
  take about twice the scrolling; it doesn't look like the mock-up.

### Option B — 44 px wide, 30 px tall each; rows about 68 px (recommended)

- Pros:
  - Close to the mock-up's proportions.
  - Above WCAG 2.2's minimum target size (2.5.8, AA: 24 × 24 px). The
    44 px rule is the stricter AAA level (2.5.5), which this project
    adopted as its own rule.
  - The pencil, trash and handle stay 44 × 44 px; only the stacked pair
    is shorter, and each arrow is still 44 px wide.
- Cons:
  - Amends the 44 px rule for one case. `Button.md` would say "a stacked
    ↑/↓ pair may be 44 × 30 px" (see Q5).
  - Mis-taps between ↑ and ↓ are easier. Their only effect is a move by
    one place, which the other arrow undoes.
- Risk: low. The P5 phone check decides whether 30 px is enough. If it
  isn't, the fix is one CSS value.

### Option C — Side by side at 44 px, not stacked

- Cons: 44 px less width leaves the name about 58 px. Doesn't fit, and
  isn't what was asked.

**Recommended: B.**

## Q2 — Where the icons come from (platform)

Only two icons are needed: a pencil and a trash can, plus a tick if Q4 is
B. The arrows stay the ↑/↓ characters, which the served Inter subset
already covers (U+2191, U+2193, RR-0034).

### Option A — Drawn by hand as inline SVG, one partial per icon (recommended)

`templates/icons/pencil.hbs` and `templates/icons/trash.hbs` hold
`<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">` with a few
`<path>`s each, drawn to the README's line (2 px stroke, round caps and
joins, `currentColor`). Like the theme switch's icons.

- Pros:
  - No new file under `assets/`, no licence, no extra request; it works
    under the CSP as it is (inline SVG markup is not script or style).
  - The README's line style is matched exactly.
  - The partial means each icon is written once.
- Cons: hand-drawn icons may look less polished than a set's. Two simple
  shapes, though.

### Option B — Copied from an open-source set (for example Lucide, ISC licence)

- Pros: drawn by an icon designer; Lucide's style (24 px, 2 px stroke,
  round caps) is close to the README's.
- Cons: a third-party source to pin and credit (a `THIRD_PARTY_NOTICES.md`
  entry, a pinned version and the copied paths), as RR-0034 did for the
  fonts. More process for two icons.

**Recommended: A.**

## Q3 — Delete on the row, and a confirmation for ingredients (owner)

### Option A — Delete on the row in all three lists; ingredients get a confirmation (recommended)

- Favorites: the trash button posts today's delete form with today's
  `hx-confirm` text, including the "used in N recipes" warning. The
  visible warning stays in the edit panel.
- Ingredients: the trash button posts today's delete form, with a new
  `hx-confirm` ("Remove ‹name›?").
- The edit panel keeps only the edit form.
- Pros: matches the mock-up; one tap fewer to delete; consistent across
  the three lists. Moving delete next to the pencil makes it easier to
  hit by mistake, which the ingredient confirmation offsets.
- Cons: one new string in each catalog; one more tap to remove an
  ingredient with JavaScript.

### Option B — Delete on the row; ingredients stay without a confirmation

- Pros: removing an ingredient stays one tap; an ingredient is easy to
  re-add.
- Cons: a one-tap irreversible removal right beside the pencil, a 44 px
  target.

### Option C — Delete stays in the edit panels; the row has only the pencil

- Pros: no change to how delete works.
- Cons: favorites and ingredients differ from categories; not the mock-up.

**Recommended: A.**

## Q4 — The categories page's pencil (owner)

Today the name is always an editable field with "Rename" beside it.

### Option A — The name field stays; the pencil submits the rename (the mock-up)

- Pros: renaming stays one step: type, tap.
- Cons:
  - A pencil usually means "start editing", not "save". Here it would
    save, and the field next to it is already editable.
  - The field gets about 102 px: about 10 characters show before it
    scrolls ("Hedelmät ja vihannekset" won't show whole).

### Option B — The name field stays; a tick icon submits the rename

- Pros: a tick reads as "save", so the meaning is right.
- Cons: the field still gets about 102 px; it differs from the other two
  lists' pencil.

### Option C — The name is text; the pencil opens an edit panel below with the rename form (recommended)

The same pattern as favorites and ingredients: `<details>` with the
pencil as its summary; the panel holds today's rename form, unchanged
(same route and field).

- Pros:
  - The pencil means the same on all three lists.
  - The name is text, so a long name wraps instead of hiding in a short
    field.
  - No form changes; works without JavaScript (`<details>` is HTML).
- Cons: renaming takes one more tap (open the panel, then save). P1's
  wording changes from "the name field … a pencil button that saves the
  name" to "the name … a pencil button that opens the rename panel".

**Recommended: C.**

## Q5 — The design-system docs (owner)

### Option A — Document both (recommended)

- `Button.md` gains an **Icon** variant: 44 × 44 px, no visible label,
  an `aria-label` naming the action and its object ("Delete Voi"), the
  icon `aria-hidden`; `brand` for edit, `danger` for delete. With Q1 B, it
  also states the stacked ↑/↓ pair's 44 × 30 px exception.
- `ListItem.md` gains a **Reorderable row** paragraph: handle, name, ↑
  above ↓, pencil, trash; what's hidden without JavaScript; the edit panel
  opens below the row.

- Pros: the next list follows the docs, not this PR's markup.
- Cons: two docs edits in the PR to review.

### Option B — No docs change

- Cons: the 44 px exception would exist only in CSS; the docs and the
  code disagree.

**Recommended: A.**

## Recommendation and question

Recommended set: **Q1 B, Q2 A, Q3 A, Q4 C, Q5 A.**

Because it matches the mock-up's one-line rows at a height close to its
proportions, keeps every action working without JavaScript through
today's forms, adds no file or dependency, makes the pencil mean the same
thing on every list, and keeps the docs in step with the code.

Crisp question: approve Q1–Q5 as recommended, or name the options you
prefer?

## Blast radius and rollback

- **What can break:** the three lists' layout. A row could still wrap on
  a narrow phone or at a large text size (the browser flow checks 360 px
  at the default size). An action could stop working if a form loses an
  attribute while it's moved; the existing route tests and browser flows
  cover each action.
- **Who is affected:** every user of the categories, favorites and recipe
  pages.
- **Rollback:** revert the PR. Templates, CSS, two catalog strings and
  docs only; no data or route change.

## Minimal context

- MB-0068's goal and acceptance P1–P5; its forbidden list: no route,
  form-field or handler change, no JavaScript or `style` attributes, no
  dependency or vendored file without an RR, no confirmation removed.
- The rows' drag handles and order forms come from MB-0066 (RR-0053,
  RR-0054) and stay as they are.

## Evidence bundle (Hardened)

- Widths: `app.css` line 462 (`main` padding `space-4`) and 582
  (`space-3` under 480 px); 548–549 (row padding `space-4`, gap
  `space-2`); 555 and 563 (2.75 rem buttons and handle, the handle's
  `−space-2` margin).
- The 44 px rule: `design-system/components/Button.md` ("Height at least
  44px"); the 48 px row: `ListItem.md`.
- WCAG 2.2 success criteria 2.5.8 Target Size (Minimum), AA: 24 × 24 CSS
  px; 2.5.5 Target Size (Enhanced), AAA: 44 × 44 CSS px.
- Icon style: `design-system/README.md`, Iconography; precedent:
  `templates/layout.hbs` lines 85–95.
- The ↑/↓ glyphs in the font subset: RR-0034, sub-answer 1.

## Owner's answer (2026-11-20)

"All as recommended": Q1 B, Q2 A, Q3 A, Q4 C, Q5 A.

Recorded in RR-0055 (signed 2026-11-20).
