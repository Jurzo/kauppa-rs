# Mission Brief — Drag a favorite into another category

```yaml
---
id: MB-0071
owner: "@juuso-kalliomaki"
goal: "On the favorites page, a user can drag a favorite by its handle from one category group into another, and drop it where it should go. That changes its category and saves its place in the new group in one request, as the owner asked when requesting drag-to-reorder (2026-11-17)."
non_goals:
  - "Dragging between other lists: ingredients into another recipe, categories, shopping list items."
  - "Dragging recipe products (the \"From recipes\" section) into a group, or a favorite out to that section; \"Make favorite\" and Delete stay as today."
  - "Changing how the groups collapse (RR-0044), beyond opening a closed group while a held row rests on its heading (the owner's decision, 2026-11-22)."
  - "A schema change: the stored category and positions stay as they are."
  - "Live updates of the move to other viewers."
acceptance:
  - id: P1
    property: "With JavaScript, dragging a favorite's handle into another group's list on `/favorites` moves it there, at the place it was dropped. The favorite's category changes, both groups show their saved order, and the move holds after a reload."
    check: "scripts/browser/run.sh (new flow: drag a favorite into another category, reload)"
  - id: P2
    property: "One request saves the move atomically: the favorite's new category, and the target group's full new order. It is saved only if the order is exactly the target group's favorites plus the moved one, and the favorite and both categories are the user's. Otherwise nothing changes and the catalog is shown as it is. Another user's favorite or category is a 404, as for the existing order route."
    check: cargo test -p kauppa-web routes::products::tests::move_between_groups
  - id: P3
    property: "The repo saves the category and the target group's order in one transaction (hidden products keep their slots, as `set_favorite_order` does), and a rejected move changes nothing."
    check: cargo test -p kauppa-core repo::product::move_between_groups
  - id: P4
    property: "The response replaces the catalog with both groups open (RR-0044 item 1: a move between units). Dragging within one group still reorders through `/favorites/order` as today."
    check: "scripts/browser/run.sh (the same flow, and reorder.spec.js's assertions unchanged)"
  - id: P5
    property: "Without JavaScript and by keyboard, a favorite still changes category through its edit panel's category picker (MB-0058), unchanged. Recipe products can't be dragged into a group. The browser suite's CSP and console checks pass."
    check: "scripts/browser/run.sh (the new flow without JavaScript; the fixtures' CSP and console watch)"
  - id: P6
    property: "Every string is in both catalogs, the cross-site check covers any new route, and existing tests are unchanged except where the route changes (listed in the PR)."
    check: cargo test -p kauppa-web --all-targets
  - id: P7
    property: "On the owner's phone: dragging a favorite by touch into another category works, including into a group further down the page."
    check: "manual: owner's check on the deployed site, recorded in MB-0071's Continuity Pack"
context:
  - templates/products/_group.hbs
  - templates/products/_catalog.hbs
  - assets/app/sortable.js
  - crates/kauppa-web/src/routes/products/mod.rs
  - crates/kauppa-web/src/routes/request.rs
  - crates/kauppa-core/src/repo/mod.rs
  - crates/kauppa-core/src/repo/sqlite/product.rs
  - crates/kauppa-web/src/templates/tests.rs
  - tests/browser/flows/reorder.spec.js
  - .agentic/missions/MB-0066-drag-to-reorder.md
  - .agentic/resolutions/RR-0043-favorite-one-save.md
  - .agentic/resolutions/RR-0044-favorite-categories-collapsible.md
  - .agentic/resolutions/RR-0046-favorite-category-requirement-amendment.md
  - .agentic/resolutions/RR-0053-drag-to-reorder.md
  - .agentic/resolutions/RR-0054-sortable-on-every-page.md
autonomy:
  may_decide:
    - "the repo method's name and the order of its statements, inside one transaction"
    - "drag styling (the drop place, the dragged row), in `assets/app.css`'s reorder section"
    - "test names inside the modules above"
  escalate:
    - { when: "dropping into a closed group (groups are closed on a full page load, RR-0044): only open groups take a drop; a drop on a closed group's heading adds the favorite at its end; or a closed group opens while a row is held over its heading (decided by the owner on acceptance, 2026-11-22: it opens while a held row rests on its heading)", to: owner }
    - { when: "the route: extend `POST /favorites/order` with the moved favorite, or a new route; what it carries (the favorite, the target category, the target order, the source order or not); and what a stale move does (the owner, 2026-11-22: decided in implementation; chosen: `POST /favorites/order` gains an optional `moved`, recorded in the Continuity Pack and the PR)", to: owner }
    - { when: "dropping into Uncategorized, which clears the category (RR-0046 lets the owner clear it), or not offering Uncategorized as a target (decided by the owner on acceptance, 2026-11-22: allowed)", to: owner }
    - { when: "changing `assets/app/sortable.js` so rows can move between lists (Sortable's `group` option) and the drop posts the move: RR-0053 approved the file for reordering within one list (decided by the owner on acceptance, 2026-11-22: updated as necessary)", to: platform-owner }
    - { when: "a confirmation before a drop changes a category (none today for reordering; the move can be dragged back) (decided by the owner on acceptance, 2026-11-22: no confirmation)", to: owner }
  forbidden:
    - "a schema change"
    - "inline scripts, a CSP change, or a new script file without an approved RR (RR-0053 constraint 2)"
    - "a vendored file change (Sortable stays 1.15.7, RR-0053)"
    - "remove the edit panel's category picker or the ↑/↓ buttons"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
  - .agentic/mentorship/security.md@v1
  - .agentic/mentorship/persistence.md@v1
resolutions: []
---
```

