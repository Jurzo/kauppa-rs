# Mission Brief — Light/dark theme switch in the top bar, and a top bar that never wraps

```yaml
---
id: MB-0055
owner: "@juuso-kalliomaki"
goal: "A user picks System, Light or Dark from a switch in the top bar; the page changes at once, the choice is kept between visits and works without JavaScript, and at every width the top bar stays on one line: the navigation moves into the menu panel before its entries would wrap."
non_goals:
  - "Keeping the theme on the account (a column, so a migration); this brief keeps it in a cookie, as the language is for signed-out users. Moving it to the account is its own brief."
  - "Themes other than the design system's light and dark, or changing any token value."
  - "Changing the language selector's behaviour, the `kauppa_lang` cookie or `POST /language` (MB-0051, RR-0031), beyond where it sits in the top bar."
  - "Restyling the menu panel itself (MB-0040), beyond holding the theme switch on phones if the owner chooses that."
  - "Theming the app icons or the manifest's colours by the chosen theme."
acceptance:
  - id: P1
    property: "Every page, signed in or out, has one theme switch in the top bar: a form posting to `/theme` with a `<fieldset>` and `<legend>`, three radio buttons `theme=system|light|dark`, each with an icon and a label, and the saved choice checked (no cookie, or a cookie with any other value, checks System). Labels and the legend come from the catalogs; the icons are decorative inline SVG with no `style` attribute."
    check: cargo test -p kauppa-web templates::tests::theme_switch
  - id: P2
    property: "`POST /theme`, behind the cross-site check, accepts only `system`, `light` or `dark` (anything else is a 400 and sets nothing), stores the choice in a cookie, and answers an htmx request with 204 and no body, and a plain form post with 303 to the `return` path (through `same_site_path`). No GET changes the theme."
    check: cargo test -p kauppa-web routes::theme::tests
  - id: P3
    property: "First paint is already in the chosen theme: the server renders the saved choice's radio `checked`, and `app.css` applies the theme from it (`:has()`), so nothing is rendered on `<html>`; the `theme-color` meta is the chosen theme's page background (`#fbf8f5` or `#121212`), or the two media-keyed metas for System. A boosted navigation keeps the choice."
    check: cargo test -p kauppa-web templates::tests::theme_first_paint
  - id: P4
    property: "Choosing a theme repaints the page at once, before the response arrives, with no JavaScript beyond htmx and no `hx-on`: by CSS keyed on the checked radio (`:has()`, RR-0035 sub-answer 3). Choosing System, after Light or Dark was saved, goes back to the OS setting on the same page."
    check: cargo test -p kauppa-web templates::tests::theme_switch_applies
  - id: P5
    property: "The switch is accessible: the group's accessible name says it chooses the colour theme, each option's label stays available to assistive technology at every width (an icon-only look hides it visually only), the radios are operable with the keyboard as native radios are (arrow keys move and choose), the focus ring shows, and the selected option is distinguishable by more than colour."
    check: cargo test -p kauppa-web templates::tests::theme_switch_accessible
  - id: P6
    property: "The top bar never wraps: there is one breakpoint, shared by the top bar's rules, below which the navigation is the menu panel and above which brand, navigation, language selector and theme switch sit on one line in either language, signed in or out. Where popover is supported, the inline navigation doesn't wrap (`flex-wrap: nowrap`); without it, it keeps wrapping. The breakpoint is 60rem unless the real markup measures wider, in which case it is the widest measured one-line width (Finnish, signed out, measured widest in CRP-0035) plus a margin, and the measurements are recorded in the Continuity Pack."
    check: cargo test -p kauppa-web templates::tests::top_bar_breakpoint
  - id: P7
    property: "On a phone, the theme switch sits inside the menu panel, as a headed group of three labelled rows; on wide screens the same switch shows inline as icon-only segments (RR-0035 sub-answers 4, 5). At 320 px the top bar still holds brand, language selector and menu button on one line, none hidden (RR-0031 sub-answer 4)."
    check: cargo test -p kauppa-web templates::tests::theme_switch_on_phones
  - id: P8
    property: "The Content-Security-Policy and the other security headers are unchanged; templates stay free of inline scripts, inline styles, `style=` and `hx-on`; no template gains literal text, and the English and Finnish catalogs have the same keys."
    check: cargo test -p kauppa-web templates::tests::csp_compatible_templates
  - id: P9
    property: "Nothing else changes: same routes besides `POST /theme`, same forms, translations and keyboard behaviour; the whole suite passes, with no behaviour assertion weakened."
    check: cargo test --workspace --all-targets
  - id: P10
    property: "In a browser (a Chromium browser, Firefox, and Safari or a phone), signed in and out, in English and Finnish: switching among System, Light and Dark repaints at once and survives a reload and a boosted navigation; with JavaScript off the form still saves; dragging the window from 1440 px down to 320 px, the top bar never takes a second line and the menu button takes over at the breakpoint; the brand icon matches the page's theme. The console shows no Content-Security-Policy violation."
    check: "manual: browser check on the release binary, with screenshots, recorded in MB-0055's Continuity Pack"
