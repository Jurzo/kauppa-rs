# Consultation Request Pack — Theme switch: the cookie and `POST /theme`, repainting without script, its shape and place, the top bar breakpoint

```yaml
---
id: CRP-0035
mission: MB-0055
owner: "@juuso-kalliomaki"
target_role: security
risk_tier: medium
urgency: medium
state: answered
runbook_trigger: "MB-0055 autonomy.escalate (cookie and route → security; `POST /theme` → api-owner; repaint mechanism, phone placement and a breakpoint near 64rem → owner); feature.md escalation trigger `i18n/request.rs` (the request's root context gains the theme) and `.agentic/resolutions/**`"
---
```

## Decision statement

Decide how the theme choice is stored and posted (a new cookie and
`POST /theme`), how the page repaints at once without script now that the
baseline's `hx-on` can't run, what shape the switch takes in the top bar
and where it sits on phones, and the top bar's breakpoint that follows from
that shape.

## Terms

- **System, Light, Dark**: the three choices. System means "follow the OS
  setting", which is what the app does today for everyone.
- **`:has()`**: a CSS selector that matches an element by what it contains.
  `:root:has(input[value="dark"]:checked)` matches the page's root element
  while the Dark radio button is checked. Supported in Chromium since 2022,
  Safari since 15.4 (2022), Firefox since 121 (December 2023).
- **Boosted navigation**: `<body hx-boost="true">` makes htmx fetch a clicked
  link and swap `<body>`. The `<html>` element and `<head>` stay as they
  were.
- **Breakpoint**: the width below which the navigation becomes the menu
  panel (MB-0040). Today 40rem (640 px).

## Current state

- **Tokens** (`assets/app.css:1-106`, MB-0054): light on `:root`; dark under
  `@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) }`
  and again under `:root[data-theme="dark"]`. Each block sets
  `color-scheme`. Nothing sets `data-theme`.
- **The top bar** (`templates/layout.hbs`): brand, menu button, the `<nav>`
  (a popover below 40rem, inline above with `flex-wrap: wrap`,
  `app.css:186-243`), then the language selector (`.lang`, outside the
  `<nav>`, `app.css:284-353`).
- **The one per-request value the layout reads** besides the language is
  `request_path`, put into every page's root context by `Ui::render`
  (`crates/kauppa-web/src/i18n/request.rs`). This file is an escalation
  trigger path.
- **The language cookie** (`i18n/request.rs`, `language_cookie`):
  `kauppa_lang`, `Path=/`, `SameSite=Lax`, `HttpOnly`, `Secure` per
  `KAUPPA_COOKIE_SECURE`, `Max-Age` one year, unsigned. `security.md:29`
  says it is "the one other cookie" beside the session.
- **`POST /language`** (`routes/language.rs`): in `routes::protected`, so
  behind the cross-site check (RR-0010); `return` is cleaned by
  `same_site_path`; answers 303.
- **HTML responses** already carry `Vary: Cookie, Accept-Language` and
  `Cache-Control: no-store` (`csp.rs`), so a cookie-dependent page is never
  served from a cache.
- **`hx-on` can't run here.** htmx 2.0.11 wraps an `hx-on` handler in its
  `allowEval` check and compiles it with `new Function("event", …)`. The
  layout sets `allowEval: false`, so htmx refuses it with
  `htmx:evalDisallowedError`. Even with `allowEval` on, the policy's
  `script-src 'self'` (no `'unsafe-eval'`) would block `new Function`.

## Options and trade-offs

### Q1 — The cookie (security)

**Recommended: `kauppa_theme`, written only for Light or Dark.**

- Value `light` or `dark`. Choosing System **removes** the cookie
  (`Max-Age=0`), so "no cookie" and "System" are the same state and there
  is one less value to handle. A missing cookie or any other value reads as
  System.
- Attributes as `kauppa_lang`: `Path=/`, `SameSite=Lax`, `HttpOnly` (no
  script reads it; the server renders the checked radio), `Secure` per
  `KAUPPA_COOKIE_SECURE`, `Max-Age` one year.
- Unsigned. The value chooses between two stylesheets' worth of colours and
  is never echoed raw: the server maps it to an enum, and the template only
  ever sees `light`, `dark` or `system`. A forged value can only pick one of
  the three, which the user could pick anyway.
- Not kept on the account (MB-0055 non-goal; a schema change). A signed-in
  user's choice therefore stays with the browser, not the account. This is
  unlike the language, which RR-0023 also stores on the account.

Alternative: store `system` as a value too. One more state, no gain.

### Q2 — `POST /theme` (security, api-owner)

**Recommended:**

- Registered in `routes::protected`, so it sits behind the cross-site check
  as `/language` does. Form fields `theme` (`system` | `light` | `dark`)
  and `return`. Any other `theme` is a 400 that sets nothing.
- With `HX-Request: true`: `204 No Content`, no body, the cookie set. The
  form uses `hx-swap="none"`, so nothing on the page is replaced.
- Without htmx (the `<noscript>` Save button): `303` to
  `same_site_path(return)`, the cookie set, so the next page renders in the
  new theme.
- `same_site_path` moves from `routes/language.rs` to a shared place (or
  is imported from there), not copied.
- The theme reaches templates as one more root-context value beside
  `request_path`, set in `Ui::render` (`i18n/request.rs`, a trigger path,
  hence this consult). Proposed name `request_theme`. RR-0006's `page_`
  prefix is for values a view passes to the layout, and this one comes from
  the request, as `request_path` does.
- No GET changes the theme; no `?theme=`.
- No log line of its own. `POST /language` logs none either, and the request
  trace already records the call.

### Q3 — Repainting at once without script (owner)

The baseline sets `data-theme` on `<html>` from an `hx-on` handler, which
can't run here (see Current state).

**A (recommended): CSS keyed on the checked radio, replacing the
`data-theme` selectors.** The token blocks' selectors change and their
values don't:

    :root { /* light tokens */ }
    @media (prefers-color-scheme: dark) {
      :root:not(:has(#theme-light:checked)) { /* dark tokens */ }
    }
    :root:has(#theme-dark:checked) { /* dark tokens */ }

- The server renders the saved choice's radio `checked`, so **first paint
  is already right**: no flash, no attribute on `<html>`.
- Choosing a radio repaints the page at the moment of the click. The htmx
  post only saves it. Choosing System falls back to the OS on the same
  page, since neither dark rule matches unless the OS is dark.
- Boosted navigation is safe: the new `<body>` brings a switch rendered from
  the cookie, which htmx has just set.
- No duplication: the same two dark blocks as today, with different
  selectors. Light needs no rule of its own, because `:root` is light.
- `data-theme` is dropped from `app.css`. It can't be mixed with `:has()`:
  a `data-theme="dark"` rendered at load would stay on `<html>` after the
  user picked Light (boost doesn't swap `<html>`), and keep the page dark.
- Cost: browsers without `:has()` (Firefox before 121, Safari before 15.4)
  follow the OS whatever the user picks. The switch still saves the choice,
  and the colours apply once the browser updates.
- Amends MB-0055 P3 ("the server renders `data-theme`" becomes "the server
  renders the saved radio checked") and changes `design-system/README.md`'s
  theming paragraph. `design-system/tokens.css` keeps its `data-theme`
  selectors as the design system's portable form. The README says how the
  app maps them.
- Risk: low. Cost: small.

**B: a full page load.** htmx posts and the server answers with
`HX-Refresh: true`, or the form is a plain POST as the language form is.
The server renders `data-theme` from the cookie.

- Pros: no selector change; `data-theme` used as the design system intended.
- Cons: every click reloads the page, a visible wait on a slow phone (goal
  2: "works on a slow phone over 3G"), and a form being filled in is lost.
- Risk: low. Cost: small.

**C: a same-origin script** (`assets/theme.js`) that sets `data-theme`.

- Pros: the baseline's behaviour exactly.
- Cons: the first JavaScript the app writes itself. AGENTS.md allows
  nothing executable under `assets/` outside `vendor/`, so it needs an
  AGENTS.md change and a platform decision, against templating.md's "htmx
  first; JS only when htmx can't". Here CSS can.
- Risk: medium (policy precedent). Cost: small code, large governance.

### Q4 — The switch's shape on wide screens (owner)

Measured in headless Edge 157 (see Evidence bundle): the width the top bar
needs to fit on one line, with the app's real stylesheet and fonts. The
widest case is Finnish signed out ("Kirjaudu sisään"):

| Switch in the bar | Widest bar | Breakpoint (widest + about 5%) |
| --- | --- | --- |
| None (today) | 765 px | (today 640 px: it wraps between 640 and 765 px) |
| **A: three icon-only segments (baseline `is-compact`)** | **913 px** | **60rem (960 px)** |
| B: one icon button opening a popover with the three choices | 825 px | 54rem (864 px) |
| C: three segments with labels (baseline default) | 1071 px | 70rem (1120 px) |

**A (recommended): icon-only segments**, the baseline's compact variant.
Labels are visually hidden, and each option keeps a `title` and its label
for assistive technology.

- Pros: the owner's design; one tap; the current choice is always visible.
- Cons: tablets in portrait (768–834 px) get the menu panel instead of the
  inline nav. Landscape tablets and laptops (1024 px and up) keep it inline.
- Risk: low.

**B: one button and a popover**, like the language selector.

- Pros: the narrowest; same pattern as the language selector.
- Cons: two taps, and the current choice shows only as the button's icon.
  It departs from the baseline.

**C: labelled segments.** 70rem is above the brief's 64rem escalation
line: laptops at 1024 px would get the menu panel. Not recommended.

### Q5 — Where it sits on a phone (owner)

CSS can't move an element into or out of the `<nav>` popover (as found for
CRP-0031 Q3), and the page must hold **one** switch, since two copies would
mean two checked radios for `:has()` to disagree on. So the HTML position
decides both widths.

**A (recommended): inside the `<nav>`, as its last group.** On wide screens
the `<nav>` is inline, so the switch shows in the top bar after the links.
The order is brand · links · sign in/out · theme · language. Inside the
`<nav>`, CSS `order` may put the theme before sign in/out. On phones it is
in the menu panel.

- In the panel (256 px of content width), the labelled segments don't fit
  (291 px measured). Recommended: there the legend shows as a visible small
  heading ("Theme" / "Teema") and the options keep their labels as three
  stacked rows, each like a nav entry (at least 48 px tall). On wide
  screens, the icon-only segments of Q4 A. Same HTML, two layouts.
- Pros: the 320 px bar is untouched. It holds brand, language selector and
  menu button with nothing to spare (MB-0054 had to shrink the brand, drop
  the chevron and make the menu button icon-only at 340 px and below).
- Cons: two taps on a phone (menu, then theme). That's acceptable for a
  setting that is rarely changed.
- Browsers without popover show the `<nav>` inline everywhere (MB-0040's
  fallback), switch included.

**B: in the top bar on phones.** Even the narrowest form (Q4 B, a 44 px
button and a gap) adds about 52 px to a bar that already has no room, so
one of the three existing items would have to go: RR-0031 sub-answer 4
forbids that. Not recommended.

### Q6 — The breakpoint and wrapping (owner, for information)

Following Q4 A and Q5 A:

- One breakpoint, **60rem**, shared by the navigation panel's rules
  (`app.css`'s `max-width: 39.9375rem` blocks become
  `max-width: 59.9375rem`).
- Above it, the inline `<nav>` gets `flex-wrap: nowrap`, but only inside
  the `@supports selector(:popover-open)` block. Browsers without popover
  have no panel to fall back to, so there the nav keeps `wrap`, which is
  better than running off the side of a phone.
- The language selector's short name (`EN`/`FI`) stays tied to 40rem: it
  is about the phone bar, not the navigation. Between 40rem and 60rem the
  bar shows brand, full language name and menu button, which fit easily.
- If the implementation measures more than 60rem with the real switch
  markup, the brief's rule applies: take the widest plus a margin and record
  it. Above 64rem, stop and ask.

### Q7 — Tests and amendments (owner, for information)

- New: `routes::theme::tests` (P2: htmx and plain posts, a bad value, a
  cross-site post, a hostile `return`, System removing the cookie);
  `templates::tests::theme_*` and `top_bar_breakpoint` (P1, P3–P7) over the
  rendered layout with each cookie state and over the served `app.css`.
- Changed: MB-0054's `design_tokens` test finds the dark blocks by their
  exact selectors (`DARK_OS`, `DARK_CHOSEN` in
  `templates/tests/design.rs:27-28`). They become the `:has()` selectors,
  with the value checks unchanged. MB-0051's `templates/tests/language.rs:21`
  reads the `39.9375rem` block for the short names. That stays at 40rem
  (Q6), so the test is unchanged. No other test reads a media query.
- Brief amendments for the RR's roll-back: P3's wording (Q3 A), P7 and the
  `escalate` entries Q4–Q6 settle, `resolutions`.
- Other roll-back targets: `security.md:29` ("the one other cookie" becomes
  two), `design-system/README.md`'s theming paragraph.

## Recommendation and question

Recommended: Q1 `kauppa_theme`, Light or Dark only, System removes it,
attributes as `kauppa_lang`; Q2 `POST /theme` behind the cross-site check,
204 under htmx, 303 otherwise, `request_theme` from `Ui::render`; Q3 A
(`:has()` on the checked radio, `data-theme` dropped from `app.css`); Q4 A
(icon-only segments); Q5 A (inside the `<nav>`, labelled rows in the
panel); Q6 one 60rem breakpoint, `nowrap` only where popover is supported.

Crisp questions:

1. Security: a `kauppa_theme` cookie holding `light` or `dark`, removed for
   System, unsigned, with `kauppa_lang`'s attributes (`Path=/`,
   `SameSite=Lax`, `HttpOnly`, `Secure` per setting, one year)?
2. Security and API: `POST /theme` in `routes::protected`, fields `theme`
   and `return`, 400 on any other value, 204 under htmx and 303 to
   `same_site_path(return)` otherwise; the theme passed to templates as
   `request_theme` from `Ui::render`?
3. Repaint: CSS keyed on the checked radio (A), dropping `data-theme` from
   `app.css` and amending P3, accepting that browsers without `:has()`
   follow the OS?
4. Shape on wide screens: icon-only segments (A), which puts the breakpoint
   near 60rem?
5. Phone placement: inside the `<nav>` (A), shown in the menu panel as a
   headed group of three labelled rows?
6. Breakpoint: one shared 60rem breakpoint, with `nowrap` only where
   popover is supported, and the language short name left at 40rem?

## Blast radius and rollback

What can break:
- Colours on every page if the token selectors are wrong. The contrast and
  token tests (MB-0054 P1, P2) keep the values; P10 checks the switching.
- The top bar at widths between 640 and 960 px changes from inline links to
  the menu button.
- A stuck cookie can't do worse than force one of the three themes, which
  the switch changes back.

Who is affected: every user, signed in or out, on every page.

Rollback plan: revert the PR. No schema, no stored data. A leftover
`kauppa_theme` cookie is ignored by the reverted app and expires within a
year.

## Minimal context

- MB-0055 P1–P10, `autonomy.escalate` and `forbidden`.
- MB-0040 (navigation panel), MB-0051 and RR-0031 (language selector and
  sub-answer 4: never hide one of the three phone-bar items), MB-0054 and
  RR-0034 (tokens, brand icon).
- RR-0010 (cross-site check), RR-0019 (CSP, unchanged), RR-0023 (language
  cookie, the model), RR-0006 (`page_` prefix).
- Files the implementation touches: `templates/layout.hbs`,
  `assets/app.css`, `locales/en.json`, `locales/fi.json`,
  `crates/kauppa-web/src/routes/theme.rs` (new), `routes/mod.rs`,
  `routes/language.rs` (only if `same_site_path` moves),
  `i18n/request.rs`, the test modules, `design-system/README.md`.

## Evidence bundle (Hardened)

**Top bar widths.** Measured 2026-11-13 in headless Edge 157 (Windows, from
WSL), 1600 px window, from a scratch page that links the repository's
`assets/app.css` and fonts (`document.fonts` reported Inter and Space
Grotesk loaded) and copies the top bar markup from `layout.hbs`: the brand
with its icon, the four links, sign in or out, the language button with
flag and full name. The bar was given `width: max-content` and the nav
`nowrap`, and its width read with `getBoundingClientRect()`. The switch was
the baseline's markup with `theme-toggle.css`. "One button" was a
`.nav-open`-styled button holding one icon.

| | none | one button | icon-only | labelled |
| --- | --- | --- | --- | --- |
| Finnish, signed in | 748 | 808 | 896 | 1055 |
| Finnish, signed out | 765 | 825 | 913 | 1071 |
| English, signed in | 726 | 786 | 874 | 1032 |
| English, signed out | 715 | 775 | 863 | 1021 |

- "none" matches the owner's report that the nav wraps below about 750 px.
- The panel's content width: `min(18rem, 85vw)` minus 2rem of padding,
  256 px at most. The labelled segments alone measure 291 px.
- Not measured: phone widths. Headless Edge gave identical results for 320
  and 360 px windows, so it wasn't honouring the narrow viewport and those
  numbers were discarded. The phone bar's tightness is MB-0054's recorded
  finding (its pack: brand shrunk, chevron hidden, icon-only menu button at
  340 px and below).
- The scratch page is in the session scratchpad, not in the repository.

**`hx-on`.** `assets/vendor/htmx-2.0.11.min.js`: the `hx-on` handler is
built as `Tn(t, function(){ … o = new Function("event", n) … })`, and
`Tn` runs its callback only when `htmx.config.allowEval` is true, else
fires `htmx:evalDisallowedError`. `layout.hbs` sets `"allowEval":false`.
`csp.rs` `POLICY`: `script-src 'self'`, no `'unsafe-eval'`.

**Triggers.** `scripts/agentic/check_triggers.py` (feature.md's table):
`i18n/request.rs` and `routes/language.rs` are triggers (both
`security:language-cookie`). The first is touched either way. The second
is touched only if `same_site_path` moves out of it; importing it from
there leaves the file alone. `templates/`, `assets/app.css`, `locales/`
and a new `routes/theme.rs` are not triggers.
