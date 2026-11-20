# Mission Brief — Compact rows with icon buttons on the categories, favorites and ingredients lists

```yaml
---
id: MB-0068
owner: "@juuso-kalliomaki"
goal: "On a phone, each row of the categories page, a favorites group and a recipe's ingredients fits on one line: drag handle, name, the ↑/↓ buttons stacked in a column, and icon buttons (pencil, trash) in place of today's text buttons, as in the owner's mock-up of 2026-11-19."
non_goals:
  - "Changing a route, a form's fields, what an action does, or the order behaviour (MB-0066, RR-0053, RR-0054)."
  - "Other lists (the shopping list, the lists index, the recipes index, hidden products), or the add forms."
  - "New JavaScript; a CSP change."
  - "Dragging a favorite into another category (a separate follow-up)."
acceptance:
  - id: P1
    property: "At a 360 px wide viewport, every row of the categories page shows on one line, with no horizontal scrolling: the drag handle (with JavaScript), the name, ↑ above ↓ (each 44 × 30 px), a pencil button that opens the rename panel below the row (today's rename form), and a trash button that deletes the category with today's confirmation."
    check: "scripts/browser/run.sh (new flow: row layout at 360 px on the three lists)"
  - id: P2
    property: "A favorites group's rows and a recipe's ingredient rows are laid out the same way: the name, ↑ above ↓, a pencil button that opens the row's edit panel (today's 'Edit'), and a trash button that deletes the row (moved out of the edit panel) with a confirmation: today's for favorites, a new 'Remove ‹name›?' for ingredients; the edit panel still opens below the row."
    check: "scripts/browser/run.sh (the same flow)"
  - id: P3
    property: "Each icon button has the accessible name its text gave it, with the row's name (for example 'Delete Voi'); the icons are hidden from assistive technology and not focusable; the keyboard order in a row is name, ↑, ↓, pencil, trash; every touch target is at least 44 × 44 px, except each stacked arrow, at least 44 × 30 px."
    check: cargo test -p kauppa-web templates::tests::icon_buttons
  - id: P4
    property: "Without JavaScript every action still works through the same forms; every string is in both catalogs; the templates stay CSP-compatible (no `style` attributes, no inline script); existing tests unchanged except where they find a button by its visible text (listed in the PR)."
    check: cargo test -p kauppa-web --all-targets
  - id: P5
    property: "On the owner's phone: the three lists read clearly, nothing wraps, and the buttons are easy to hit."
    check: "manual: owner's check on the deployed site, recorded in MB-0068's Continuity Pack"
context:
  - templates/categories/_section.hbs
  - templates/products/_group.hbs
  - templates/products/_product_edit.hbs
  - templates/recipes/_ingredients.hbs
  - templates/layout.hbs
  - assets/app.css
  - design-system/components/Button.md
  - design-system/components/ListItem.md
  - crates/kauppa-web/src/templates/tests.rs
  - tests/browser/flows/reorder.spec.js
  - .agentic/resolutions/RR-0034-design-system-fonts-icons-name.md
  - .agentic/resolutions/RR-0053-drag-to-reorder.md
autonomy:
  may_decide:
    - "the CSS (grid or flex) and class names, inside `assets/app.css`'s existing sections"
    - "a template partial per icon, so each SVG is written once"
    - "test names inside the modules above"
  escalate:
    - { when: "the touch-target size of the stacked arrows: each at least 44 px as `Button.md` and `app.css` require today (rows grow to about 88 px), or a smaller size such as 32 px (rows about 64 px, as in the mock-up) with the rule amended (RR-0055)", to: owner }
    - { when: "where the icons come from: drawn by hand as inline SVG in the templates (as the theme switch's three icons, MB-0055), or copied from an icon set (a licence and a pin, as RR-0034 did for the fonts) (RR-0055)", to: platform-owner }
    - { when: "moving delete out of the favorites and ingredients edit panels to the row, and whether an ingredient's delete (today 'Remove', without confirmation) gets one (RR-0055)", to: owner }
    - { when: "the pencil on the categories page: it saves the edited name (the mock-up) though a pencil usually means 'edit'; or a different icon (a tick) for saving (RR-0055)", to: owner }
    - { when: "a change to `design-system/` (an icon-button variant in `Button.md`, the row in `ListItem.md`) (RR-0055)", to: owner }
  forbidden:
    - "change a route, a form field or a handler"
    - "add JavaScript or `style` attributes"
    - "add a dependency or vendored file without an RR"
    - "remove a confirmation that exists today"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
resolutions: [RR-0055]
---
```

## Context

Requested by the owner (2026-11-19): "The categories view is too cramped on
a mobile width and not everything fits on one line … The view should be
clarified by swapping the text buttons into icon based ones and stacking the
arrows on top of each other. Recipe list section and Favorites list section
also suffer from similar issues and should be similarly reworked."

- **Today, at a phone width** (the owner's screenshot): a category row
  wraps into three lines: the drag handle alone; the name field and
  "Rename"; then ↑, ↓ and "Delete".
- **The mock-up** (the owner's image, dark theme): one line per row: the
  handle (⠿, muted), the name field, ↑ above ↓ (the disabled one muted),
  an orange pencil, an orange trash can; rows separated by a rule inside
  one card.
- **The rows** (`templates/categories/_section.hbs`,
  `templates/products/_group.hbs` with `_product_edit.hbs`,
  `templates/recipes/_ingredients.hbs`): each has the handle, the name (an
  input on the categories page, text elsewhere), a move form with ↑/↓
  (`aria-label`s "Move ‹name› up/down"), and either a rename form and a
  delete form (categories) or a `<details class="product-edit">` whose
  summary says "Edit" and whose panel holds the edit form and the delete
  ("Delete" for favorites, with a confirmation; "Remove" for ingredients,
  without one).
- **Rules that apply:** buttons are at least 44 px tall (`Button.md`,
  `app.css`); the CSP is `script-src 'self'; style-src 'self'`, so no
  `style` attributes; inline SVG is already used for the theme switch's
  icons (`layout.hbs`, `aria-hidden="true" focusable="false"`); every
  string goes through the catalogs (`{{t}}`).
- **Tests that find buttons by their visible text** (for example the
  browser flows' "Delete", "Edit") will look them up by accessible name
  instead; the names stay the same words.

## Conceptual plan

1. **Clarify (S1).** One CRP for the escalations: target size, icon
   source, delete placement and confirmation, the categories pencil, and
   the design-system docs.
2. Icons as partials; the three templates' rows rebuilt with the same
   forms (P2, P3, P4).
3. CSS: one row grid shared by the three lists; the arrows' column (P1).
4. Browser flow at 360 px; template test for the icon buttons (P1–P4).
5. The owner's phone check (P5).

Slow-mode triggers: a row that can't fit 360 px with the agreed target
size; an action that would need JavaScript to stay usable.

## Validation plan

- P1, P2: a Playwright flow at 360 × 800: per row, the controls' boxes
  share a line (their vertical centres within the row's box) and the page
  has no horizontal overflow; the actions still work.
- P3: a template test renders each list and checks each icon button's
  accessible name, `aria-hidden` icons and the order.
- P4: the full test suite and the no-JavaScript flows.
- P5: the owner's phone check.