context:
  - templates/layout.hbs
  - assets/app.css
  - design-system/README.md
  - design-system/tokens.css
  - locales/en.json
  - locales/fi.json
  - crates/kauppa-web/src/routes/language.rs
  - crates/kauppa-web/src/i18n/request.rs
  - crates/kauppa-web/src/state.rs
  - crates/kauppa-web/src/csp.rs
  - crates/kauppa-web/src/templates/tests.rs
  - .agentic/missions/MB-0040-mobile-navigation-panel.md
  - .agentic/missions/MB-0051-language-dropdown-in-top-menu.md
  - .agentic/missions/MB-0054-design-system-and-branding.md
  - .agentic/resolutions/RR-0019-content-security-policy.md
  - .agentic/resolutions/RR-0023-translations.md
  - .agentic/resolutions/RR-0031-language-dropdown.md
  - .agentic/resolutions/RR-0034-design-system-fonts-icons-name.md
  - .agentic/mentorship/templating.md
  - .agentic/mentorship/security.md
autonomy:
  may_decide:
    - "class names, spacing and the exact breakpoint, by P6's rule (measure, take the widest, add a margin)"
    - "whether the language selector's short-name switch keeps MB-0040's 40rem or follows the new breakpoint, whichever keeps the bar on one line"
    - "test names inside `templates::tests` and `routes::theme::tests` (record a rename here)"
    - "catalog key names for the legend and the three labels"
    - "updating `design-system/README.md`'s theming paragraph (\"only the OS setting decides\") to describe the switch"
    - "which PRs the work is split into (suggested: top bar first, then the switch)"
  escalate:
    - { when: "the measured breakpoint is above 64rem (tablets in landscape would get the menu panel), or the bar can't fit on one line at any width without dropping something", to: owner }
    - { when: "keeping the choice on the account as well (a schema change)", to: data-owner }
    - { when: "the brand icon (`favicon.svg` through `<img>`) follows the OS instead of the chosen theme, and fixing it needs a different icon file under `assets/`", to: platform-owner }
    - { when: "any change to the Content-Security-Policy, the htmx config, or a new file under `assets/` that isn't CSS", to: security }
    - { when: "a change to the `kauppa_theme` cookie or `POST /theme` beyond RR-0035 sub-answers 1 and 2", to: security }
  forbidden:
    - "`hx-on`, inline scripts, inline styles or `style=` attributes, or JavaScript beyond the vendored htmx"
    - "a GET that changes the theme, or `?theme=` links"
    - "literal text in templates (every label through `{{t}}`)"
    - "edit `migrations/` or add a column without an approved RR"
    - "add dependencies"
    - "lower lint strictness"
    - "change token values in `design-system/` or `app.css`"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
  - .agentic/mentorship/security.md@v1
