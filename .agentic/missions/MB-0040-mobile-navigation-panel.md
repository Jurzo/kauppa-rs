# Mission Brief — Mobile-friendly navigation panel

```yaml
---
id: MB-0040
owner: "@juuso-kalliomaki"
goal: "On a phone the site navigation lives in a side panel that opens from a menu button and closes again, instead of a row of links squeezed into the top bar, while desktop keeps quick access to every section."
non_goals:
  - "New pages or changes to which sections exist."
  - "Bottom tab bars or gestures (swipe to open)."
  - "JavaScript beyond htmx and native HTML features."
acceptance:
  - id: P1
    property: "The layout has a menu button in the top bar that opens a panel with every navigation entry (Lists, Recipes, Favorites, Categories, and Sign out or Sign in), and the panel can be closed by a close control, by Escape, and by choosing an entry (boosted navigation leaves it closed)."
    check: cargo test -p kauppa-web templates::tests::navigation_panel
  - id: P2
    property: "It works without JavaScript (native `<dialog>`, the `popover` attribute, `<details>`, or a CSS-only pattern, as the owner approves), with no inline script or `on*=` attribute, so MB-0031's CSP can apply."
    check: cargo test -p kauppa-web templates::tests::navigation_needs_no_script
  - id: P3
    property: "Accessibility: the button has an accessible name and exposes the panel's open state; focus moves into the panel when it opens and back to the button when it closes; the current section is marked with `aria-current=\"page\"`."
    check: "manual: keyboard and screen-reader check (e.g. Firefox and Android TalkBack), recorded in MB-0040's Continuity Pack"
  - id: P4
    property: "At 320 px wide no page scrolls horizontally and the top bar fits on one line; at desktop widths the navigation is either still in the panel or shown inline, per the owner's choice."
    check: "manual: browser check at 320, 390 and 1280 px, screenshots in MB-0040's Continuity Pack"
  - id: P5
    property: "Sign out stays a POST form protected by the cross-site check (MB-0018), and every existing route test passes."
    check: cargo test -p kauppa-web --all-targets
context:
  - templates/layout.hbs
  - assets/app.css
  - crates/kauppa-web/src/templates/tests.rs
  - .agentic/missions/MB-0031-content-security-policy.md
  - .agentic/mentorship/templating.md
autonomy:
  may_decide:
    - "markup and CSS within P1–P4; breakpoint values"
    - "where the current section comes from (a `page_` layout parameter, RR-0006)"
  escalate:
    - { when: "the pattern: native `<dialog>` or `popover` (opening needs `commandfor`/`popovertarget`, check browser support), `<details>`, or a CSS-only checkbox; and desktop inline vs panel", to: owner }
    - { when: "any JavaScript file, even under `assets/`", to: platform-owner }
    - { when: "the language switcher (MB-0035) or an admin link joins the panel", to: owner }
  forbidden:
    - "inline scripts or event-handler attributes"
    - "turning Sign out into a link (GET)"
    - "new dependencies"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
  - .agentic/mentorship/security.md@v1
resolutions: []
---
```

## Context

Requested by the owner (2026-11-09). `templates/layout.hbs` has a top bar
with the brand and a `<nav>` of four links plus a Sign out form (or Sign
in). On a narrow phone they wrap or crowd. `<body hx-boost="true">` swaps
the whole body on navigation, so a panel's open state resets on each page
change, which suits "close on choosing an entry".

Native options need no JavaScript: the `popover` attribute with
`popovertarget` (light dismiss and Escape built in), `<dialog>` opened by
invoker commands (`commandfor`, newer browsers), or `<details>`. MB-0031
(CSP) forbids inline script; MB-0035 (languages) may add a switcher here.

## Conceptual plan

1. Owner picks the pattern and the desktop behaviour.
2. Layout markup and CSS; `page_nav` (or similar) for `aria-current`.
3. Tests for the markup; manual accessibility and width checks.

## Validation plan

- P1, P2: template tests over the rendered layout.
- P3, P4: manual checks with screenshots.
- P5: the full suite.
