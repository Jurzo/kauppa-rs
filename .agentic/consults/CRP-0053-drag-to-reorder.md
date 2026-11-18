# Consultation Request Pack — Drag to reorder: vendoring Sortable.js, the first first-party script, the ↑/↓ buttons, and the order routes

```yaml
---
id: CRP-0053
mission: MB-0066
owner: "@juuso-kalliomaki"
target_role: platform
risk_tier: medium
urgency: low
state: open
runbook_trigger: "MB-0066 autonomy.escalate (vendoring Sortable.js, first-party JavaScript → platform-owner; the ↑/↓ buttons and the order routes → owner; any CSP change → security); a new vendored file under `assets/vendor/` (RR-0015 constraint 1, AGENTS.md do-not-touch boundaries); an AGENTS.md and templating.md change (AGENTS.md change procedure); feature.md escalation trigger `csp.rs` (only if Q5 changed the CSP; recommended: no change)"
---
```

## Decision statement

How does drag-to-reorder reach the browser under `script-src 'self'`:
which Sortable.js (Q1), where the few lines that connect it to htmx live
and which pages load them (Q2), whether the ↑/↓ buttons stay (Q3), how
the new order routes work (Q4), and is the CSP left as it is (Q5)?

## Current state

- Three ↑/↓ reorderings, each a small form per row posting `dir=up|down`:
  `POST /categories/{id}/move`, `POST /favorites/{id}/move` (within its
  category), `POST /recipes/{id}/ingredients/{ingredient}/move`. They work
  without JavaScript, and htmx swaps the section in place.
- Client code: three vendored, pinned files (htmx 2.0.11, response-targets
  2.0.4, SSE 2.2.4), loaded on every page from `layout.hbs` with SRI. No
  first-party JavaScript: AGENTS.md and `templating.md` allow nothing
  executable under `assets/` outside `assets/vendor/`; RR-0049 left
  first-party JavaScript out of scope.
- CSP (`csp.rs`): `default-src 'self'; script-src 'self'; style-src
  'self'; …`; htmx runs with `allowEval`, `allowScriptTags` and
  `includeIndicatorStyles` off (RR-0019).
- htmx's Sortable example (https://htmx.org/examples/sortable/) wraps the
  rows in a `<form hx-trigger="end">` and connects Sortable with an inline
  `htmx.onLoad(…)` script. The inline script is blocked by our CSP, and our
  rows already contain forms (rename, move, delete), which can't sit inside
  another form.

## Q1 — Sortable.js (platform)

### Option A — Sortable 1.15.7, `Sortable.min.js` from the npm registry, byte-for-byte (recommended)

Vendored as `assets/vendor/Sortable-1.15.7.min.js`, pinned in
`templates/tests.rs` by length, SHA-256 and SRI like the other three.

- Pros:
  - The library htmx's example uses; handles mouse and touch (its own
    fallback where native drag-and-drop is unreliable: iOS, Chrome for
    Android).
  - No `eval`, no `new Function`, no `<style>` element; styles are set
    through `element.style` (see the evidence), which `style-src` doesn't
    govern.
  - Its `group` option is what the follow-up (dragging between favorite
    categories) needs.
- Cons:
  - 45,478 bytes (htmx is 52,182) on the pages that load it (Q2).
  - Released 2026-02-11; the previous release was 2024-11-28: maintained,
    but slowly.
- Risk: low.

### Option B — No library: native HTML5 drag-and-drop in our own script

- Cons: touch support is uneven (Sortable itself disables native
  drag-and-drop on iOS and Chrome for Android); more first-party code to
  review.

### Option C — Don't add dragging

- Cons: the owner's request isn't met.

**Recommended: A.**

## Q2 — The connecting code, and which pages load it (platform)

### Option A — One first-party file, `assets/app/sortable.js`, loaded only on the three pages (recommended)

- **The file.** About 20 lines: `htmx.onLoad` creates a `Sortable` on each
  element marked `data-sortable` (on page load and after every htmx swap),
  with `handle: ".drag-handle"` so touching a row elsewhere still scrolls
  the page (MB-0066 P7). Sortable's `end` event triggers the order form.
- **The form.** Rows keep their own forms. The order form sits beside the
  list (`<form id="…-order" hx-post="…/order" hx-trigger="end from:…">`),
  and each row carries `<input type="hidden" name="id" form="…-order">`.
  htmx builds the body with `new FormData(form)`, which includes
  form-associated inputs in document order, so after a drop the ids arrive
  in the new order (checked in htmx 2.0.11's source).
- **Loading.** A layout parameter (`page_sortable=true`, RR-0006's `page_`
  prefix) adds both `<script defer>` tags, with SRI, on the categories,
  favorites and recipe pages only. Other pages are unchanged.
- **The rule it changes.** AGENTS.md (do-not-touch boundaries) and
  `templating.md` gain: first-party scripts may live under `assets/app/`
  only, as files approved by an RR; no inline code, no `eval`, no
  `innerHTML`; a test lists the allowed files, so a new one fails until
  it's approved. The file isn't pinned by checksum: it's our code, reviewed
  in the PR like any other.

- Pros:
  - Keeps `script-src 'self'` and no inline scripts.
  - The smallest first-party surface that works; the follow-up extends the
    same file.
- Cons:
  - The project's first first-party JavaScript, a standing precedent.
  - Two more requests on those three pages (cached: `/assets` is the only
    path with long-lived caching, RR-0029).

### Option B — The same file, loaded on every page from the layout

- Pros: one place in `layout.hbs`, like the other scripts.
- Cons: every page pays 45 KB for three pages' feature (AGENTS.md goal 2:
  a slow phone over 3G).

**Recommended: A.**

## Q3 — The ↑/↓ buttons (owner)

