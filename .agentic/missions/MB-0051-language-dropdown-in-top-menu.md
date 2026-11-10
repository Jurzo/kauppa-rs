# Mission Brief — Language dropdown with flags in the top menu

```yaml
---
id: MB-0051
owner: "@juuso-kalliomaki"
goal: "The language is chosen from a dropdown in the top menu that shows each language's flag and name, with a short name on phones, replacing the two buttons in the footer, and it still works without JavaScript."
non_goals:
  - "Languages other than English and Finnish (adding one stays a catalog file, a flag and a `Language` variant)."
  - "Changing how the language is stored or resolved, the `kauppa_lang` cookie, or `POST /language` (RR-0023 sub-answers 3 to 5)."
  - "Country or region selection: a flag here stands for a language."
  - "Restyling the rest of the navigation."
acceptance:
  - id: P1
    property: "The top bar of every page, signed in or out, has one language selector: a closed control showing the current language's flag and name, which opens a list of every supported language, each with its flag and its name in that language, the current one marked as selected. The footer switcher is gone."
    check: cargo test -p kauppa-web templates::tests::language_dropdown
  - id: P2
    property: "Choosing a language works as today, without JavaScript: a plain `POST /language` with `lang` and the `return` path, behind the cross-site check, answered by a full page load so `<html lang>` updates. The existing language tests pass with only their markup expectations changed."
    check: cargo test -p kauppa-web i18n::tests::language_selection
  - id: P3
    property: "On a narrow screen the control and the list show a short name (`EN`, `FI`) beside the flag; on a wide screen the full name (`English`, `Suomi`). Both come from the catalogs, and the full name stays available to assistive technology at every width."
    check: cargo test -p kauppa-web templates::tests::language_dropdown_names
  - id: P4
    property: "The selector is accessible: the control has an accessible name that says it chooses the language, it and the options are reachable and operable by keyboard, flags are decorative (the name is the text), each option carries its own `lang`, and the list closes on Escape or after a choice."
    check: cargo test -p kauppa-web templates::tests::language_dropdown_accessible
  - id: P5
    property: "Flags are same-origin SVG files under `assets/` (the owner's choice, 2026-11-11), served as `image/svg+xml` and covered by the embedded-asset test; the Content-Security-Policy is unchanged (no `data:` images, no inline style or script), and no template gains literal text."
    check: cargo test -p kauppa-web routes::assets::tests::serves_every_asset
  - id: P6
    property: "In a browser at 320 px and on a desktop width, the selector fits the top bar beside the brand and the menu, opens and closes, doesn't overlap or get trapped by the navigation panel, and switching language works on a signed-out and a signed-in page."
    check: "manual: browser check on the release binary, recorded in MB-0051's Continuity Pack"
context:
  - templates/layout.hbs
  - assets/app.css
  - locales/en.json
  - locales/fi.json
  - crates/kauppa-web/build.rs
  - crates/kauppa-web/src/routes/assets.rs
  - crates/kauppa-web/src/routes/language.rs
  - crates/kauppa-web/src/i18n/tests.rs
  - crates/kauppa-web/src/templates/tests.rs
  - crates/kauppa-web/src/csp.rs
  - .agentic/resolutions/RR-0023-translations.md
  - .agentic/resolutions/RR-0019-content-security-policy.md
  - .agentic/mentorship/templating.md
autonomy:
  may_decide:
    - "class names, spacing and the breakpoint, reusing MB-0040's 40rem unless it doesn't fit"
    - "test names inside `templates::tests` (record a rename here)"
    - "catalog key names for the short names and the control's label"
  escalate:
    - { when: "moving the switcher out of the footer and changing its markup from two buttons (RR-0023 sub-answer 7 and its `aria-pressed` buttons): an RR amends it", to: owner }
    - { when: "the dropdown mechanism without JavaScript: `<details>`/`<summary>`, a `popover` list, or a native `<select>` with a submit button (a `<select>` can't show flag images in its options)", to: owner }
    - { when: "where it sits on a phone: in the top bar beside the menu button, or inside the navigation panel", to: owner }
    - { when: "which flag stands for English (the reference image uses the United Kingdom's) and for each later language", to: owner }
    - { when: "the SVG flag files: source, license and size; `svg` needs a content type in `build.rs` and a `SERVED_AS` row, and third-party files under `assets/` are a platform decision", to: platform-owner }
    - { when: "any change to the Content-Security-Policy, the `kauppa_lang` cookie or `POST /language`", to: security }
  forbidden:
    - "JavaScript beyond htmx, inline scripts, inline styles or `style=` attributes"
    - "`data:` image URLs, or loading flags from another origin"
    - "a GET that changes the language, or `?lang=` links"
    - "literal text in templates (every label through `{{t}}`)"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
  - .agentic/mentorship/security.md@v1
resolutions: []
---
```

## Context

Requested by the owner (2026-11-11), with a reference image: a dropdown
whose closed state shows a flag, the language name and a chevron, and whose
open list shows one row per language (flag, name in that language, a check
mark on the current one). The image also shows a compact variant with
two-letter codes (`EN`, `FR`), which the owner wants on phones.

Today (MB-0035, RR-0023 sub-answer 7) the switcher is a `<footer>` form on
every page: two submit buttons, `English` and `Suomi`, with
`aria-pressed`, posting to `/language` with `hx-boost="false"`. RR-0023
chose the footer to stay clear of MB-0040's navigation panel and because it
is reachable signed out. Moving it to the top menu and changing its shape
amends that sub-answer.

Constraints that shape the design:

- **No JavaScript** (templating.md; RR-0019 allows only same-origin
  scripts and the app adds none beyond htmx). The open/close behaviour must
  be native: `<details>`, or the `popover` attribute MB-0040 already uses
  for the navigation panel. Each option stays a submit button in the one
  POST form.
- **Images are same-origin only** (`img-src 'self'`, no `data:`). Flags are
  new SVG files under `assets/`. `build.rs` knows only `css`, `js` and
  `webmanifest`, and fails the build on another extension, so `svg` needs a
  content type there and a row in the asset test. The files are loaded with
  `<img>`, where a browser runs no script inside an SVG, and they should
  contain none.
- **Emoji flags were considered and not chosen** (owner, 2026-11-11):
  Chrome and Edge on Windows have no flag glyphs and show the letters
  ("GB", "FI") instead.
- **Flags stand for languages here, not countries.** English has no single
  flag; the reference uses the United Kingdom's. The name is always shown
  as text, so the flag is decoration.
- **The top bar is tight on a phone**: brand, menu button, and now the
  selector, at 320 px. The short name exists for this.
- Boosted navigation swaps `<body>`, so an open dropdown closes on
  navigation, as the menu panel does.

## Conceptual plan

1. **Clarify (S1).** One CRP to the owner (placement, mechanism, flags;
   amending RR-0023 sub-answer 7), with the asset question to
   platform-owner and a note to security that the endpoint, cookie and CSP
   don't change.
2. Tests first: the layout's selector markup (P1, P3, P4), the asset
   (P5); adjust the markup expectations in `language_selection` (P2).
3. Flag assets and the content type; the layout and `app.css`; catalog keys
   for short names and the control's label, in both languages.
4. Browser check (P6) by the owner.

Slow-mode trigger: the selector can't fit at 320 px beside the menu button
without hiding one of them.

## Validation plan

- P1, P3, P4: tests over the rendered layout in both languages.
- P2: the existing language route tests.
- P5: the embedded-asset test and the CSP template scan.
- P6: the owner's browser check at 320 px and desktop, signed in and out.