resolutions: [RR-0035]
---
```

## Context

Requested by the owner (2026-11-13): a dark/light mode switch in the top
nav, with the top nav revised at the same time. Today, at widths from about
750 px down to the 40rem (640 px) breakpoint, the inline navigation wraps
onto a second line before the menu button takes over. The owner supplied a
baseline for the switch (`theme-toggle.html`, `theme-toggle.css`); both are
copied verbatim in the appendix below, since the originals are outside the
repository.

**What exists.** MB-0054 put the Black Steel tokens in `assets/app.css`:
light by default, dark under `prefers-color-scheme: dark` unless
`<html data-theme="light">`, and dark under `<html data-theme="dark">`.
Nothing sets `data-theme` yet; `design-system/README.md` says it is there
"for a later theme choice (an attribute the server would render on
`<html>`)". This is that choice. The `theme-color` metas are keyed on the
OS scheme only.

**The top bar today** (`templates/layout.hbs`, `app.css` → "Site
navigation"): brand (`margin-right: auto`), the `<nav>` (a popover panel
below 40rem, inline above it, with `flex-wrap: wrap`), then the language
selector. On phones `order` puts the menu button last. The wrap comes from
`flex-wrap: wrap` plus a breakpoint chosen before the brand grew (MB-0054)
and the language selector arrived (MB-0051). The switch adds one more item,
so the breakpoint has to come from measurement, not a guess. CSS can't
detect wrapping, so the fix is a breakpoint wide enough for the widest
bar, plus `nowrap` so that a too-low value overflows visibly in testing
instead of wrapping quietly. Finnish labels are longer (`Kategoriat`,
`Kirjaudu sisään`); measured, Finnish signed out is the widest bar
(CRP-0035 Evidence bundle).

**The baseline and this app's rules.** The baseline is a radio group in a
form: htmx posts on change (`hx-swap="none"`), a `<noscript>` Save button
covers no-JS, and the server marks the saved choice `checked`. Most of it
fits as is. Three parts don't:

- `hx-on:change="…"` is inline JavaScript. The htmx config in the layout
  sets `allowEval: false`, and the CSP (`script-src 'self'`, RR-0019) has no
  `'unsafe-eval'`, so it would not run. Replaced by CSS (RR-0035
  sub-answer 3). `app.css` applies dark from
  `:root:has(<dark radio>:checked)`, and from the OS unless the light
  radio is checked; light stays on `:root`. The checked radio repaints the
  page with no script, and since the server renders the saved radio
  `checked`, that also covers first paint. `data-theme` is dropped from
  `app.css`: a value rendered on `<html>` would go stale after a boosted
  navigation (htmx swaps `<body>`, not `<html>`). Browsers without `:has()`
  follow the OS.
- `name="return_to"`: the language form uses `return` and `same_site_path`.
  Reuse both.
- The `<noscript>` button: the app's no-JS story is "the form still
  submits". A `<noscript>` element is fine under the CSP. It needs a catalog
  label.

**Cookie.** The language cookie (`i18n::language_cookie`) is the model:
same-site, `Secure` per `cookie_secure`, long-lived. No script reads the
theme cookie, so it can be `HttpOnly`. A new cookie and a new route are
security-tier (AGENTS.md decision rights), so they go through a consult and
an RR first.

**`theme-color`.** After an htmx-only change, the head isn't swapped, so
the browser bar colour updates on the next full load. That's acceptable
unless the owner says otherwise. Note it in the Continuity Pack.

**Brand icon.** The top bar shows `favicon.svg` through `<img>`. Its colours
follow `prefers-color-scheme`, which inside an image browsers resolve from
the embedding page's `color-scheme`. `app.css` sets that per theme, so the
icon should follow the choice. P10 checks it.

**Builds on MB-0054.** Its tokens, name, icons and fonts are all on `main`
(PRs #83, #84, #85), and the owner passed its browser check (2026-11-13).
The top bar described here is the one on `main` after those merges.

## Conceptual plan

1. **Clarify (S1).** One CRP. To security and api-owner: the cookie and
   `POST /theme`. To the owner: the repaint mechanism (recommend CSS
   `:has()`) and where the switch sits on phones (recommend inside the menu
   panel with labels, since the 320 px bar is already full with brand,
   language and menu button). Answered by RR-0035 (CRP-0035, all as
   recommended).
2. **Top bar first (P6).** Measure the one-line width of the bar in both
   languages, signed in and out, with a placeholder the size of the compact
   switch. Set the breakpoint, make the inline nav `nowrap`, and check that
   320 px is unchanged. This can ship alone.
3. **Tests first** for P1 to P5, P7 and P8, then the route and cookie, the
   layout, `app.css` and the catalogs.
4. **Browser check (P10)** by the owner.

Slow-mode triggers: the measured breakpoint goes past 64rem; the `:has()`
selector lists make the token blocks unreadable, or force duplicating
values that a test can't keep in sync; the 320 px bar can't hold its three
items with the switch anywhere in it.

## Validation plan

- P1, P3, P5, P7: tests over the rendered layout in both languages, signed
  in and out, with each cookie value (none, `system`, `light`, `dark`,
  garbage).
- P2: route tests (htmx and plain posts, a bad value, a cross-site post,
  a hostile `return`).
- P4, P6: tests over the served `app.css` (the selectors that apply each
  theme; one shared breakpoint value; `nowrap` on the inline nav). The real
  proof is P10.
- P8: the CSP template scan and the catalog key-parity test.
- P9: the whole suite.
- P10: the owner's browser check, with screenshots and the measured widths
  in the Continuity Pack.

## Appendix: the owner's baseline (verbatim)

`theme-toggle.html`:

```html
<!--
  Theme toggle: HTML + HTMX, Black Steel tokens.

  The server renders this fragment and marks the saved choice as `checked`
  (read from the `theme` cookie; no cookie = System).

  - htmx posts the choice on change; the server stores it in a cookie and answers 204.
  - hx-on applies it instantly on the client, so nothing waits for the round trip.
  - Without JavaScript, the form still works: the <noscript> button submits it
    and the server redirects back to `return_to`.

  For the icon-only app-bar version, add the class `is-compact` to the <form>.
