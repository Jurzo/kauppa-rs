# Mission Brief — Adopt the Black Steel design system and brand the app as kauppalista.app

```yaml
---
id: MB-0054
owner: "@juuso-kalliomaki"
goal: "The app looks like the design system in `design-system/` (Black Steel: its colours, type, spacing, shapes and app icon, in light and dark) and calls itself `kauppalista.app` everywhere a user sees its name, with no loss of function, no JavaScript added and the security headers as strict as they are now."
non_goals:
  - "Behaviour the component notes describe but the app doesn't have: a floating add button, toasts with Undo, swipe to delete, moving checked items after a delay, \"To get\" / \"In cart\" groups. This brief restyles what exists; each of those is its own brief."
  - "A theme chooser. The app keeps following the OS colour scheme; the tokens' `data-theme` selectors ship unused. A chooser needs somewhere to keep the choice (a cookie or account setting) and is its own brief."
  - "Renaming anything a user doesn't see: the repository, the crates, the binary, `KAUPPA_*` settings, cookie names, the database file, the deploy files."
  - "Registering, pointing or deploying the `kauppalista.app` domain (`KAUPPA_DOMAIN` is deployment configuration)."
  - "Offline support or a service worker. The manifest gains icons and the name, nothing more."
  - "Changing the design system itself, beyond corrections the owner asks for."
acceptance:
  - id: P1
    property: "Tokens: the stylesheet the app serves defines every colour, spacing, radius, shadow and font token of `design-system/tokens.json` with the same values, for light (default) and for dark (`prefers-color-scheme: dark`); the old variables (`--fg`, `--muted`, `--primary`, `--primary-fg`, `--error`, `--radius`) are gone; and outside the token definitions `app.css` has no raw colour (no hex, `rgb()` or named colour), only `var(--…)`."
    check: cargo test -p kauppa-web templates::tests::design_tokens
  - id: P2
    property: "Contrast: computed from the served tokens, every pair in the design system's contrast table meets its level in both themes (text pairs at least 4.5:1, `border-strong` on `surface` at least 3:1), and filled brand controls use `on-brand` for their text (so dark mode never puts white on orange)."
    check: cargo test -p kauppa-web templates::tests::design_contrast
  - id: P3
    property: "Components: buttons (primary, secondary, quiet, danger), text fields, selects and list rows follow `design-system/components/`: controls at least 44px tall, list rows at least 48px, input text 16px, control edges in `border-strong`, a 2px `focus` outline with 2px offset on every interactive element, checked items in `ink-subtle` with a line-through and a `brand` checkbox. At most one primary button per page. MB-0053's select chevron and MB-0040's and MB-0051's popovers keep their rules, in the new tokens."
    check: cargo test -p kauppa-web templates::tests::design_components
  - id: P4
    property: "Name: the page title suffix, the top bar brand, the home page heading and the manifest's `name` say `kauppalista.app`; the manifest's `short_name` and the `apple-mobile-web-app-title` meta say `kauppalista`; no user-visible `kauppa` on its own remains in a template, a catalog or the manifest. The brand in the top bar shows the app icon beside the name and still links to `/`."
    check: cargo test -p kauppa-web templates::tests::brand_name
  - id: P5
    property: "Icons: the layout links a favicon and an Apple touch icon, the manifest lists the 192 px, 512 px and maskable icons, and every linked file is served from `/assets/icons/` with the right content type and is byte for byte the file in `design-system/assets/` (checksums in the test). `favicon.svg` passes its own content test (RR-0034 sub-answer 3): a style block that only colours shapes, no `@import`, `url(`, script, handler or link. `theme-color` is the site's page background from `design-system/README.md` (`#fbf8f5` light, `#121212` dark), and the manifest's `theme_color` and `background_color` are the light one; the icon colours in `design-system/assets/README.md` are used for nothing but the icons."
    check: cargo test -p kauppa-web routes::assets::tests::brand_icons
  - id: P6
    property: "The Content-Security-Policy and the other security headers are unchanged (or changed only as an approved Resolution Record says), every page loads nothing from another origin, and the templates stay free of inline styles and scripts."
    check: cargo test -p kauppa-web templates::tests::csp_compatible_templates
  - id: P9
    property: "Fonts: `assets/fonts/` holds one WOFF2 file per family (Inter, Space Grotesk, JetBrains Mono), the Latin subset of the TTF in `design-system/fonts/` with every weight kept, served as `font/woff2` and pinned by size and sha256; `app.css` declares each with an `@font-face` rule whose `src` is that same-origin file and whose `font-display` is `swap`; nothing preloads them."
    check: cargo test -p kauppa-web routes::assets::tests::brand_fonts
  - id: P7
    property: "Nothing else changes: same routes, forms, field names, labels, translations and keyboard behaviour; the whole suite passes, with tests that pinned old variable names updated to the new tokens and no assertion about behaviour weakened."
    check: cargo test --workspace --all-targets
  - id: P8
    property: "In a browser (a Chromium browser, Firefox, and Safari or a phone), at 320 px and on a desktop, in light and dark: every page reads as Black Steel, nothing scrolls sideways, focus rings show, the tab shows the icon in both colour schemes, and \"Add to Home Screen\" on a phone gives the icon and the name `kauppalista.app`. The browser console shows no Content-Security-Policy violation."
    check: "manual: browser check on the release binary, with screenshots, recorded in MB-0054's Continuity Pack"
context:
  - design-system/README.md
  - design-system/tokens.json
  - design-system/tokens.css
  - design-system/components/Button.md
  - design-system/components/TextField.md
  - design-system/components/ListItem.md
  - design-system/assets/README.md
  - design-system/fonts/README.md
  - assets/app.css
  - assets/manifest.webmanifest
  - templates/layout.hbs
  - templates/index.hbs
  - crates/kauppa-web/build.rs
  - crates/kauppa-web/src/routes/assets.rs
  - crates/kauppa-web/src/routes/assets/tests.rs
  - crates/kauppa-web/src/templates/tests.rs
  - crates/kauppa-web/src/templates/tests/select.rs
  - crates/kauppa-web/src/csp.rs
  - .agentic/resolutions/RR-0019-content-security-policy.md
  - .agentic/resolutions/RR-0029-request-handling.md
  - .agentic/resolutions/RR-0031-language-dropdown.md
  - .agentic/resolutions/RR-0034-design-system-fonts-icons-name.md
  - .agentic/consults/CRP-0034-design-system-fonts-icons-name.md
  - THIRD_PARTY_NOTICES.md
autonomy:
  may_decide:
    - "how `app.css` is rewritten onto the tokens, and class names, as long as templates keep their structure and tests keep their meaning"
    - "sizes and spacing where the design system gives a range or says nothing, using its tokens"
    - "which PRs the work is split into (suggested: tokens and components; name and icons)"
    - "test names inside `templates::tests` and `routes::assets::tests` (record a rename here)"
    - "leaving out icon files nothing links to (the light 192/512 PNGs, `source/`)"
  escalate:
    - { when: "a font format, character set, weight range or family other than RR-0034 sub-answer 1's (WOFF2, Latin subset, the three families), or `build_fonts.py` can't reproduce the committed font files", to: platform-owner }
    - { when: "a file under `assets/` beyond the six icons and three fonts RR-0034 names, or another cache rule for them", to: platform-owner }
    - { when: "the browser check finds a browser that shows `favicon.svg` as a black square: switch to the fixed-colour icon as RR-0034 sub-answer 3 says, and tell the owner (no new consult)", to: owner }
    - { when: "any change to the Content-Security-Policy or to `csp.rs`", to: security }
    - { when: "a route outside `/assets/` (browsers ask for `/favicon.ico` on their own)", to: api-owner }
    - { when: "the name in any place or form RR-0034 sub-answer 5 doesn't name", to: owner }
    - { when: "a template needs new structure (a wrapper element, a moved control) to match a component, or a design rule conflicts with what the app does (more than one primary action on a page, orange used where the design forbids it)", to: owner }
    - { when: "a correction to a file under `design-system/`", to: owner }
  forbidden:
    - "loading anything from another origin (fonts, styles, images) without an approved RR"
    - "inline styles, inline scripts, `data:` URLs"
    - "new JavaScript, new dependencies"
    - "the Rust logo, Ferris or the word \"Rust\" in the app's branding (the design system's own rule)"
    - "changing behaviour, routes, translations' meaning or form fields under cover of the restyle"
    - "editing AGENTS.md, mentorship packs, runbooks or Resolution Records without the consult's RR"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
  - .agentic/mentorship/security.md@v1
resolutions: [RR-0034]
---
```

## Context

Requested by the owner (2026-11-13): "A *design-system* directory has been
added to the repository. A mission brief needs to be created for adopting
the design system as well as branding the app as `kauppalista.app`."

**The design system** (`design-system/`, committed with this brief) is
"Black Steel": a checklist-basket icon in orange on a steel tile, and a
theme built from it. At the owner's direction (2026-11-13) it was fitted to
this project before it was committed: the Angular notes and the Google Fonts
link are gone, the READMEs name `kauppalista.app`, `/assets/` paths and
`templates/layout.hbs`, and `build_icons.py` writes into its own directory.
No token value and no icon file changed. It holds:

- `tokens.json` (the source) and `tokens.css` ("generated from tokens.json";
  the generator isn't in the directory): colours for light and dark,
  spacing on a 4px base, radii, shadows, three font stacks, type classes;
- `README.md`: principles, colour usage, a checked contrast table,
  typography, and how to wire it up in `app.css` and the layout;
- `components/`: Button, TextField, ListItem;
- `assets/`: `favicon.svg` (adaptive), `favicon.ico`, PNG favicons, an Apple
  touch icon, 192/512 and maskable PWA icons, light variants, and `source/`
  with the SVG sources and `build_icons.py`.

The owner also settled which colours are whose (2026-11-13): the site,
including `theme-color`, uses the colours in `design-system/README.md`; the
table in `design-system/assets/README.md` describes the icons only.

**The app today.** `assets/app.css` (320 lines) has its own small variable
set (`--bg`, `--fg`, `--muted`, `--border`, `--primary`, `--primary-fg`,
`--error`, `--radius`), blue as the primary colour, a system font stack and
12 raw hex values. Dark mode follows the OS. The name `kauppa` shows in
three places: the title suffix and the top bar brand in
`templates/layout.hbs`, and the heading of `templates/index.hbs`; the
manifest also says `kauppa`, with `theme_color` `#1f6feb` and no icons.
There is no favicon.

**What stands in the way.** Four things in the design system don't fit the
app as it is, and each belongs to an owner rather than to the agent:

1. *Fonts.* The design names three families (Inter, Space Grotesk,
   JetBrains Mono). The owner supplied the files (2026-11-13), now in
   `design-system/fonts/` with their OFL licences and a notice in
   `THIRD_PARTY_NOTICES.md`: one upright variable TTF per family, 1.2 MB
   together (Inter is 875 kB). The policy in `csp.rs` is
   `default-src 'self'; …` (RR-0019), which already lets fonts load from
   the app's own origin, so self-hosting needs no policy change. It does
   need font files under `assets/` (a new kind of asset and a content type
   in `build.rs`), and a choice of format: 1.2 MB is a lot for a slow
   phone, and WOFF2 or a cut-down character set would shrink it (none of
   the three reserves its font name, so the licence allows converting).
2. *Icons as assets.* AGENTS.md allows vendored JS under `assets/vendor/`
   and the flag SVGs under `assets/flags/`, each pinned by checksum.
   `build.rs` knows `css`, `js`, `svg` and `webmanifest` and fails the build
   on anything else, so PNG and ICO need rows there, and the assets rule in
   AGENTS.md needs a sentence (a roll-back from an RR, as RR-0031's was).
3. *The adaptive favicon.* `favicon.svg` changes colours with an inline
   `<style>` and a media query. Asset responses carry the app's
   Content-Security-Policy, and `style-src 'self'` may stop a browser from
   applying a style block inside an SVG it loads as an image; this has to
   be seen in a browser, not assumed. It also differs from the flags, which
   a test keeps free of `<style>`.
4. *Behaviour in the component notes.* ListItem and Button describe a
   floating add button, Undo toasts, swipe to delete and "To get" / "In
   cart" groups. Those are features, listed under non-goals.

Existing tests pin some CSS: `templates/tests/select.rs` requires the
chevron to use `var(--muted)`, and the navigation and language tests read
`app.css`. They move to the new token names (P7); their meaning stays.

**The name.** `kauppalista.app` replaces `kauppa` where users see it. `design-system/assets/README.md` shows the `<head>` links and the manifest's
icon entries with `/assets/` paths.

## Conceptual plan

1. **Consult first.** One Consultation Request Pack for the `escalate`
   items that need an owner's decision before any code: fonts, the new asset
   kinds and their rule in AGENTS.md, the adaptive favicon, where the tokens
   live, the name's form and `short_name`. It closes in one Resolution
   Record. Answered by RR-0034.
2. **Tokens and components** (tests first: P1, P2, P3). Replace the variable
   block with the tokens; restyle buttons, fields, selects, list rows, cards,
   the top bar, the popovers and banners from the component notes; move the
   pinned tests to the new names.
3. **Name, icons and fonts** (tests first: P4, P5, P9). Layout, home page,
   manifest, the icon files, `build_fonts.py` and the font files, their
   content types and checksums.
4. **Browser check** (P8) by the owner, from a checklist in the Continuity
   Pack.

Checkpoints: after the RR; after each of steps 2 and 3 (gates green, pack
updated).

Slow-mode triggers: a component rule that can't be met without changing a
template's structure; a contrast pair that fails once computed; a page with
two primary actions; anything that would touch `csp.rs`; the favicon not
switching in a browser.

Open questions: the `escalate` list. None is answered by this brief.

## Validation plan

- P1: a test that reads `design-system/tokens.json` and the served CSS and
  compares token by token, per theme; and scans `app.css` outside the token
  block for raw colours and for the old variable names.
- P2: a test that computes WCAG contrast from the served tokens for the
  pairs in the README's table, and checks that rules filling with `--brand`
  set their text to `--on-brand`.
- P3: a test over `app.css` and the rendered templates for the minimum
  sizes, the focus rule, the control edges and the checked-item style, and
  that each page renders at most one primary button. `select_chevron`,
  `navigation_panel` and the `language_dropdown` tests keep passing.
- P4: a test over the rendered layout and home page (both languages), the
  catalogs and the manifest.
- P5: a test that every icon the layout and manifest reference is served,
  with its content type and its sha256 equal to the file in
  `design-system/assets/`; plus the existing `serves_every_asset` and
  `cache_headers`.
- P9: a test that each font file is served as `font/woff2` with its pinned
  size and sha256, and that `app.css` has the three `@font-face` rules with
  same-origin `src` and `font-display: swap`. Re-running `build_fonts.py`
  and comparing checksums is a manual step, recorded in the Continuity Pack.
- P6: the existing CSP template scan and `csp::tests`, unchanged.
- P7: the full suite and the fast gate.
- P8: the owner's browser check, with screenshots at 320 px and desktop in
  both themes.
