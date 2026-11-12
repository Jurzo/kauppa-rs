# Consultation Request Pack — Language dropdown: amending RR-0023 sub-answer 7, the mechanism, phone placement, the flags and their files

```yaml
---
id: CRP-0031
mission: MB-0051
owner: "@juuso-kalliomaki"
target_role: product
risk_tier: low
urgency: medium
state: answered
runbook_trigger: "MB-0051 autonomy.escalate (RR-0023 sub-answer 7 amendment, mechanism, phone placement, flags → owner; flag files → platform-owner; note to security); AGENTS.md do-not-touch: third-party files under `assets/`; feature.md escalation trigger `.agentic/resolutions/**`"
---
```

## Decision statement

Decide how the language switcher moves from the footer to a dropdown in the
top bar: the amendment to RR-0023 sub-answer 7, the no-JavaScript mechanism,
where it sits on a phone, which flags stand for English and Finnish, and
how the two SVG files are sourced, licensed, stored and served.

## Terms

- **Popover**: an element with the HTML `popover` attribute. The browser
  hides it until a button with `popovertarget="<its id>"` opens it. The
  browser closes it on Escape and on a click outside ("light dismiss"). No
  script is involved.
- **Top layer**: where the browser draws an open popover, above everything
  else on the page, whatever the page's `z-index` values.
- **Boosted navigation**: `<body hx-boost="true">` makes htmx fetch a
  clicked link and swap the `<body>`, instead of a full page load.

## Current state

- The switcher is a `<footer>` form with two submit buttons marked
  `aria-pressed` (`templates/layout.hbs:45-54`), as RR-0023 sub-answer 7
  decided (`.agentic/resolutions/RR-0023-translations.md:124-133`). Its
  styles are `assets/app.css:103-110`.
- The form posts to `/language` with `hx-boost="false"`, a hidden `return`
  path and `lang` from the pressed button. The handler answers 303 and sets
  the cookie (`crates/kauppa-web/src/routes/language.rs:40-58`).
- The top bar holds the brand, the menu button and the `<nav>`
  (`templates/layout.hbs:20-41`). The `<nav>` is a popover on phones, below
  40rem, and inline on wide screens (`assets/app.css:49-97`, MB-0040).
- `assets/` holds `app.css`, `manifest.webmanifest` and two vendored
  scripts. There is no image yet, and no template has an `<img>`.
- `build.rs` knows three extensions, `css`, `js` and `webmanifest`, and
  fails the build on any other (`crates/kauppa-web/build.rs:15-19,79-84`).
- The policy is `img-src 'self'` with no `data:`
  (`crates/kauppa-web/src/csp.rs:22-24`).
- The owner's two flag files are parked outside the repo, at
  `/home/juuso/kauppa-rs/.claude/mb-0051-flags/en.svg` and `fi.svg`. See
  the Evidence bundle for what they contain.

## Options and trade-offs

### Q1 — Amending RR-0023 sub-answer 7 (owner)

Sub-answer 7 says: a `<footer>`, two submit buttons, the current one marked
`aria-pressed="true"`. The brief's goal contradicts the first and third
part, so an RR must amend it.

- **Recommended:** the new RR amends sub-answer 7 in three places and keeps
  the rest.
  - Changes: the switcher sits in the top bar, not a `<footer>`; it is a
    closed control that opens a list, not two always-visible buttons; the
    current language is marked as Q2 decides, not with `aria-pressed`.
  - Stays: one POST form to `/language`; one submit button per language,
    named in its own language, with its own `lang`; `hx-boost="false"` so
    the answer is a full page load and `<html lang>` updates; no JavaScript;
    works without htmx; usable at 320 px; on every page, signed in or out.
  - The `<footer>` element and the `.site-footer` and `.lang-switch` rules
    are removed. Nothing else uses them (`grep -rn "site-footer\|lang-switch"`
    finds only `layout.hbs`, `app.css` and `i18n/tests.rs`).
- Alternative: keep the footer buttons as well as the dropdown. Two
  switchers on every page; the brief's P1 says the footer one is gone.

### Q2 — The mechanism, without JavaScript (owner)

All three options keep the one POST form. A and B keep one submit button
per language, each with `lang` and the language's own name.

**A: `<details>`/`<summary>`.** The `<summary>` is the closed control; the
form with the two buttons is the content.

- Pros:
  - Works in every browser, including ones without popover.
  - The list is easy to place: `position: absolute` under the `<summary>`.
  - Keyboard: Tab reaches the summary; Enter or Space opens it. Screen
    readers announce "collapsed" and "expanded".
- Cons:
  - **Escape does not close it, and neither does a click outside.** The
    brief's P4 requires Escape, so P4 would have to be weakened.
  - It is drawn inside the sticky top bar's stacking order, not the top
    layer, so it needs its own `z-index` care.
  - The open state is an `open` attribute in the page's HTML. htmx saves
    the HTML for the Back button, so going Back can bring a page back with
    the list open.
- Risk: low. Cost: small.

**B (recommended): a popover opened by a button**, the mechanism the
navigation panel already uses (`templates/layout.hbs:26-28`).

    <button type="button" popovertarget="lang-menu"> flag, name, chevron </button>
    <form id="lang-menu" popover method="post" action="/language" hx-boost="false">
      hidden "return" field; one submit button per language
    </form>

- Pros:
  - **Escape closes it and returns focus to the control; a click outside
    closes it.** Both are browser behaviour. P4 holds as written.
  - The browser tells assistive technology whether the control is expanded.
    No ARIA attribute is hand-written for that.
  - Top layer: nothing in the page can cover or clip the list.
  - One mechanism in the app, already tested and browser-checked in
    MB-0040.
  - Popovers of this kind close each other. On a phone, opening the menu
    closes the language list, and the reverse. They can't overlap or trap
    each other (P6).
  - The open state is not in the HTML. A boosted navigation swaps `<body>`
    and the list is closed in the new page; so is a page restored by Back.
- Cons:
  - The browser draws a popover centred in the window by default. CSS must
    place it under the control. CSS anchor positioning does this exactly
    where the browser has it; elsewhere a fixed position under the top bar
    at the right edge is the fallback. This is the main implementation
    risk, and P6's browser check covers it.
  - Browsers from before 2024 don't know `popover`. There the list would be
    always visible. The same `@supports selector(:popover-open)` test that
    MB-0040 uses (`assets/app.css:66`) handles it: without support, the
    opener is hidden and the two flag buttons show inline in the top bar,
    which is today's switcher with flags.
  - MB-0040's test helper expects exactly one popover opener in the page
    (`crates/kauppa-web/src/templates/tests.rs:439-446`). It must be
    narrowed to "the opener whose target is the `<nav>`". A test-only
    change.
- Risk: low. Cost: small to medium (the placement CSS).

**C: a native `<select>` and a submit button.**

- Pros: the best-known control, with the best keyboard and screen-reader
  support and the phone's own picker.
- Cons:
  - **`<option>` can't show an image**, so no flags in the list. That fails
    P1 and the owner's reference image.
  - Without script a `<select>` doesn't submit on change. It needs a
    visible submit button beside it: two controls in a top bar that is
    already tight at 320 px.
- Risk: low. Cost: small.

**Details that apply to A and B (recommended as listed):**

- **The closed control** shows the current language's flag, its name and a
  chevron. Its accessible name says what it is for, for example "Language:
  English": the existing `language.label` text, visually hidden, before the
  visible name.
- **No ARIA menu roles** (`role="menu"`, `menuitemradio`, `listbox`). Those
  roles promise arrow-key navigation, which needs script. The options are
  plain submit buttons: Tab moves between them, Enter or Space chooses.
- **The current language** is marked `aria-current="true"` on its button,
  with a visible check mark that is hidden from assistive technology.
  `aria-pressed` describes a toggle button, which fitted two side-by-side
  buttons but not an item in a list.
- **Flags are decoration**: `<img alt="">` with `width` and `height`. The
  name is the text.
- **Names**: each option has the full name (`English`, `Suomi`) and the
  short name (`EN`, `FI`). On narrow screens the full name is visually
  hidden, not `display: none`, and the short name is `aria-hidden`. So
  assistive technology reads the full name at every width (P3).
- **After a choice** the answer is a full page load, so the list is closed.

### Q3 — Placement on a phone (owner)

The selector's place in the HTML decides this. CSS can't move an element
into or out of the `<nav>` popover.

**A (recommended): in the top bar, between the brand and the menu button.**
On wide screens it sits at the right end, after the inline navigation.

- Pros:
  - One tap to open, on every page, signed in or out.
  - It is what the reference image's compact variant is for; the short
    names have a purpose.
  - It stays outside the navigation popover, so Q2's "they close each
    other" holds.
- Cons: the top bar gets tight at 320 px. Estimate, at a 16 px root font
  with the system font, inside the 288 px the bar's padding leaves:

  | Part | English | Finnish |
  | --- | --- | --- |
  | Brand "kauppa" (bold, 1.1rem) | 62 px | 62 px |
  | Selector: flag 20 px, `EN`/`FI`, chevron, padding, border | 78 px | 78 px |
  | Menu button: "☰ Menu" / "☰ Valikko" (`app.css:56-61`) | 93 px | 105 px |
  | Two 1rem gaps (`app.css:39`) | 32 px | 32 px |
  | **Total of 288 px** | **265 px** | **277 px** |

  It fits, with 23 px to spare in English and 11 px in Finnish. That is
  an estimate from font metrics, not a measurement. If the browser check
  shows it doesn't fit, the levers are a 0.5rem gap on phones (16 px) and
  dropping the chevron on phones (14 px). Both are within the brief's
  `may_decide`. Hiding the selector or the menu button is the brief's
  slow-mode trigger, and I would stop and ask.
- Risk: low.

**B: inside the navigation panel**, as its last entry.

- Pros: no width problem; the panel has room for full names.
- Cons:
  - Two taps on a phone (menu, then language).
  - The short names would have no use, against P3.
  - With Q2 B it would be a popover inside a popover. Browsers support
    that, but it is one more thing to check.
- Risk: low.

### Q4 — Which flags (owner)

- **Recommended:** the United Kingdom's flag for English and Finland's flag
  for Finnish, as the owner's two files show. A later language's flag is
  chosen in the brief that adds the language.
- Alternative for English: the United States' flag. English has no flag of
  its own; either is a convention. The flag is decoration, and the name is
  always shown.

### Q5 — The flag files: source and licence (platform)

The files carry no licence text or source comment. One marker points at
their origin: `<g class="nc-icon-wrapper">`, the wrapper the Nucleo icon
app writes on export. Their geometry (a 32 px grid, a 30×24 flag with 1 px
of padding) matches Nucleo's free flag set as its page describes it
(https://nucleoapp.com/svg-flag-icons).

What that implies, if the files are Nucleo's:

- They would be under the **Nucleo Icons License**, a proprietary licence,
  not an open-source one. This repository is public and MIT-licensed
  (`LICENSE`; `Cargo.toml:9`).
- Nucleo's licence page (https://nucleoapp.com/license), as I could read
  it through a summarising fetch, allows up to 100 Nucleo icons in
  publicly accessible source files and forbids redistributing them
  otherwise. Two flags would be inside that allowance. **I could not read
  the licence text directly, and I can't confirm the files' origin. The
  owner must confirm both.**
- The repository's MIT licence would not cover the two files. A short note
  should say so.

Options:

- **(a) (recommended, if the owner confirms the licence allows it): the
  owner's two files, byte for byte.** The RR records their source and
  licence. A note in `README.md` says the flag files are not under the
  repository's MIT licence. Free in cost; they match the reference image.
- (b) Two flags from an MIT-licensed set, for example `lipis/flag-icons`,
  at a pinned version. A clear open-source licence; a different look
  (square corners, 4:3). Use this if (a) can't be confirmed.
- (c) Flags drawn for this project. The designs of both national flags are
  public. Finland's is two rectangles; the United Kingdom's takes more
  care. No licence question; the result looks plainer.

### Q6 — Where the files live and how they are served (platform)

- **Location (recommended): `assets/flags/en.svg` and
  `assets/flags/fi.svg`**, named by language tag, as `Language::tag`
  gives it. A flag stands for a language here (the brief's non-goal), and
  adding a language is then "a catalog file, a flag and a `Language`
  variant".
  - Alternative: `assets/images/flags/`. One more folder level with nothing
    else in `images/`.
  - Not recommended: `assets/vendor/`. Files there are cached for a year
    as immutable because their names carry a version
    (`crates/kauppa-web/src/routes/assets.rs:84-88`, RR-0029 sub-answer 3).
    `en.svg` carries none, so a replaced flag would be stuck in browsers.
- **Content type:** add `("svg", "image/svg+xml")` to `CONTENT_TYPES`
  (`crates/kauppa-web/build.rs:15-19`), and two rows to `SERVED_AS`
  (`crates/kauppa-web/src/routes/assets/tests.rs:17-25`). No `charset`
  parameter: `actix-files` 0.7.4, which the contract is pinned to, adds one
  only to text types, and the files are plain ASCII.
- **Why the type matters:** every response carries
  `X-Content-Type-Options: nosniff` (`csp.rs:40`), and a browser draws an
  SVG in `<img>` only when it is served as `image/svg+xml`. A wrong type
  shows a broken image. `serves_every_asset` pins the type (P5).
- **Cache:** nothing to decide. Outside `vendor/` a file gets
  `Cache-Control: no-cache` and an `ETag` (`assets.rs:84-99`). A full page
  load revalidates each flag and gets a small 304. `cache_headers` loops
  over every asset, so it covers the new files unchanged.
- **CSP:** unchanged. `img-src 'self'` covers `/assets/flags/*.svg`,
  because the files are same-origin. No `data:` URL is used.
- **"Nothing executable" (AGENTS.md, do-not-touch):** a browser runs no
  script and loads no outside resource from an SVG shown through `<img>`.
  The two files hold only `<svg>`, `<g>`, `<rect>` and `<path>` (Evidence
  bundle). If someone opens a flag's URL directly, the SVG is a document;
  it gets the same CSP header as every response, which blocks inline
  script.
  - **Recommended:** treat SVG images as non-executable under that rule,
    and keep them so with a test: every `.svg` under `assets/` has no
    `<script`, no `on…=` attribute, no `href`, no `<foreignObject`, no
    `<style` and no `url(`. A content rule rather than a checksum, so the
    owner can replace a flag without updating a hash.
  - Stricter alternative: pin each file's SHA-256 in the test suite, as
    for the vendored scripts.
  - AGENTS.md's rule names only JavaScript today. The RR could ask for a
    sentence on image files there. That edit is the owner's (roll-back).

### Q7 — Tests (owner, for information)

- **P1, P3, P4** (`templates::tests::language_dropdown`, `…_names`,
  `…_accessible`): new tests over the rendered layout, in both languages,
  using the helpers MB-0040 added (`templates/tests.rs:375-430`).
- **P2** (`i18n::tests::language_selection`): the route behaviour is
  untouched. Three markup assertions change:
  `crates/kauppa-web/src/i18n/tests.rs:273-278` (the form's
  `class="lang-switch"` opening tag), `283-284` (the two buttons with
  `aria-pressed`) and `292-295` (`aria-pressed="true"` on `fi`). They
  become the new form tag and `aria-current`. The `return` field assertion
  (`279-282`) and everything after line 296 stay as they are.
- **P5** (`routes::assets::tests::serves_every_asset`): passes once the
  two `SERVED_AS` rows exist; it already fails on a file without a row
  (`assets/tests.rs:75-78`). Plus the SVG content test from Q6.
- **MB-0040's tests**: with Q2 B, the `panel()` helper is narrowed as
  described there. `navigation_needs_no_script` and
  `csp_compatible_templates` already scan every template for script,
  `style=` and handlers, and apply to the new markup unchanged.
- **Catalogs**: new keys for the short names (and the label, if its
  wording changes), in both `locales/*.json`. The existing parity and
  no-literal-text tests cover them. `alt=""` passes the literal-text scan.
- **P6**: the owner's browser check at 320 px and desktop, recorded in the
  Continuity Pack.

### Note to security

Nothing in security's area changes: `POST /language` and its handler
(`routes/language.rs`), the cross-site check in front of it, the
`kauppa_lang` cookie (`i18n/request.rs`) and the Content-Security-Policy
(`csp.rs`) stay byte for byte. The form still posts the same two fields.
The only new request type is a same-origin `GET` for an image. Security
only needs to confirm this reading.

## Recommendation and question

Recommended: Q1 amend as listed; Q2 B (popover) with the listed details;
Q3 A (top bar); Q4 the United Kingdom's and Finland's flags; Q5 (a) the
owner's files once the owner confirms their licence; Q6 `assets/flags/`,
`image/svg+xml`, the default cache rule and a content test.

Crisp questions:

1. Amend RR-0023 sub-answer 7: the switcher moves from the footer to the
   top bar as a dropdown, the footer is removed, and the rest of the
   sub-answer (one POST form, a button per language with its own `lang`
   and name, `hx-boost="false"`, no JavaScript, 320 px) stays?
2. Mechanism: a popover opened by a button (B), with inline flag buttons
   as the fallback in browsers without popover?
3. Semantics: the current language marked `aria-current="true"` with a
   decorative check mark, replacing `aria-pressed`; no ARIA menu roles;
   the control named "Language: <current name>"?
4. Phone placement: in the top bar between the brand and the menu button
   (A), with the short name?
5. Flags: the United Kingdom's flag for English and Finland's for Finnish;
   a later language's flag is decided in the brief that adds it?
6. Flag files: where do the two supplied files come from, and under what
   licence? If the licence allows them in this public repository, use them
   byte for byte and record source and licence in the RR and a `README.md`
   note (a)? If not, which fallback: an MIT-licensed set (b) or drawn for
   the project (c)?
7. Location and serving: `assets/flags/<language tag>.svg`,
   `("svg", "image/svg+xml")` in `build.rs`, two `SERVED_AS` rows, and the
   existing `no-cache` plus `ETag` rule?
8. AGENTS.md's assets rule: SVG images loaded through `<img>` are not
   "executable", kept so by a content test (no script, handlers, links,
   `<foreignObject>`, styles) rather than a checksum? And should the RR
   add a sentence on image files to AGENTS.md?
9. Security: confirmed that `POST /language`, the `kauppa_lang` cookie and
   the Content-Security-Policy don't change, so nothing further is needed
   from security?

## Blast radius and rollback

What can break:
- The switcher is on every page. A broken selector leaves a user unable to
  change language from the page; the stored language keeps working.
- The top bar at 320 px: the selector could crowd the menu button (P6).
- The popover list could be misplaced in a browser where the placement CSS
  falls back.
- A wrong content type shows a broken flag image; the names still show.
- MB-0040's navigation tests share a helper that changes.

Who is affected: every user, signed in or out, on every page.

Rollback plan: revert the PR. No schema, cookie, endpoint or policy
changes, and no stored data is involved. Browsers hold the flag files only
under `no-cache`, so nothing stale remains.

## Minimal context

- MB-0051 P1–P6; `autonomy.escalate` and `forbidden`.
- RR-0023 sub-answer 7 (amended) and constraint 7 (unchanged: only
  `POST /language` switches). RR-0019 (CSP, unchanged). RR-0029 sub-answer
  3 (asset caching, unchanged).
- Files the implementation touches: `templates/layout.hbs`,
  `assets/app.css`, `assets/flags/*.svg` (new), `locales/en.json`,
  `locales/fi.json`, `crates/kauppa-web/build.rs`,
  `crates/kauppa-web/src/routes/assets/tests.rs`,
  `crates/kauppa-web/src/templates/tests.rs`,
  `crates/kauppa-web/src/i18n/tests.rs`, and `README.md` if Q6 (a).
- Roll-back targets for the RR: `templating.md` (the switcher's place, if
  the owner wants it recorded), AGENTS.md's assets rule (Q8), RR-0023 (an
  amendment note), the brief's `resolutions`.

## Evidence bundle (Hardened)

The two files, read on 2026-11-12 where they are parked (not moved):

| | `en.svg` | `fi.svg` |
| --- | --- | --- |
| Size | 1635 bytes, 17 lines | 700 bytes, 8 lines |
| SHA-256 | `c23bc9017ee858827c4777561a756d7173f4e2a8bc6f1b8a31b877dee1d6cab6` | `e10582601989d478d1a0cdded748d1fee78d361ed1ae4b4fa38e788b297fc1de` |
| Shows | United Kingdom's flag, rounded corners | Finland's flag, rounded corners |
| Elements | `svg`, `g`, 4 `rect`, 9 `path` | `svg`, `g`, 1 `rect`, 3 `path` |

- Both: `width="32px" height="32px" viewBox="0 0 32 32"`; plain ASCII; no
  XML declaration, DOCTYPE or entity.
- Neither has a `<script>`, an `on…=` handler, an `href` or `xlink:href`,
  a `url(…)`, `<use>`, `<image>`, `<a>`, `<foreignObject>`, `<style>` or
  `style=` (a case-insensitive `grep -ciE` for these finds 0 matches in
  each file).
- The only `class` is `nc-icon-wrapper` on the `<g>`. No stylesheet
  reaches inside an `<img>`, so it does nothing. It is the provenance
  marker Q5 discusses. Colours are `fill` attributes; two paths use
  `opacity`.
- No comment, `<title>`, `<desc>` or `<metadata>`: no licence or author
  text in either file.

Other evidence:

- `gh repo view --json visibility,licenseInfo` → `PUBLIC`, MIT.
- `scripts/agentic/check_triggers.py` lists no trigger for `assets/`,
  `build.rs`, `templates/` or the test files; `routes/language.rs`,
  `csp.rs` and `i18n/request.rs` are triggers and are not touched.
- `actix-files-0.7.4/src/encoding.rs:12-30`: a charset is added only to
  JavaScript, HTML, CSS, plain text, CSV and TSV.
- Not verified here, for the implementation and P6 to check: which browser
  versions support CSS anchor positioning; the width estimate in Q3; the
  Nucleo licence text.
