# Resolution Record — Language dropdown in the top bar, with MIT-licensed SVG flags (amends RR-0023)

```yaml
---
id: RR-0031
closes: CRP-0031
mission: MB-0051
decided_by: "@Jurzo"
role: product
risk_tier: low
decided_at: 2026-11-12T14:38:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/resolutions/RR-0023-translations.md
  - AGENTS.md
  - .agentic/missions/MB-0051-language-dropdown-in-top-menu.md
  - .agentic/consults/CRP-0031-language-dropdown.md
---
```

## Decision and outcome

Approve CRP-0031 as recommended for Q1–Q5 and Q7–Q9, and **option (b)**
for Q6: the flags come from **`lipis/flag-icons` 7.6.0** (MIT), with the
**United Kingdom's flag (`gb`) for English**. The owner-supplied files
(apparently Nucleo exports, licence unconfirmed) are not used.

Sub-answers:

1. **Amend RR-0023 sub-answer 7 (Q1).** The switcher moves from the
   `<footer>` to the top bar as a dropdown; the footer switcher is removed.
   Kept from sub-answer 7: one POST form to `/language` with the `return`
   field, one submit button per language with its own `lang` attribute and
   its name in that language, `hx-boost="false"`, no JavaScript, usable at
   320 px.
2. **Mechanism (Q2).** A `popover` list opened by a button
   (`popovertarget`), as MB-0040's navigation panel is. In a browser
   without popover support the language buttons show inline.
3. **Semantics (Q3).** The current language's button carries
   `aria-current="true"` and a decorative check mark (`aria-hidden`); no
   `aria-pressed`, no ARIA menu roles. The opening button's accessible
   name is "Language: <current language's name>" from the catalogs.
4. **Phones (Q4).** In the top bar between the brand and the menu button,
   showing the short name (`EN`, `FI`) beside the flag; full names on wide
   screens. If it doesn't fit at 320 px, tighten spacing or drop the
   chevron on phones; never hide the brand, the menu button or the
   selector.
5. **Flags (Q5, Q6 (b)).**
   - English: `gb`; Finnish: `fi`. A later language's flag is chosen in
     the brief that adds it.
   - Source: the npm tarball
     `https://registry.npmjs.org/flag-icons/-/flag-icons-7.6.0.tgz`,
     whose sha512 equals npm's `dist.integrity`
     `sha512-hir/PsXwr0EESyizKrXJzmkzpLIMf+1vlNKLqAfvdh+bsU94a7LYWTiubkHKqptGoCPzOUGcm6c4PfC9/BaGSQ==`
     (checked 2026-11-12).
   - Files, byte for byte, the 4:3 versions:
     - `package/flags/4x3/gb.svg` → `assets/flags/en.svg` (504 B, sha256
       `c8be1e7208798a4ae692ee1e937065d498bb29e741943f6172b29118b8ed8066`);
     - `package/flags/4x3/fi.svg` → `assets/flags/fi.svg` (234 B, sha256
       `012edd984cf1879463c2855b7df02dea2f7de7f7a66b244fb1e22b4227f7aa65`).
   - Licence: MIT, copyright 2013 Panayiotis Lipiridis. The notice goes in
     a new root file `THIRD_PARTY_NOTICES.md` (a file under `assets/`
     would be served, and `build.rs` refuses unknown extensions).
6. **Location and serving (Q7).** `assets/flags/<language tag>.svg`;
   `("svg", "image/svg+xml")` in `build.rs`'s `CONTENT_TYPES`; a
   `SERVED_AS` row per file; the existing `no-cache` plus `ETag` rule
   (RR-0029), not `vendor/`'s immutable one.
7. **Not executable (Q8).** SVG images shown through `<img>` run no script.
   A test keeps them so: no `<script>`, `on*` attribute, `href`,
   `xlink:href`, `url(`, `<use>`, `<image>`, `<foreignObject>`, `<style>`
   or `style=`. The sha256 values above are also checked by a test, so the
   files stay byte for byte as vendored. AGENTS.md's assets rule gains one
   sentence (roll-back).
8. **Security (Q9).** `POST /language`, the `kauppa_lang` cookie and the
   Content-Security-Policy don't change; `img-src 'self'` covers the
   flags. Nothing further from security.

Covers:

- MB-0051's implementation PR, P1–P6;
- `assets/flags/en.svg`, `assets/flags/fi.svg`, `THIRD_PARTY_NOTICES.md`,
  the `build.rs` content type and the tests above;
- the changes to `layout.hbs`, `app.css`, both catalogs, and the tests
  named in CRP-0031 (including narrowing MB-0040's `panel()` helper to the
  navigation's opener);
- the roll-backs below.

Does not cover:

- the owner-supplied Nucleo-style files, or any other flag source or
  version;
- JavaScript, ARIA menu patterns, or any change to `/language`, the cookie
  or the CSP;
- a flag for a third language.

## Rationale and trade-offs

- **flag-icons (b).** MIT fits this public MIT repository; the supplied
  files' licence couldn't be confirmed, and a proprietary licence would
  not cover redistribution here. Pinning a version and hashing the files
  follows RR-0015's vendoring discipline without the `vendor/` caching.
- **`gb` for English.** The owner's choice; a flag stands for the language
  here, and the name is always shown as text.
- **Popover.** Closes on Escape and on a click outside, without script;
  `<details>` does neither, and `<select>` can't show flags.
- **No menu roles.** They promise arrow-key navigation that needs script.

## Approvals and authority

Approved by the owner (product: Q1–Q6, the AGENTS.md sentence), with
platform-owner for Q7–Q8 (asset type, location, vendoring) and
security-owner confirming Q9.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0031-language-dropdown.md`
- Mission Brief: `.agentic/missions/MB-0051-language-dropdown-in-top-menu.md`
- Amends: RR-0023 sub-answer 7.
- Related: RR-0019 (CSP, `img-src 'self'`), RR-0029 (asset caching),
  RR-0015 (vendoring discipline), MB-0040 (navigation popover).
- Evidence: CRP-0031 → Evidence bundle; the tarball check above.

## Constraints and follow-on rules (Hardened)

1. **Flags are inert images.** The content test stays; a flag file that
   fails it is not added.
2. **Flags are pinned.** File, sha256 and source version change together,
   under an RR; the notice in `THIRD_PARTY_NOTICES.md` stays.
3. **No script for the dropdown.** Opening and closing stay native
   (`popover`); a feature needing script is escalated.
4. **One form, one endpoint.** Switching stays a plain POST to
   `/language`; no GET, no `?lang=`.

## Roll-back into the system (Hardened)

To be applied in MB-0051's implementation PR:

- `RR-0023` → "Supersession and history": add "Amended by RR-0031
  (2026-11-12): the switcher is a dropdown in the top bar with SVG flags,
  not footer buttons (sub-answer 7); the rest of sub-answer 7 stands."
- `AGENTS.md` → Do-not-touch boundaries, the `assets/` bullet: after "…a
  platform decision." add "SVG images under `assets/flags/` are not
  executable: they are shown through `<img>`, pinned by checksum, and a
  test keeps them free of script, links and styles (RR-0031)."
- `MB-0051`: `resolutions: []` → `[RR-0031]`; P5's property names
  `assets/flags/` and flag-icons 7.6.0; Conceptual plan step 1 gains
  "Answered by RR-0031."
- `CRP-0031`: `state: open` → `answered`.

## Supersession and history

Status: active once approved. Amends RR-0023 sub-answer 7.