### Option A — Keep them, beside a drag handle (recommended)

- Pros:
  - Without JavaScript and by keyboard, reordering works as today (MB-0066
    P4; AGENTS.md goal 1, keyboard-friendly). Sortable has no keyboard
    support.
  - The move routes and their tests stay unchanged.
- Cons: two ways to do one thing on each row; the handle takes a little
  room on a phone.

### Option B — Hide them when JavaScript runs

- Cons: keyboard users lose reordering.

### Option C — Remove them and retire the move routes

- Cons: users without JavaScript and keyboard users lose reordering.

**Recommended: A.** The handle (a "⠿" glyph with an accessible name, e.g.
"Drag to reorder ‹name›") is hidden without JavaScript, since it does
nothing there.

## Q4 — The order routes (owner)

### Option A — One route per list, the full order, an exact-set check (recommended)

- `POST /categories/order`, `POST /favorites/order` (with the category's id
  as a field), `POST /recipes/{id}/ingredients/order`; the body is `id`
  repeated, in the new order.
- **Saved only if exact:** the ids must be exactly the user's current rows
  of that list: none missing, extra, repeated or foreign. Inside one
  `BEGIN IMMEDIATE` transaction, through the repo traits
  (`CategoryRepo::reorder` exists; `ProductRepo` and `RecipeRepo` gain one
  method each).
- **A stale order** (a row added or removed in another tab) or a tampered
  one saves nothing; the response is the section with the current order,
  so the page snaps back. A recipe or category that isn't the user's is 404,
  as the move routes.
- Without htmx (no JavaScript means no drag, so only a hand-made request):
  a redirect back to the page.
- The cross-site check covers the new routes.

- Pros: one request per drop; the server never applies a partial or mixed
  order.
- Cons: three new routes and two repo methods.

### Option B — Reuse the move routes: one step per drop (`to=<index>`)

- Cons: an index is ambiguous when the list changed elsewhere; no way to
  tell a stale drop from a real one.

**Recommended: A.**

## Q5 — The CSP (security)

### Option A — No change (recommended)

Stays `script-src 'self'; style-src 'self'`. If the browser suite's CSP
check reports a violation on any flow, the mission stops (MB-0066's
slow-mode trigger) instead of loosening it.

**Recommended: A.**

## Recommendation and question

Recommended: **Q1 A, Q2 A, Q3 A, Q4 A, Q5 A.**

Because: dragging comes from a pinned, widely used library and one small
first-party file under a new, narrow rule, without touching the CSP; the
buttons keep reordering working without JavaScript and by keyboard; and a
drop is saved only as a complete, current order.

Crisp questions to answer:
- **Q1:** Sortable 1.15.7 (A), our own drag-and-drop (B), or none (C)?
- **Q2:** `assets/app/sortable.js` on the three pages, with the AGENTS.md
  and `templating.md` rule above (A), or on every page (B)?
- **Q3:** keep ↑/↓ beside a handle (A), hide them with JavaScript (B), or
  remove them (C)?
- **Q4:** full-order routes with an exact-set check (A), or one step per
  drop (B)?
- **Q5:** no CSP change (A)?

## Blast radius and rollback

What can break: the categories, favorites and recipe pages (their
markup, the new scripts); the three order routes.
Who is affected: every user on those pages.
Rollback plan: revert the PR (the vendored file, the first-party file, the
templates and routes); the ↑/↓ routes are unchanged, so reordering keeps
working. The AGENTS.md and `templating.md` amendment is reverted with its
roll-back commit. No schema change.

## Minimal context

- MB-0066 (accepted 2026-11-18), its acceptance P1–P7.
- `templates/categories/_section.hbs`, `templates/products/_group.hbs`,
  `templates/recipes/_ingredients.hbs`, `templates/layout.hbs`.
- `crates/kauppa-web/src/csp.rs`, `crates/kauppa-web/src/templates/tests.rs`
  (`assert_vendored`).
- RR-0006 (layout `page_` parameters), RR-0015 and RR-0017 (vendoring),
  RR-0019 (CSP), RR-0029 (asset caching), RR-0049 (first-party JavaScript
  out of scope).
- The follow-up (dragging a favorite into another category) is not decided
  here; it builds on Q1 A's `group` option and Q2 A's file.

## Evidence bundle (Hardened)

- **Supply chain (npm registry, 2026-11-18):** `sortablejs`, maintainers
  `rubaxa` and `owenm`; latest 1.15.7 (2026-02-11), before it 1.15.6
  (2024-11-28); license MIT (`package.json` and `LICENSE`); no
  dependencies.
- **The tarball** `sortablejs-1.15.7.tgz`: its sha512 matches the
  registry's `dist.integrity`
  (`Kk8wLQPl…ET7u6ux0A==`).
- **`Sortable.min.js`:** 45,478 bytes; SHA-256
  `bf4241bc73fef7f11c59a283a69fe8051cdd31c6d8ff5a2b9ba219e7831fcf76`;
  SRI `sha384-DgmC6Xe2bSN2WjTDXzWYbUbxyhNP+NNkGDR/g78pCXV7E7rcVTGxVg0uIVCUUcBc`.
- **CSP scan of `Sortable.min.js`:** no `eval`, `new Function`,
  `innerHTML`, `insertRule` or `<style>` creation. One `style.cssText`
  write, on a detached test element (`pointer-events` feature check); style
  changes go through `element.style` (CSSOM), which `style-src` doesn't
  block. It dispatches its events with `CustomEvent`, so `hx-trigger="end
  from:…"` sees them. The browser suite's CSP check confirms this in the
  implementation.
- **htmx 2.0.11** serializes a form with `new FormData(form)`, which
  includes inputs associated through the `form` attribute, in document
  order.
