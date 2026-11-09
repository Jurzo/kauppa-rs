# Mission Brief — Store view: name first, quantity after it

```yaml
---
id: MB-0038
owner: "@juuso-kalliomaki"
goal: "In shopping mode each row reads as the item's name followed by its quantity in parentheses, in a lighter grey, e.g. \"Maito (2 l)\", so the eye finds the item first."
non_goals:
  - "Changing the list page's or recipes' quantity display."
  - "Parsing or normalising quantities."
acceptance:
  - id: P1
    property: "In the store view, for editors and viewers alike, an item with a quantity renders its name, then the quantity in parentheses inside an element styled muted (e.g. `<span class=\"shop-quantity\">(2 l)</span>`); an item without a quantity renders the name alone, with no empty parentheses."
    check: cargo test -p kauppa-web routes::lists::tests::shop::name_then_quantity
  - id: P2
    property: "Accessible labels are unchanged in meaning (\"Mark Maito found\", \"Remove Maito — not needed\"), and checking or removing an item still returns the updated store fragment."
    check: cargo test -p kauppa-web routes::lists::tests::shop
  - id: P3
    property: "The muted style keeps at least WCAG AA contrast (4.5:1) against the row background, in found and not-found rows."
    check: "manual: contrast of the chosen colour, recorded in MB-0038's Continuity Pack"
context:
  - templates/lists/_shop.hbs
  - assets/app.css
  - crates/kauppa-web/src/routes/lists/tests/shop.rs
autonomy:
  may_decide:
    - "class name and exact colour, within P3"
    - "test names inside `routes::lists::tests::shop`"
  escalate:
    - { when: "the owner meant literal braces `{2 l}` rather than parentheses", to: owner }
    - { when: "the same order should also apply on the list page", to: owner }
  forbidden:
    - "JavaScript"
    - "inline styles"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
resolutions: []
---
```

## Context

Requested by the owner (2026-11-09): "show the item name first and quantity
after it in braces and a bit grayed out". This brief reads "braces" as
parentheses, "Maito (2 l)"; the owner confirms when accepting the brief.
`templates/lists/_shop.hbs` renders
`<span class="ingredient-quantity">{{quantity}}</span> {{text}}` in two
places (editor button and read-only row). `.ingredient-quantity` is shared
with the list page and recipes, so the store view needs its own class.

## Conceptual plan

Template change in both branches of `_shop.hbs`, a new CSS rule, tests
first.

## Validation plan

- P1, P2: route tests on the store page and its fragment.
- P3: contrast checked against `app.css` colours.