-->
<form class="theme-toggle" method="post" action="/theme"
      hx-post="/theme" hx-trigger="change" hx-swap="none"
      hx-on:change="const t = event.target.value; t === 'system' ? document.documentElement.removeAttribute('data-theme') : document.documentElement.setAttribute('data-theme', t)">
  <!-- Server: fill in the current path so no-JS submits land back here. -->
  <input type="hidden" name="return_to" value="/settings">
  <fieldset>
    <legend>Colour theme</legend>

    <label class="theme-toggle__option" title="System">
      <input type="radio" name="theme" value="system" checked>
      <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8M12 16v4"/></svg>
      <span class="theme-toggle__label">System</span>
    </label>

    <label class="theme-toggle__option" title="Light">
      <input type="radio" name="theme" value="light">
      <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="4"/><path d="M12 2.5v2M12 19.5v2M4.6 4.6l1.4 1.4M18 18l1.4 1.4M2.5 12h2M19.5 12h2M4.6 19.4L6 18M18 6l1.4-1.4"/></svg>
      <span class="theme-toggle__label">Light</span>
    </label>

    <label class="theme-toggle__option" title="Dark">
      <input type="radio" name="theme" value="dark">
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5Z"/></svg>
      <span class="theme-toggle__label">Dark</span>
    </label>
  </fieldset>
  <noscript><button type="submit" class="theme-toggle__save">Save</button></noscript>
</form>
```

`theme-toggle.css`:

```css
/* Theme toggle (HTML + HTMX). Needs tokens.css loaded first. */

.theme-toggle { display: inline-flex; align-items: center; gap: var(--space-2); margin: 0; }

.theme-toggle fieldset {
  display: inline-flex;
  gap: 2px;
  margin: 0;
  padding: 3px;
  border: 1px solid var(--border);
  border-radius: var(--radius-full);
  background: var(--surface-sunken);
}

/* Visible to screen readers only */
.theme-toggle legend,
.theme-toggle.is-compact .theme-toggle__label {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip: rect(0 0 0 0);
  white-space: nowrap;
}

.theme-toggle__option {
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  min-height: 40px;
  padding: 0 14px;
  border-radius: var(--radius-full);
  font: 600 14px/20px var(--font-sans);
  color: var(--ink-muted);
  cursor: pointer;
  transition: background-color 150ms ease, color 150ms ease, box-shadow 150ms ease;
}
.theme-toggle.is-compact .theme-toggle__option { width: 40px; padding: 0; justify-content: center; }
.theme-toggle__option:hover { color: var(--ink); }

/* The real radio covers the whole option: a large tap target, native keyboard support */
.theme-toggle__option input { position: absolute; inset: 0; margin: 0; opacity: 0; cursor: pointer; }

.theme-toggle__option:has(input:checked) { background: var(--surface); color: var(--ink); box-shadow: var(--shadow-sm); }
.theme-toggle__option:has(input:checked) svg { color: var(--brand); }
.theme-toggle__option:has(input:focus-visible) { outline: 2px solid var(--focus); outline-offset: 2px; }

.theme-toggle__option svg {
  width: 18px;
  height: 18px;
  flex: none;
  fill: none;
  stroke: currentColor;
  stroke-width: 2;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.theme-toggle__save {
  min-height: 40px;
  padding: 0 14px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-md);
  background: var(--surface);
  color: var(--ink);
  font: 600 14px/20px var(--font-sans);
  cursor: pointer;
}

@media (prefers-reduced-motion: reduce) {
  .theme-toggle__option { transition: none; }
}
```

Notes on fitting it to this app: the options are 40 px tall, under the
44 px control minimum in `design-system/components/` (the padded fieldset
makes the whole group 48 px, so check which one the design system means);
the visually-hidden rule lacks the `clip-path: inset(50%)` the language
selector's rule has; and the `transition` should stay inside
`prefers-reduced-motion: no-preference`, as the panel's does.
