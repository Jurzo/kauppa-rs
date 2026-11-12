# Mission Brief — Select chevrons with room to breathe

```yaml
---
id: MB-0053
owner: "@juuso-kalliomaki"
goal: "Every `<select>` in the app shows its down chevron with comfortable space from the right edge, the same in every browser and in light and dark mode, by hiding the browser's default arrow and drawing the app's own."
non_goals:
  - "Replacing `<select>` with a custom widget or adding JavaScript."
  - "Restyling other form controls beyond what the selects need to match them."
  - "The language selector's chevron (MB-0051), which isn't a `<select>`."
acceptance:
  - id: P1
    property: "Every `<select>` the templates render (category picker, recipe picker, share role, list picker) hides the browser's arrow (`appearance: none`) and shows the app's chevron, with right padding wide enough that the selected text never runs under it."
    check: cargo test -p kauppa-web templates::tests::select_chevron
  - id: P2
    property: "The chevron follows the theme (light and dark) and needs no change to the Content-Security-Policy: no `data:` URL, no inline style, no script. If drawn from a file, the file is a same-origin asset covered by the asset tests."
    check: cargo test -p kauppa-web templates::tests::csp_compatible_templates
  - id: P3
    property: "The selects stay native: same names, labels, required attributes and keyboard behaviour; existing tests pass."
    check: cargo test -p kauppa-web --all-targets
  - id: P4
    property: "In a browser (a Chromium browser, Firefox, and Safari or a phone), at 320 px and on a desktop, in light and dark mode, each select shows one chevron (no double arrow) with visible space from the edge, and long option text doesn't overlap it."
    check: "manual: browser check on the release binary, recorded in MB-0053's Continuity Pack"
context:
  - assets/app.css
  - templates/products/_category_picker.hbs
  - templates/lists/_list.hbs
  - templates/lists/_share_form.hbs
  - templates/recipes/_add_to_list.hbs
  - crates/kauppa-web/src/csp.rs
  - crates/kauppa-web/src/templates/tests.rs
  - .agentic/resolutions/RR-0019-content-security-policy.md
  - .agentic/resolutions/RR-0031-language-dropdown.md
autonomy:
  may_decide:
    - "padding and chevron size"
    - "drawing the chevron with CSS only (gradients or borders on a wrapper's pseudo-element), so no new file is needed"
    - "test names inside `templates::tests` (record a rename here)"
  escalate:
    - { when: "drawing it from an SVG file under `assets/` (a new asset; RR-0031's flag rules show the pattern, but an icon file is a new kind of asset)", to: platform-owner }
    - { when: "wrapping each `<select>` in a new element in the templates, if CSS on the `<select>` alone can't draw it", to: owner }
    - { when: "any change to the Content-Security-Policy", to: security }
  forbidden:
    - "`data:` URLs or inline styles"
    - "JavaScript"
    - "removing the native `<select>`"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
resolutions: []
---
```

## Context

Requested by the owner (2026-11-12): "The downward chevron of the select
elements sits too close to the select element's edge. Some padding should be
added to it. The default chevron can be hidden and a custom one drawn in its
place."

`assets/app.css` styles the form selects
(`.add-product select`, `.product-edit select`) with `padding: 0.5rem`, a
border and radius, and leaves the browser's own arrow, which each browser
draws at its own place near the right edge. There are four `<select>`
elements: the category picker (`products/_category_picker.hbs`, used on the
list, favorites and recipe pages), the recipe picker on the list page, the
share role, and the list picker on the recipe page.

The usual fix is `appearance: none` plus a chevron drawn with a background
image and extra right padding. The CSP (RR-0019) has `img-src 'self'` with no
`data:`, so the common inline-SVG `data:` URL is not allowed. Options that
fit: an SVG file under `assets/` (same origin; MB-0051 added `image/svg+xml`
serving), or CSS-only drawing with gradients, which can also use the theme's
colour variables for dark mode (an SVG file in a background can't).

## Conceptual plan

1. Choose the drawing method: CSS-only if it looks right (within
   `may_decide`), otherwise consult on an SVG file.
2. Tests first (P1, P2), then the CSS.
3. Browser check (P4) by the owner.

## Validation plan

- P1: a test over `app.css` that every select rule sets `appearance: none`,
  a chevron and a right padding at least the chevron's width plus a gap, and
  over the templates that every `<select>` is covered by those rules.
- P2: the existing CSP template scan, and the asset tests if a file is
  added.
- P3: the full suite. P4: the owner's browser check.
