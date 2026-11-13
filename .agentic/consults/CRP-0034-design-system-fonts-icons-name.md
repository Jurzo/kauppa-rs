# Consultation Request Pack — Design system adoption: serving the fonts and icons, the adaptive favicon, where the tokens live, and the name's form

```yaml
---
id: CRP-0034
mission: MB-0054
owner: "@juuso-kalliomaki"
target_role: platform
risk_tier: low
urgency: medium
state: answered
runbook_trigger: "MB-0054 autonomy.escalate (fonts, new asset kinds, tokens at run time → platform-owner; adaptive favicon → platform-owner and security; root route → api-owner; name → owner); AGENTS.md do-not-touch: files under `assets/`; feature.md escalation trigger `.agentic/resolutions/**`"
---
```

## Decision statement

Decide how MB-0054 brings the design system's fonts and icons into what the
app serves (format, location, content types, caching and the rule in
AGENTS.md), whether the colour-switching favicon is used as it is, whether
the tokens are written into `app.css` or served as a second stylesheet, and
the exact form of the name `kauppalista.app` in the page and the manifest.

## Terms

- **Variable font**: one font file holding a range of weights (an "axis"),
  instead of one file per weight. Inter's file also has an optical-size
  axis (`opsz`), which adjusts letter shapes to the text size.
- **WOFF2**: the compressed font format browsers load. Converting a TTF to
  WOFF2 changes no glyph; it only compresses the file.
- **Subset**: a copy of a font with only some characters kept. "Latin"
  below means the character set Google Fonts calls `latin` (Basic Latin,
  Latin-1, dashes, curly quotes, the euro sign, a few arrows), plus Š š Ž ž
  for Finnish loan words.
- **Maskable icon**: an icon with padding around the mark, so Android can
  crop it to any shape.
- **Content test**: the test from RR-0031 that keeps the flag SVGs inert
  (no script, handlers, links, `<style>` and so on).

## Current state

- `design-system/fonts/` holds one upright variable TTF per family, from
  the owner's archive: Inter 874,708 B, Space Grotesk 134,112 B, JetBrains
  Mono 189,028 B (1,197,848 B together), each with its `OFL.txt`. None
  declares a Reserved Font Name.
- `design-system/assets/` holds the icons. `favicon.svg` (1,022 B) has an
  inline `<style>` with a `prefers-color-scheme` media query.
- `assets/` holds `app.css`, `manifest.webmanifest`, two vendored scripts
  and two flags. `build.rs` knows `css`, `js`, `svg` and `webmanifest` and
  fails the build on any other extension
  (`crates/kauppa-web/build.rs:16-21`).
- `/assets/` caching: files under `vendor/` are `immutable` for a year;
  every other file is `no-cache` with an `ETag`, so a browser keeps it and
  asks "changed?" on a full page load
  (`crates/kauppa-web/src/routes/assets.rs:43-47,84`; RR-0029 sub-answer 3
  and constraint 4).
- The policy is `default-src 'self'; script-src 'self'; style-src 'self';
  img-src 'self'; …` (`crates/kauppa-web/src/csp.rs:22-24`). It has no
  `font-src` or `manifest-src`, so both fall back to `default-src 'self'`:
  same-origin fonts and the manifest load, nothing else does.
- AGENTS.md's assets rule names two kinds of file under `assets/`:
  vendored JS under `assets/vendor/`, and flag SVGs under `assets/flags/`.
- The layout has no favicon link and one `theme-color` meta (`#1f6feb`).
  The manifest's `icons` is empty.
- Already decided by the owner (2026-11-13): the app self-hosts the
  design's fonts; site colours, including `theme-color`, are the ones in
  `design-system/README.md`.

## Options and trade-offs

### Q1 — Font format and size (platform)

Measured on the three files (see the Evidence bundle):

| Option | What ships | Inter | Space Grotesk | JetBrains Mono | Total |
| --- | --- | --- | --- | --- | --- |
| A | The TTFs as supplied | 874,708 | 134,112 | 189,028 | 1,197,848 B |
| B | WOFF2, every character | 348,820 | 48,920 | 72,864 | 470,604 B |
| C | WOFF2, Latin subset, every weight | 105,640 | 26,692 | 40,920 | 173,252 B |
| D | C, cut to the weights the design uses | 49,056 | 24,024 | 21,456 | 94,536 B |

D keeps Inter at weights 400–600 with the optical size fixed at its text
setting (14), Space Grotesk at 600–700, and JetBrains Mono at 500 only.

- **A.** Pros: byte for byte from the archive; no tooling. Cons: 1.2 MB
  on a slow phone, against the goal "works on a slow phone over 3G".
- **B.** Pros: every script the fonts cover (Cyrillic, Greek, Vietnamese).
  Cons: 470 kB, most of it characters the app's two languages never use.
- **C (recommended).** Pros: 173 kB; any weight stays available, so a
  later design change needs no new font build. Cons: text outside the
  subset (an item name in Cyrillic, say) shows in the system font, as all
  text does today.
- **D.** Pros: 95 kB. Cons: a weight outside the kept ranges would be
  faked by the browser until the fonts are rebuilt; Inter loses its
  optical-size adjustment (it matters little here, since headings use
  Space Grotesk).

For B, C and D the served files are derived, not the archive's bytes:

- A script beside the sources (`design-system/fonts/build_fonts.py`,
  using `fonttools` and `brotli` at pinned versions) writes them. It is
  run by hand when fonts change, not in CI and not by `cargo build`; it
  adds no Cargo dependency.
- The outputs are committed and a test pins their sha256, as the flags and
  vendored scripts are pinned.
- The OFL allows modified copies when the licence travels with them. With
  no Reserved Font Name, they may keep the families' names.
  `THIRD_PARTY_NOTICES.md` says the served files are subsets.

All options: `@font-face` rules in `app.css` with `font-display: swap`
(text shows at once in the system font and switches when the font
arrives), no preload, and the symbols the templates use that the fonts
lack (☰ ✕ ✓ ▾ ☑ ☐) keep coming from the system font as today.

### Q2 — Icons and fonts as new kinds of asset (platform)

- **Recommended:**
  - Icons in `assets/icons/`, byte for byte from `design-system/assets/`:
    `favicon.svg`, `favicon.ico`, `apple-touch-icon.png`, `icon-192.png`,
    `icon-512.png`, `icon-maskable-512.png` (41,616 B together). Not
    shipped: the 16 and 32 px PNGs (the ICO holds those sizes), the light
    PNGs and the full-size SVGs (nothing links them).
  - Fonts in `assets/fonts/`, as Q1 decides.
  - `build.rs` `CONTENT_TYPES` gains `("png", "image/png")`,
    `("ico", "image/x-icon")` and `("woff2", "font/woff2")` (or
    `("ttf", "font/ttf")` under Q1 A); one `SERVED_AS` row per file.
  - A test pins each file's sha256 and, for the icons, checks it equals
    the file in `design-system/assets/`.
  - Caching: the existing `no-cache` plus `ETag` rule. A full page load
    costs one small "not modified" answer per file; navigation inside the
    app is boosted and reloads nothing.
  - AGENTS.md's assets rule gains a sentence: image files (PNG, ICO, SVG)
    under `assets/icons/` and font files under `assets/fonts/` are not
    executable, come from `design-system/`, and are pinned by checksum in
    the test suite.
- Alternative: put the fonts under `assets/vendor/fonts/` with a version
  in each name, to get the `immutable` cache rule. It saves the "not
  modified" round trips, but `vendor/` today means "byte for byte from a
  reviewed third-party source" (RR-0015), which subset fonts are not.

### Q3 — The colour-switching favicon (platform, security)

`favicon.svg` is light by default and dark when the OS is, through an
inline `<style>`. Two concerns: whether the policy's `style-src 'self'`
stops the style, and that RR-0031's content test forbids `<style>` in the
flags.

Tested: headless Edge 157 (Chromium), with the app's exact policy on both
the page and the SVG response, applies the style (the icon renders in its
dark colours on a machine set to dark, identical to the run without the
policy). Firefox and Safari are not tested here. If a browser did block
the style, the icon would render as a plain black square, since the
colours exist only in the style block.

- **A (recommended): use it as it is.** The file is pinned by checksum.
  Its content test is the flags' test minus the `<style>` ban, plus a ban
  on `@import` (the existing ban on `url(` stays), so the style block can
  colour shapes and nothing else. Browsers without SVG favicon support
  take `favicon.ico`. P8 checks Firefox and Safari; if either shows the
  black square, switch to B without a new consult.
- **B: fixed colours.** Ship one fixed-colour SVG (the Black steel one)
  with no `<style>`, under the flags' test unchanged. The tab icon no
  longer follows the colour scheme. This needs a new file in
  `design-system/assets/` (the existing `source/icon-small-dark.svg`
  qualifies).

The top bar's brand icon (MB-0054 P4) would be the same `favicon.svg`
through `<img>`, so it follows the colour scheme with the page under A.

### Q4 — Where the tokens live at run time (platform)

- **A (recommended): `app.css` holds the token block.** One stylesheet,
  one request. The P1 test reads `design-system/tokens.json` and fails
  when a served token differs, and also checks `design-system/tokens.css`
  against `tokens.json`, since nothing generates that file here.
- **B: serve `tokens.css` as a second stylesheet,** copied to `assets/`
  with a test that the copy is identical. Pros: no hand-kept block. Cons:
  a second render-blocking request on every full page load; the file
  carries rules the app doesn't use (`data-theme` blocks, `.text-*`
  classes).
- **C: `build.rs` generates the block from `tokens.json`.** Pros: can't
  drift. Cons: a JSON parser in the build script (a build dependency, the
  dependency-bump runbook) for about fifty lines of CSS.

### Q5 — The name's form (owner)

- **Recommended:**
  - `kauppalista.app`, lower case, in the page title suffix
    (`<page> — kauppalista.app`), the top bar brand, the home page heading
    and the manifest's `name`.
  - Manifest `short_name`: `kauppalista` (11 characters). Home screens
    cut labels at about 12, so the full name would show as
    `kauppalista.a…`. The same value in an `apple-mobile-web-app-title`
    meta, which iOS uses for its label.
  - `README.md` gains one sentence saying the app is called
    `kauppalista.app`; the repository, crates and settings keep their
    names (a non-goal of the brief).
- Alternative: `short_name` `kauppalista.app` and accept the cut.

### Q6 — A `/favicon.ico` route (api-owner)

Some browsers ask for `/favicon.ico` on their own, whatever the page
links.

- **Recommended: no route.** The `<link rel="icon">` tags point at
  `/assets/icons/`; a browser that also probes the root gets the app's
  usual answer for an unknown path. No public route is added.
- Alternative: serve the ICO at `/favicon.ico` too. One more public,
  unauthenticated route for a cosmetic gain.

### Q7 — Manifest colours (owner)

A manifest takes one `theme_color` and one `background_color`; it can't
follow the colour scheme.

- **Recommended:** both are the light page background `#fbf8f5` (light is
  the default theme). The layout's two `theme-color` metas still switch
  the browser bar between `#fbf8f5` and `#121212`.
- Alternative: `#121212` for both, so the launch screen is dark for
  everyone.

### Q8 — Security (security)

With the recommendations above: the Content-Security-Policy and the other
headers in `csp.rs` don't change; fonts, icons and the manifest load from
the app's own origin under `default-src 'self'` and `img-src 'self'`; no
template gains an inline style or script; no route, cookie or form
changes. The one new thing for security to accept is Q3 A: an SVG with a
`<style>` block under `assets/`, shown through `<link rel="icon">` and
`<img>`, where it can run no script and (by the content test) load
nothing.

## Recommendation and question

Recommended: Q1 C, Q2 as recommended, Q3 A, Q4 A, Q5 as recommended, Q6 no
route, Q7 light, Q8 confirmed.

Because: 173 kB of fonts keeps the design's type without a megabyte on a
slow phone and without locking the weights; the icons and fonts follow the
pattern the flags set (own folder, content type, checksum, a sentence in
AGENTS.md); the favicon works under the policy where it could be tested
and has a decided fallback; one stylesheet stays one request; and nothing
in the security headers moves.

Crisp questions to answer:

1. Fonts: A (TTF, 1.2 MB), B (WOFF2, 471 kB), C (WOFF2 Latin, 173 kB) or
   D (C cut to the used weights, 95 kB)? If B, C or D: a hand-run script
   under `design-system/fonts/` with pinned `fonttools` and `brotli`, its
   outputs committed and pinned by sha256?
2. Assets: icons in `assets/icons/` and fonts in `assets/fonts/`; the
   three content types; the six icon files listed; `no-cache` plus `ETag`;
   and the sentence for AGENTS.md's assets rule?
3. Favicon: A (as it is, with its own content test, falling back to B if
   P8 finds a browser that shows a black square) or B (fixed colours)?
4. Tokens: A (in `app.css`, kept true by the test), B (second stylesheet)
   or C (generated by `build.rs`)?
5. Name: `kauppalista.app` in lower case in those four places;
   `short_name` and the iOS title `kauppalista`; one sentence in
   `README.md`?
6. No `/favicon.ico` route?
7. Manifest `theme_color` and `background_color`: `#fbf8f5`?
8. Security: confirmed that the policy and headers don't change, and that
   the favicon's `<style>` block is acceptable under Q3 A's test?

## Blast radius and rollback

What can break:
- A wrong content type or path shows no icon, or leaves text in the system
  font. The app keeps working either way.
- A font that loads late makes text re-flow once (`font-display: swap`).
- The binary grows by the icons and fonts: about 215 kB under Q1 C,
  1.24 MB under A.
- A browser that blocks the favicon's style shows a black square in the
  tab (Q3 A) until the fallback lands.
- The name change shows in every title and in installed home-screen
  shortcuts, which keep their old label until re-added.

Who is affected: every user, on every page.

Rollback plan: revert the PR. No schema, cookie, route or policy changes,
and no stored data is involved. Browsers hold the files under `no-cache`,
so nothing stale remains.

## Minimal context

- MB-0054 P1–P8; `autonomy.escalate` and `forbidden`.
- RR-0019 (CSP, unchanged). RR-0029 sub-answer 3 and constraint 4 (asset
  caching, unchanged under the recommendation). RR-0031 sub-answers 6–7
  and constraints 1–2 (the flags' pattern; unchanged, the favicon gets its
  own rule). RR-0015 (what `vendor/` means).
- Files the implementation would touch: `assets/app.css`,
  `assets/manifest.webmanifest`, `assets/icons/*` and `assets/fonts/*`
  (new), `templates/layout.hbs`, `templates/index.hbs`,
  `crates/kauppa-web/build.rs`,
  `crates/kauppa-web/src/routes/assets/tests.rs`,
  `crates/kauppa-web/src/templates/tests.rs` and `tests/*.rs`,
  `design-system/fonts/build_fonts.py` (new, under Q1 B–D),
  `THIRD_PARTY_NOTICES.md`, `README.md`.
- Roll-back targets for the RR: AGENTS.md's assets rule (Q2), the brief's
  `resolutions` and its `escalate` list, `design-system/README.md` and
  `design-system/fonts/README.md` (how the fonts are served).

## Evidence bundle (Hardened)

Font sizes, measured 2026-11-13 with `fonttools` 4.65.0 and `brotli` 1.2.1
on the files in `design-system/fonts/`:

| | Characters | TTF | WOFF2 | WOFF2 Latin | WOFF2 Latin, limited axes |
| --- | --- | --- | --- | --- | --- |
| Inter 4.001 | 2,849 | 874,708 | 348,820 | 105,640 | 49,056 (`wght` 400–600, `opsz` 14) |
| Space Grotesk 2.000 | 735 | 134,112 | 48,920 | 26,692 | 24,024 (`wght` 600–700) |
| JetBrains Mono 2.211 | 976 | 189,028 | 72,864 | 40,920 | 21,456 (`wght` 500) |

- Subset ranges: `U+0000-00FF, U+0131, U+0152-0153, U+0160-0161,
  U+017D-017E, U+02BB-02BC, U+02C6, U+02DA, U+02DC, U+0304, U+0308,
  U+0329, U+2000-206F, U+20AC, U+2122, U+2191, U+2193, U+2212, U+2215,
  U+FEFF, U+FFFD`, all layout features kept.
- Characters outside Latin-1 in the templates and both catalogs: `—` `–`
  `“` `”` `…` `↑` `↓` (all inside the subset) and `✕` `✓` `☑` `☐` `☰` `▾`
  (not in the subset; system font, as today).
- The sizes are from one run and were not checked for byte-for-byte
  repeatability across machines; the implementation pins tool versions and
  records the sha256 values.

Favicon under the policy, tested 2026-11-13:

- A local server sent `favicon.svg` and a page holding
  `<img src="favicon.svg">`, once with the app's policy string from
  `csp.rs` and `X-Content-Type-Options: nosniff` on both responses, once
  with neither.
- Headless Microsoft Edge 157.0.4341 took a screenshot of each. Both show
  the same icon in its dark colours (steel tile, orange basket, white
  tick), so the style block applied with the policy in place. The machine
  is set to dark; the light branch of the media query was not exercised.
- Not tested: Firefox, Safari, and the icon in a real tab strip (a
  headless screenshot shows the image, not the tab).

Icons to ship (sha256):

| File | Bytes | sha256 |
| --- | --- | --- |
| `favicon.svg` | 1,022 | `cce6532f406b885b8583766dba3b70747b2ef00a947b0f469ae438b76c3cd5c7` |
| `favicon.ico` | 3,628 | `05127b260746e53f31bbf8838b89e1dbaa508b7e6b99cc9bc2c723a371b0ddca` |
| `apple-touch-icon.png` | 3,758 | `abbe3c733c5d5b809d391c803dd9f8d0b4138736ad3fa38860a695a54fc3b4d4` |
| `icon-192.png` | 6,047 | `33669056e90fe658b9edad757135990e5907792e7ad5f6fe854990ca05c8c348` |
| `icon-512.png` | 17,530 | `3d992c9d1b04ea70f8721a4a50b88702e6dc8ac4bed542f9db76d05a79bd7888` |
| `icon-maskable-512.png` | 9,631 | `16476c61a533ac218f4ae4c810dc4e47ce0b4e9002636cd8143b9dcf2a6d53a3` |

Other evidence:

- `crates/kauppa-web/src/csp.rs:22-24`: no `font-src` or `manifest-src`
  directive, so `default-src 'self'` governs both.
- `image/x-icon` is what `mime_guess` 2 (the source of the existing
  content-type table) gives for `.ico`; not re-measured here.
- Not verified here, for the implementation and P8 to check: what the app
  answers today for `GET /favicon.ico`; iOS's label length; Safari's
  support for SVG favicons.
