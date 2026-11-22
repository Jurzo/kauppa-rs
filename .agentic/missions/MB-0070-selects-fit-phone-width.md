# Mission Brief — Selects that fit a phone's width with long names

```yaml
---
id: MB-0070
owner: "@juuso-kalliomaki"
goal: "On a phone, a form with a `<select>` stays inside the page however long the names in it are. The select shrinks to the width of its form and cuts off a long name, so the page never scrolls sideways because a category, list or recipe has a long name."
non_goals:
  - "A length limit on category, list or recipe names (they have none today)."
  - "Replacing the native `<select>` with a custom control, or showing the full name some other way."
  - "Changing which options a picker offers, their order or their wording."
  - "Sideways overflow at 360 px that isn't caused by a select."
acceptance:
  - id: P1
    property: "At 360 × 800, with a category, a shopping list and a recipe each named 60 characters long, none of these pages scrolls sideways (the document is no wider than the viewport), and every select shown lies inside its form: the favorites add form; a favorite's edit panel; a recipe's add-to-list form; a list's add-recipe form; a list item's edit panel; a recipe's category prompt for a new ingredient; and the import review's category picker."
    check: "scripts/browser/run.sh (new flow: selects at 360 px with long names)"
  - id: P2
    property: "The fix is in `assets/app.css` only. At any name length a select can't be wider than the field around it, and the chevron rule (`templates::tests::select_chevron`) still holds: only `select` sets a select's appearance, background, padding or outline."
    check: cargo test -p kauppa-web templates::tests::select_fits
  - id: P3
    property: "At 1280 × 720 the same forms keep today's layout. A select is as wide as its longest option, up to its form's width, and the add forms' selects stay at least 10 rem wide (`.add-product select`)."
    check: "scripts/browser/run.sh (the same flow, at desktop width)"
  - id: P4
    property: "No template, route or catalog change, so the forms post as today with and without JavaScript; existing tests are unchanged."
    check: cargo test -p kauppa-web --all-targets
  - id: P5
    property: "On the owner's phone, with a long category name: the favorites page doesn't scroll sideways, and the picker still reads clearly."
    check: "manual: owner's check on the deployed site, recorded in MB-0070's Continuity Pack"
context:
  - assets/app.css
  - templates/products/_category_picker.hbs
  - templates/products/_catalog.hbs
  - templates/products/_product_form.hbs
  - templates/recipes/_add_to_list.hbs
  - templates/recipes/_ingredients.hbs
  - templates/lists/_forms.hbs
  - templates/lists/_items.hbs
  - templates/review/page.hbs
  - crates/kauppa-web/src/templates/tests.rs
  - tests/browser/flows/rows.spec.js
  - design-system/components/TextField.md
autonomy:
  may_decide:
    - "the CSS, inside `assets/app.css`'s existing form and select sections"
    - "whether a cut-off name ends in an ellipsis where the browser supports one"
    - "test names inside the modules above"
  escalate:
    - { when: "a note on selects' width in `design-system/components/TextField.md`, which doesn't mention selects today (decided by the owner on acceptance, 2026-11-22: add the note)", to: owner }
    - { when: "a form can't fit 360 px with its select at any width (its other fields need the room): a template change, a non-goal", to: owner }
  forbidden:
    - "change a template, a route, a handler or a catalog"
    - "set a select's appearance, background, padding or outline outside the `select` rule (`templates::tests::select_chevron`)"
    - "add JavaScript or `style` attributes"
runbook: .agentic/runbooks/bugfix.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
resolutions: []
---
```

## Context

Found while building MB-0068 (2026-11-20) and noted in its Continuity Pack
as a follow-up. The owner asked for a brief on 2026-11-22.

**Repro**, on main at `4d4c240`, measured on 2026-11-22 with a throwaway
Playwright probe (not committed):

- **Setup:** a 360 × 800 viewport, through `scripts/browser/run.sh`'s app.
  The viewport is 360 px wide and `main`'s padding leaves 12 px each side,
  so a form's right edge is at 348 px.
- **The favorites add form**, by the length of the longest category name:

  | Longest name | The select's right edge | Page width |
  |---|---|---|
  | 20 characters | 278 px | 360 px |
  | 30 characters | 348 px | 360 px |
  | 40 characters | 385 px | 385 px |
  | 60 characters | 532 px | 532 px |

- **Other forms, with 60-character names:**

  | Form | The select's right edge | Page width |
  |---|---|---|
  | A favorite's edit panel, open | 549 px | 549 px |
  | A recipe's add-to-list form (a list's name) | 522 px | 522 px |
  | A list's add-recipe form (a recipe's title) | 522 px | 522 px |
  | A list item's edit panel, open | 549 px | 549 px |

- **Not measured:** the recipe's category prompt and the import review
  page. They use the same label and select rules, and the new flow measures
  them.

**Cause** (`assets/app.css`):

- The labels in `.add-category`, `.add-product`, `.product-edit form` and
  `.row-edit form` are flex items (`flex: 1 1 10rem`) with the default
  `min-width: auto`, so a label is never narrower than its content.
  `.recipe-details label` is the same.
- A select's content width is its widest option. `select { min-width: 0 }`
  doesn't help, because the label holding it can't shrink.
- `.add-product select { min-width: 10rem; }` sets a floor, not a ceiling.

**What exists:**

- The select's look comes from one rule (MB-0053). Only it may set a
  select's appearance, background, padding or outline
  (`templates::tests::select_chevron`). Its right padding keeps the text
  clear of the chevron.
- On a phone, opening a select shows the browser's own menu, which shows
  each name in full. Only the closed select cuts a long name off.
- MB-0068's `rows.spec.js` already has a no-sideways-scroll check at 360 px
  (`expectNoSideScroll`) that lists the elements that stick out.

## Conceptual plan

1. **Red.** A browser flow at 360 px with 60-character names, failing on
   each form above (bugfix runbook, rule 2).
2. **CSS (P2).**
   - The labels that hold a select are capped at their form's width
     (`max-width: 100%`). A flex item's automatic minimum is its content's
     width, and the maximum caps that minimum, so the label still starts
     at its select's width (desktop unchanged, P3) but can't exceed its
     line. `min-width: 0` instead would let the label share a desktop line
     and cut the name there too.
   - The same for an open list item panel; the export review's `fieldset`
     gets `min-width: 0` (its default minimum is its content).
   - A select is never wider than its container (`max-width: 100%`), so it
     cuts the name off and keeps its padding and chevron.
3. **Desktop check (P3).** The same flow at 1280 px: selects as wide as
   their longest option, as today.
4. **The design-system note.** `TextField.md` gains a bullet on selects:
   the field's box, as wide as the longest option up to the form, a long
   name cut off on a phone (the owner's decision on acceptance,
   2026-11-22).
5. **The owner's phone check (P5).**

Slow-mode triggers:

- A form whose other fields, not its select, overflow 360 px.
- A browser that ignores `max-width` on a select.

## Validation plan

- **P1, P3:** a new flow (or a test in `rows.spec.js`).
  - It creates the long names, opens each form at 360 × 800, and checks
    the document width and each select's box.
  - It repeats at 1280 × 720 for the widths there.
  - Red before the CSS change, green after.
- **P2:** a template test reads `assets/app.css`. It checks that `select`
  sets `max-width: 100%`, and that every label rule holding a select lets
  it shrink. `select_chevron` passes unchanged.
- **P4:** the full test suite.
- **P5:** the owner's phone check.