## Context

Requested by the owner when asking for drag-to-reorder (2026-11-17): "A
follow-up on this is to allow drag-and-drop between favorite item's
categories which would change the item's category as well." MB-0066 left
it out as a non-goal, to be briefed once its consult had decided the client
code. The owner asked for this brief on 2026-11-22.

**The favorites page today:**

- Each category is a `<details class="product-group catalog-unit">`,
  closed on a full page load and open after an action that touched it
  (RR-0044).
- Its rows are in `<ul id="group-…-list" data-sortable>`, beside an order
  form (`#group-…-order`). Each row's hidden `id` input joins that form
  through its `form` attribute.
- Sortable's `end` event triggers the form, which posts
  `POST /favorites/order` with `category` (an id, or `none` for
  Uncategorized) and the ids in their new order (MB-0066, RR-0053
  sub-answer 4).
- **Recipe products** sit in a separate closed "From recipes" section, with
  no drag handle.

**Changing a category today:**

- The edit panel's form saves the name and the category together
  (`POST /favorites/{id}`, `ProductRepo::update`, MB-0058, RR-0043). It
  works without JavaScript.
- A moved favorite is added at the end of its new group.
- "No category" clears it (RR-0046).
- The response is chosen by `choose_swap`: a move between groups replaces
  the catalog with both groups open (RR-0044 item 1).

**The order rule:**

- `ProductRepo::set_favorite_order` saves a group's order only if the ids
  are exactly that group's favorites; hidden products keep their slots.
- No method today moves a favorite into another group and orders that
  group in one transaction. `update` followed by `set_favorite_order` would
  be two transactions.

**The client code:**

- `assets/app/sortable.js` (RR-0053) creates one Sortable per
  `[data-sortable]` list, with `handle: ".drag-handle"` and
  `draggable: "li"`. Lists aren't grouped, so a row can't leave its list.
- Sortable's `group` option lets rows move between lists that share a
  group name. A move between lists fires `end` on the source list, with
  `from`, `to` and the new index.
- With only that option added, the moved row's hidden input would still
  belong to the source group's form, through its `form` attribute. The
  source form would post the source group's own set, with the moved id at
  its new place in the document. The exact-set check would pass, the
  category wouldn't change, and the target group's form wouldn't be
  triggered.
- So the script, the forms or the route have to change; the consult
  decides which.

**Moving on a phone:** on a phone a group further down the page is off
screen. Whether Sortable 1.15.7 scrolls the page while a row is held near
the edge is a known unknown, for P7 and the flow.

## Conceptual plan

1. **Clarify (S1).** Done on acceptance (2026-11-22), without a CRP: a
   closed group opens while a held row rests on its heading; the route is
   decided in implementation; Uncategorized takes a drop; no
   confirmation; `sortable.js` changes as needed. Opening a group from
   script departs from RR-0044 constraint 1 ("No JavaScript"); raised with
   the owner for a Resolution Record before merge.
2. **Repo (P3).** One transaction: check the favorite and the target are
   the user's, set the category, then save the target group's order with
   the exact-set check.
3. **The route and its swap (P2, P4).**
4. **Client (P1).** The script change the RR approves; the group lists
   share a Sortable group; the drop posts the move.
5. **Browser flow (P1, P4, P5); the owner's phone check (P7).**

Slow-mode triggers:

- The drop can't be posted without inline script, a CSP change or a new
  script file.
- Touch dragging into a group further down scrolls the page by accident or
  can't reach it.
- The move needs a schema change.

## Validation plan

- **P2, P3:** route and repo tests: the move saved; stale (order not the
  exact set); a foreign favorite or category (404); a recipe product
  refused; nothing changed on a rejection.
- **P1, P4, P5:** a Playwright flow that drags a favorite into another open
  group (and into a closed one, opened by hovering), checks the order
  request and both groups, and reloads. Without JavaScript, the edit
  panel's picker moves it. The fixtures' CSP and console watch stays on.
- **P6:** the full test suite.
- **P7:** the owner's phone check.
