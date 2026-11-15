# Mission Brief — Collapsible categories on the favorites page, collapsed by default

```yaml
---
id: MB-0059
owner: "@juuso-kalliomaki"
goal: "The favorites page shows each category as a collapsible group, collapsed when the page opens, so a long catalog is a short list of category names that the user opens as needed."
non_goals:
  - "The shopping view's or a list's grouping by category."
  - "Reordering categories (that stays on `/categories`)."
  - "Remembering open groups across visits or devices (RR-0044: not wanted)."
  - "JavaScript: native `<details>`/`<summary>`, as MB-0040's navigation and the item edit sections use."
acceptance:
  - id: P1
    property: "On `/favorites`, each category group (and the Uncategorized group, if any) is a `<details>` whose `<summary>` is the category name with its count of products; on a full page load every group is closed."
    check: cargo test -p kauppa-web routes::products::tests::catalog::groups_collapsed_by_default
  - id: P2
    property: "An action whose effect stays inside one group (↑/↓, a Save that keeps the category, a delete that leaves products in the group) replaces only that group, rendered open, so other groups keep their state; an action that moves a product between groups or adds or removes a group (a category change, 'No category' included; add; make favorite; deleting a group's last product) replaces the whole catalog with the touched groups open (`HX-Retarget`); without htmx every action answers the full page with the touched groups open; an error answers the whole catalog with the touched group open (RR-0044)."
    check: cargo test -p kauppa-web routes::products::tests::catalog::group_swaps
  - id: P3
    property: "'From recipes' is one `<details>`, closed on a full page load, its category groups shown inside as headings (not collapsible); an action inside it that stays inside it replaces only the section, rendered open; 'Make favorite' replaces the whole catalog with the section and the target group open (RR-0044)."
    check: cargo test -p kauppa-web routes::products::tests::hidden::collapsible
  - id: P4
    property: "Keyboard and screen reader: each summary is reachable by Tab and toggles with Enter/Space, its name is announced with its state; the layout works at 320 px and on desktop in both themes."
    check: "manual: browser check (keyboard, screen reader, phone and desktop widths, light and dark), recorded in MB-0059's Continuity Pack"
  - id: P5
    property: "No new JavaScript, no inline styles (CSP), every new string in both catalogs; existing tests change only where they asserted the old markup."
    check: cargo test -p kauppa-web --all-targets
context:
  - templates/products/_catalog.hbs
  - templates/products/index.hbs
  - assets/app.css
  - crates/kauppa-web/src/routes/products/mod.rs
  - crates/kauppa-web/src/routes/products/tests/
  - design-system/README.md
  - .agentic/missions/MB-0040-mobile-navigation-panel.md
  - .agentic/mentorship/templating.md
autonomy:
  may_decide:
    - "markup and styles, inside the design system"
    - "how the server marks the touched group open (a field in the catalog view)"
    - "test names inside `routes::products::tests` (record renames here)"
  escalate:
    - { when: "after an action, the other groups: all closed except the touched one, or keep those the user had open (needs the open set sent with each form, or a cookie) (decided: RR-0044, per-group swaps)", to: owner }
    - { when: "the 'From recipes' section: collapse each of its groups, or the whole section as one, and its default (decided: RR-0044, one collapsed section)", to: owner }
    - { when: "whether the add form's choice of category opens that group after adding (P2 covers it as 'touched'), and whether an empty category group is shown at all (decided: RR-0044, it opens; empty categories stay hidden)", to: owner }
  forbidden:
    - "JavaScript or inline scripts/styles"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
resolutions: [RR-0044]
---
```

## Context

Requested by the owner (2026-11-15): "Favorites view should have the
categories be collapsible (default to collapsed)."

Today `templates/products/_catalog.hbs` renders each category as a
`<section class="product-group">` with an `<h2>` and every product row
expanded, then the 'From recipes' section the same way. With many
favorites the page is long on a phone.

Every action on the page (add, edit, ↑/↓, delete, make favorite) swaps the
whole `#favorites` fragment. A `<details>` that the user opened is closed
again by that swap unless the server renders it open, which is why P2 asks
the server to open the group the action touched.

No trigger path is touched (`routes/products/` and templates are not in
`check_triggers.py`), so only the owner's behaviour questions need a
consult.

Related: MB-0058 edits `_product_edit.hbs` inside these groups; whichever
lands second rebases.

## Conceptual plan

1. **Clarify (S1).** Owner questions on the other groups, 'From recipes',
   and the add form. Done: CRP-0044 answered, RR-0044 approved (per-group
   swaps; one collapsed 'From recipes' section).
2. Tests first (P1–P3); the view model's open flag; the template and CSS.
3. Browser check (P4).

## Validation plan

- Template and route tests over full pages and fragments after each
  action.
- The owner's browser check, keyboard and screen reader included.
