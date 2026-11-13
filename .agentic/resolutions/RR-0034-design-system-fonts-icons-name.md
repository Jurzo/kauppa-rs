# Resolution Record — Design system adoption: subset WOFF2 fonts and icons under `assets/`, the adaptive favicon, tokens in `app.css`, and the name `kauppalista.app`

```yaml
---
id: RR-0034
closes: CRP-0034
mission: MB-0054
decided_by: "@Jurzo"
role: platform
risk_tier: low
decided_at: 2026-11-13T15:28:00Z
status: approved
superseded_by: null
rolls_back_into:
  - AGENTS.md
  - .agentic/missions/MB-0054-design-system-and-branding.md
  - .agentic/consults/CRP-0034-design-system-fonts-icons-name.md
  - design-system/README.md
  - design-system/fonts/README.md
---
```

## Decision and outcome

Approve CRP-0034's **recommended set, questions 1–8** (owner, 2026-11-13:
"all as recommended").

Sub-answers:

1. **Fonts (Q1 C, platform).** The app serves Inter, Space Grotesk and
   JetBrains Mono as WOFF2 files cut to the Latin subset, every weight of
   each variable font kept (about 173 kB together).
   - Subset ranges: `U+0000-00FF, U+0131, U+0152-0153, U+0160-0161,
     U+017D-017E, U+02BB-02BC, U+02C6, U+02DA, U+02DC, U+0304, U+0308,
     U+0329, U+2000-206F, U+20AC, U+2122, U+2191, U+2193, U+2212, U+2215,
     U+FEFF, U+FFFD`, all layout features kept.
   - A script, `design-system/fonts/build_fonts.py`, writes them from the
     TTFs in `design-system/fonts/` with `fonttools` and `brotli` at
     versions the script pins. It is run by hand, not in CI and not by
     `cargo build`, and adds no Cargo dependency.
   - The outputs are committed and a test pins each file's size and
     sha256.
   - `@font-face` rules in `assets/app.css` with `font-display: swap`; no
     preload.
   - `THIRD_PARTY_NOTICES.md` names the served files as subsets of the
     three families under the SIL OFL 1.1, with the licence texts in
     `design-system/fonts/`.
2. **Assets (Q2, platform).**
   - Icons in `assets/icons/`, byte for byte from `design-system/assets/`:
     `favicon.svg`, `favicon.ico`, `apple-touch-icon.png`, `icon-192.png`,
     `icon-512.png`, `icon-maskable-512.png`, with the sha256 values in
     CRP-0034's Evidence bundle.
   - Fonts in `assets/fonts/`.
   - `build.rs` `CONTENT_TYPES` gains `("png", "image/png")`,
     `("ico", "image/x-icon")` and `("woff2", "font/woff2")`; one
     `SERVED_AS` row per file.
   - A test pins each file's sha256 and checks that each icon equals its
     file in `design-system/assets/`.
   - Caching: the existing `no-cache` plus `ETag` rule (RR-0029), not
     `vendor/`'s immutable one.
   - AGENTS.md's assets rule gains one sentence (roll-back).
3. **Favicon (Q3 A, platform and security).** `favicon.svg` is used as it
   is, for the tab icon and for the top bar's brand icon.
   - It is pinned by checksum.
   - Its content test is the flags' test without the `<style>` ban and
     with a ban on `@import`; the ban on `url(` stays.
   - `favicon.ico` is linked for browsers without SVG favicon support.
   - If MB-0054's browser check (P8) finds a browser that shows the icon
     as a black square, the app switches to a fixed-colour SVG with no
     `<style>` (`design-system/assets/source/icon-small-dark.svg`), under
     the flags' content test unchanged, without a new consult.
4. **Tokens (Q4 A, platform).** `assets/app.css` holds the token block; no
   second stylesheet. The P1 test compares the served tokens with
   `design-system/tokens.json`, and `design-system/tokens.css` with
   `tokens.json`.
5. **Name (Q5, owner).** `kauppalista.app`, lower case, in the page title
   suffix (`<page> — kauppalista.app`), the top bar brand, the home page
   heading and the manifest's `name`. The manifest's `short_name` and an
   `apple-mobile-web-app-title` meta are `kauppalista`. `README.md` gains
   one sentence naming the app; the repository, crates and settings keep
   their names.
6. **No `/favicon.ico` route (Q6, api-owner).** The icon links point at
   `/assets/icons/`; no route is added.
7. **Manifest colours (Q7, owner).** `theme_color` and `background_color`
   are `#fbf8f5`. The layout's two `theme-color` metas are `#fbf8f5`
   (light) and `#121212` (dark).
