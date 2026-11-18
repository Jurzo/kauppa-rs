# Mission Brief — Reorder by dragging (Sortable), where arrow buttons reorder today

```yaml
---
id: MB-0066
owner: "@juuso-kalliomaki"
goal: "Wherever a user reorders things with ↑/↓ buttons today (categories, favorites within a category, a recipe's ingredients), they can drag them into place instead, as in htmx's Sortable example (https://htmx.org/examples/sortable/), and the new order is saved in one request."
non_goals:
  - "Dragging a favorite into another category to change its category: the follow-up mission the owner named (2026-11-17), briefed separately."
  - "Reordering anything that can't be reordered today (shopping list items, lists, recipes)."
  - "Live updates of a reorder to other viewers (MB-0057 covers shopping lists only)."
  - "A schema change: the stored positions stay as they are."
acceptance:
  - id: P1
    property: "On the categories page, on each favorites category and on a recipe's ingredients, the rows can be dragged (mouse and touch) into a new order; dropping sends the whole new order in one request, and the page shows the saved order."
    check: "scripts/browser/run.sh (new flow: drag on each of the three pages, reload, the order holds)"
  - id: P2
    property: "One route per list takes the full order (`POST …/order`, or as the consult names it) and saves it only if it is exactly the user's current set of ids for that list (no missing, extra, duplicate or foreign id; otherwise nothing changes and the current order is shown again); the user's own rows only (404 otherwise), as the existing move routes."
    check: cargo test -p kauppa-web routes::categories::tests::order_saves_exact_set
  - id: P3
    property: "The same validation for favorites within one category and for a recipe's ingredients."
    check: cargo test -p kauppa-web routes::products::tests::order_within_category
  - id: P4
    property: "Without JavaScript, and with the keyboard, reordering still works (the ↑/↓ buttons stay, or a replacement the consult approves); the pages' accessible names don't regress."
    check: "scripts/browser/run.sh (flows with JavaScript off and by keyboard)"
  - id: P5
    property: "The client code is what RR-<consult> approves: Sortable.js vendored byte-for-byte under `assets/vendor/`, pinned by sha256, length and SRI in `templates/tests.rs`; the code that connects it to htmx in a file the RR allows (no inline script; the CSP stays `script-src 'self'`); the browser suite's CSP and console checks pass on every flow."
    check: cargo test -p kauppa-web templates::tests
  - id: P6
    property: "Every string in both catalogs; the cross-site check covers the new routes; existing tests unchanged except where the consult retires the move routes (listed in the PR)."
    check: cargo test -p kauppa-web --all-targets
  - id: P7
    property: "On a phone: dragging categories, favorites and ingredients works by touch without scrolling the page by accident."
    check: "manual: owner's check on the deployed site, recorded in MB-0066's Continuity Pack"
context:
  - templates/categories/_section.hbs
  - templates/products/_group.hbs
  - templates/recipes/_ingredients.hbs
  - templates/layout.hbs
  - crates/kauppa-web/src/routes/categories.rs
  - crates/kauppa-web/src/routes/products/mod.rs
  - crates/kauppa-web/src/routes/recipes/ingredients.rs
  - crates/kauppa-core/src/repo/mod.rs
  - crates/kauppa-web/src/csp.rs
  - crates/kauppa-web/src/templates/tests.rs
  - assets/vendor/
  - tests/browser/
  - .agentic/resolutions/RR-0015-vendor-htmx.md
  - .agentic/resolutions/RR-0018-boosted-form-errors.md
  - .agentic/resolutions/RR-0019-content-security-policy.md
  - .agentic/resolutions/RR-0049-live-list-updates.md
autonomy:
  may_decide:
    - "drag handle styling and wording, through catalog keys in both languages"
    - "test names inside the modules above"
  escalate:
    - { when: "vendoring Sortable.js: its version, source (the npm registry, byte-for-byte), license (MIT) and checksums (a new vendored file, RR-0015 constraint 1, AGENTS.md do-not-touch boundaries) (decided: RR-0053, Sortable 1.15.7, checksums in RR-0053)", to: platform-owner }
    - { when: "first-party JavaScript: htmx's example connects Sortable with an inline `htmx.onLoad` script, which the CSP blocks; a first-party script file under `assets/` is not allowed today (AGENTS.md), and RR-0049 left first-party JavaScript out of scope (decided: RR-0053, `assets/app/sortable.js` on three pages, AGENTS.md rule)", to: platform-owner }
    - { when: "keeping the ↑/↓ buttons (no-JavaScript and keyboard users) or retiring the move routes (decided: RR-0053, the buttons stay)", to: owner }
    - { when: "the new order routes: their shape, the exact-set check, and what a stale order does (decided: RR-0053, `POST …/order`, exact set, a stale order snaps back)", to: owner }
    - { when: "a change to the CSP or the security headers (RR-0019) (decided: RR-0053, no CSP change)", to: security }
  forbidden:
    - "inline scripts, `unsafe-inline`, `unsafe-eval`, nonces or hashes in the CSP"
    - "a vendored file without an approved RR"
    - "a schema change"
    - "lower lint strictness"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
  - .agentic/mentorship/security.md@v1
resolutions: [RR-0053]
---
```

## Context

Requested by the owner (2026-11-17): "Where items are ordered up and down
using arrow buttons, migrate the functionality to use **Sortable** instead
https://htmx.org/examples/sortable/. A follow-up on this is to allow
drag-and-drop between favorite item's categories which would change the
item's category as well."

What exists:

- **Three ↑/↓ reorderings**, each a form per row posting a direction:
  - categories: `POST /categories/{id}/move` (`templates/categories/_section.hbs`; `CategoryRepo::reorder` takes the full order);
  - favorites within a category: `POST /favorites/{id}/move` (`templates/products/_group.hbs`; `ProductRepo::move_within_category`);
  - a recipe's ingredients: `POST /recipes/{id}/ingredients/{ingredient}/move` (`templates/recipes/_ingredients.hbs`; `RecipeRepo::move_ingredient`).
- **htmx's Sortable example** wraps the rows in a `<form class="sortable"
  hx-post="/items" hx-trigger="end">` with a hidden input per row, and
  connects Sortable.js with an inline `htmx.onLoad(…)` script that creates
  a `Sortable` on each `.sortable` and turns it off during the request.
- **Client code today** is three vendored, pinned files (htmx 2.0.11,
  response-targets 2.0.4, SSE 2.2.4) and no first-party JavaScript. The CSP
  is `script-src 'self'` with no inline scripts; `AGENTS.md` allows
  executable files in `assets/` only under `assets/vendor/`. So this
  mission needs a platform decision on Sortable.js and on the few lines
  that connect it (a first-party file, or another way).
- **The follow-up** (dragging a favorite into another category, changing
  its category) builds on this mission's client code and is briefed
  separately once this one's consult has decided it.

## Conceptual plan

1. **Clarify (S1).** One CRP: the vendored file and the connecting code
   (platform); the ↑/↓ buttons and the order routes (owner); no CSP
   change (security). Done: CRP-0053 answered, RR-0053 approved (all A).
2. The order routes with their exact-set check (P2, P3).
3. Vendored file and connecting code, pinned (P5); the templates (P1,
   P4).
4. Browser flows (P1, P4); the owner's phone check (P7).

Slow-mode triggers: Sortable needs a CSP change; touch dragging scrolls
the page on a phone; the connecting code doesn't fit the approved file.

## Validation plan

- Route tests for each order route; browser flows with JavaScript, without
  it, and by keyboard; the owner's phone check.