8. **Security (Q8, security).** The Content-Security-Policy and the other
   headers in `csp.rs` don't change. The favicon's `<style>` block is
   accepted under sub-answer 3's test.

Covers:

- MB-0054's implementation PRs, P1–P8;
- `assets/icons/*`, `assets/fonts/*`, the three `build.rs` content types
  and the tests above;
- `design-system/fonts/build_fonts.py` and the notice in
  `THIRD_PARTY_NOTICES.md`;
- the changes to `layout.hbs`, `index.hbs`, `app.css`, the manifest and
  `README.md` that the sub-answers name;
- the roll-backs below.

Does not cover:

- any change to `csp.rs`, a font or file from another host, or a route
  outside `/assets/`;
- font files in another format or with another character set, italics, or
  a fourth family;
- an `immutable` cache rule for fonts or icons;
- a theme chooser, or the behaviour listed in MB-0054's non-goals;
- renaming the repository, crates, settings or cookies.

## Rationale and trade-offs

- **Latin WOFF2 (C).** 173 kB instead of 1.2 MB keeps the design's type
  usable on a slow phone. Keeping every weight (rather than option D's
  95 kB) means a later design change needs no new font build. Text outside
  the subset falls back to the system font, as all text does today.
- **Own folders, existing cache rule.** Follows the flags' pattern
  (RR-0031). `vendor/` keeps meaning "byte for byte from a reviewed
  third-party source" (RR-0015), which subset fonts are not.
- **Favicon as it is.** Headless Edge applies its style under the app's
  policy (CRP-0034's Evidence bundle); the fallback is decided in advance
  for a browser that doesn't.
- **Tokens in `app.css`.** One stylesheet stays one request; the test
  keeps it true to `tokens.json` without a build dependency.
- **`short_name` `kauppalista`.** Home screens cut labels at about 12
  characters.

## Approvals and authority

@Jurzo as platform-owner for Q1, Q2 and Q4; as platform and security for Q3; product for Q5 and Q7; api-owner for Q6; security for Q8.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0034-design-system-fonts-icons-name.md`
- Mission Brief: `.agentic/missions/MB-0054-design-system-and-branding.md`
- Related: RR-0019 (CSP, unchanged), RR-0029 (asset caching, unchanged),
  RR-0031 (the flags' pattern and content test), RR-0015 (what `vendor/`
  means).
- Evidence: CRP-0034 → Evidence bundle (font sizes, the favicon test, icon
  checksums).

## Constraints and follow-on rules (Hardened)

1. **Nothing from another host.** Fonts, icons and styles load from the
   app's own origin; the policy isn't loosened for them.
2. **Icons are pinned and come from `design-system/assets/`.** A served
   icon equals its source file; file and sha256 change together.
3. **Fonts are pinned and reproducible.** The served files come from
   `build_fonts.py` and the TTFs in `design-system/fonts/`; file, sha256
   and the script's inputs change together. A different format, character
   set or family needs an RR.
4. **The favicon's style block only colours shapes.** No `@import`, no
   `url(`, no script, handlers or links. No other SVG under `assets/` may
   carry `<style>`.
5. **`immutable` stays for `vendor/` only** (RR-0029 constraint 4).
6. **`app.css` tokens equal `tokens.json`.** A token changes in
   `design-system/tokens.json` first; components read tokens, not raw
   colours.

## Roll-back into the system (Hardened)

To be applied after this record is signed, in the consult's PR or
MB-0054's first implementation PR:

- `AGENTS.md` → Do-not-touch boundaries, the `assets/` bullet: after "…and
  a test keeps them free of script, links and styles (RR-0031)." add "The
  app icons under `assets/icons/` (PNG, ICO and one SVG) and the font
  files under `assets/fonts/` (WOFF2) are not executable: they come from
  `design-system/`, are pinned by checksum in the test suite, and the
  icon SVG's style block may only colour its shapes (RR-0034)."
- `MB-0054`: `resolutions: []` → `[RR-0034]`; the `escalate` items this
  record answers are removed or narrowed (fonts, new asset kinds, the
  favicon, where the tokens live, the name's form); P4 names `short_name`
  `kauppalista`; P5 names `assets/icons/`; a font property and check are
  added (the served fonts and their `@font-face` rules); Conceptual plan
  step 1 gains "Answered by RR-0034."
- `CRP-0034`: `state: open` → `answered`.
- `design-system/README.md` and `design-system/fonts/README.md`: how the
  fonts are served (the subset, the script, `assets/fonts/`), replacing
  "a decision for MB-0054".

## Supersession and history

Status: active once approved.
