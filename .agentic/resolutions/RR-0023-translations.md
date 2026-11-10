# Resolution Record — Translations: in-house `{{t}}` helper, JSON catalogs, `users.language` plus `kauppa_lang` cookie, typed core errors

```yaml
---
id: RR-0023
closes: CRP-0023
mission: MB-0035
decided_by: "@juuso.kalliomaki"
role: platform
risk_tier: medium
decided_at: 2026-11-10T08:12:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/mentorship/templating.md
  - .agentic/missions/MB-0035-finnish-and-english.md
  - .agentic/consults/CRP-0023-translations.md
---
```

## Decision and outcome

Approve CRP-0023's recommendation on every question (Q1–Q10): option **B**,
an **in-house `{{t}}` handlebars helper** over **build-time-embedded JSON
catalogs**, with no new crate; a **nullable `users.language` column** in
`migrations/0009_user_language.sql` plus a **`kauppa_lang` cookie**, set by
**`POST /language`**; **typed core error reasons** mapped to keys in
`kauppa-web`; the **switcher in the layout footer**; and **MB-0035 scheduled
after the other UI briefs**.

Sub-answers:

1. **Mechanism (Q1 B, platform).**
   - A `{{t "key" name=value}}` helper looks the key up in its language's
     catalog and substitutes `{name}` placeholders with **HTML-escaped**
     arguments. Catalog text is trusted and written as is.
   - A missing key or a missing argument is a `RenderError`, never fallback
     text, so strict mode still catches it.
   - **One registry per language**, each with the helper bound to its own
     catalog (`AppState.templates` becomes a per-language set). View structs
     get no `lang` field; partials with sub-contexts need nothing.
   - Rust-side messages use the same catalogs through
     `I18n::text(lang, key, args)`.
   - No dependency: no `Cargo.toml` change, no new package in `Cargo.lock`.
     Not approved: A (`fluent-templates`), A′ (`fluent-bundle`), C
     (`rust-i18n`).
2. **Catalogs (Q2, platform; key names are `may_decide`).**
   - `locales/<tag>.json` (`locales/en.json`, `locales/fi.json`) at the
     workspace root next to `templates/`. `build.rs` embeds every
     `locales/*.json`, as it does `templates/`. The file stem is the BCP 47
     tag written into `<html lang>`.
   - Parsed with `serde_json` (already a direct dependency of `kauppa-web`)
     when the registries are built.
   - Flat dotted keys by area (`lists.empty`, `errors.category_exists`),
     `{name}` placeholders.
   - Plurals: a value may be `{"one": "…", "other": "…"}`, chosen by
     `count == 1`. That is exact for `en` and `fi`. A later language with
     other rules adds a small rule function.
   - No catalog value contains `<` or `&` (P2 enforces it). A sentence around
     a link is split into before / link / after keys.
   - JSON, not TOML (TOML would be a manifest change).
3. **Preference (Q3 A, data-owner).**
   - Exactly one new migration, **`migrations/0009_user_language.sql`**:

     ```sql
     ALTER TABLE users ADD COLUMN language TEXT;
     ```

     with a comment header naming MB-0035 and RR-0023. Nullable, no default,
     no `CHECK` (a `CHECK (language IN ('en','fi'))` would make a third
     language a table rebuild). Purely additive: no backfill, no `DROP`,
     `DELETE` or `UPDATE`; existing rows read NULL ("not chosen").
   - Core validates on write (`Language::parse`) and treats an unknown stored
     value as NULL on read.
   - `UserRepo::language(user_id)` and
     `UserRepo::set_language(user_id, Option<Language>)`.
   - **Resolution order per request:** signed-in user's column →
     `kauppa_lang` cookie → `Accept-Language` (hand-parsed q-values, primary
     subtag `en`/`fi`) → `en`.
   - The column is read on every signed-in request (one primary-key lookup),
     so a change on one device applies on the next request on the others.
   - On registration, the new account's column is set from the cookie when
     one is present, never from `Accept-Language` through
     `UserRepo::set_language` after `InvitationRepo::register` commits, so
     the register transaction (RR-0012 constraint 1) is unchanged.
4. **Cookie (Q4, security).** `kauppa_lang`:
   - value exactly one of the supported tags (`en`, `fi`); anything else is
     ignored on read and never echoed back;
   - `Path=/`, `SameSite=Lax`, `HttpOnly`, `Secure` when
     `KAUPPA_COOKIE_SECURE=true`, `Max-Age` one year (31,536,000 s);
   - unsigned: tampering can only pick another supported language;
   - set by the switch handler, not by middleware. **No change** to the
     session middleware, `kauppa_sid`, or cookie settings in
     `main.rs`/`config.rs`. No session field (Q3 C not approved);
   - HTML responses add `Vary: Cookie, Accept-Language`.
5. **Switch endpoint (Q5, security).**
   - `POST /language` with form field `lang`, registered with `#[post]` in
     `routes::protected`, so it is behind RR-0010's cross-site check and in
     `csrf/tests/coverage.rs` automatically. No GET with side effects, no
     `?lang=` links.
   - Signed in: writes the column and the cookie. Signed out: the cookie
     only. An unsupported `lang` is 400.
   - Answers **303 See Other** to a hidden `return` field holding the current
     request path (and query). It is accepted only if it starts with a single
     `/` (not `//`); otherwise the redirect goes to `/`. No `Referer` use, no
     open redirect.
     - Also reject `return` values that
     contain `\` or control characters (`/\evil.example` is read as
     `//evil.example` by browsers).
   - Not rate-limited.
6. **Core errors (Q6 A, owner).**
   - `CoreError::Invalid(String)` and `Conflict(String)` become typed reasons
     (e.g. `Invalid::NameEmpty(Entity)`, `Invalid::TooMany(Entity)`,
     `Invalid::MultiplierRange`, `Invalid::NoIngredients { recipe }`,
     `Conflict::Exists { entity, name }`). Core stays language-free.
   - Their `Display` keeps today's English text, so logs, `error.rs`'s
     `responses_unchanged` and core tests keep their strings.
   - `kauppa-web` maps every variant to a key and arguments in **one
     exhaustive `match`**: a new variant without a key is a compile error.
     Web-only messages (`INVALID_LOGIN`, "choose a list to add to", …) become
     a web-side enum mapped the same way. `InvitationState` is mapped in web
     (`invitations.state.pending`, …), not through `as_str()`.
   - Not approved: B (message keys in core), C (matching English strings).
7. **Switcher (Q7, owner).**
   - A `<footer>` in `layout.hbs`, on every page including sign-in and
     registration: one POST form with two submit buttons, each named in its
     own language with its own `lang` attribute
     (`<button lang="en">English</button> <button lang="fi">Suomi</button>`),
     the current one marked `aria-pressed="true"`.
   - `hx-boost="false"` on the form, so the answer is a full page load and
     `<html lang>` updates.
   - No JavaScript; works without htmx; usable at 320 px. Independent of
     MB-0040 (the footer is outside the navigation it moves).
8. **Order (Q8, owner).** MB-0035 lands **after** MB-0029, MB-0034 and
   MB-0036–MB-0040, so each string is translated and reviewed once. As the
   CRP allows ("optionally infrastructure early"), the infrastructure may land
   first: catalogs, helper, language resolution, switcher, `<html lang>`, the
   P2/P3 tests, with only the layout converted. The conversion with P1 and P4
   then lands last, and later briefs add keys to both catalogs as they go.
   - Infrastructure-early split is allowed.
9. **Finnish wording (Q9, owner).**
   - User data is **never inflected**. The case goes on a head noun, or the
     name follows a colon or quotes (e.g. "Lopetetaanko jakaminen käyttäjän
     {name} kanssa?", "Lisättiin {items} listaan ”{list}”").
   - Plurals by `one`/`other`. The owner reviews the Finnish in the PR.
   - With this style, the brief's slow-mode trigger (case inflection of user
     data) does not fire.
10. **Dates and error pages (Q10, owner).**
    - Date formats live in the catalog as chrono format strings
      (`format.date`, `format.datetime`), numeric fields only:
      - English **unchanged**: `%-d.%-m.%Y` and `%Y-%m-%d %H:%M UTC`;
      - Finnish: `%-d.%-m.%Y` and `%-d.%-m.%Y klo %-H.%M UTC`.
    - Times stay **UTC**. No `chrono/unstable-locales`, no `chrono-tz`.
    - **Error pages are translated after MB-0029** defines a request-aware
      error page. If MB-0035 must land before MB-0029 after all, the four
      `error.rs` bodies ("not found", "unauthorized", "forbidden", "internal
      server error", plus the cross-site 403) stay English. They are P4's
      only exception, and no middleware is added around the RR-0010 scope.

Covers:

- MB-0035's implementation, as one PR or as the infrastructure-then-conversion
  pair in sub-answer 8, with P1–P6 as written;
- `migrations/0009_user_language.sql` exactly as above, and no other
  migration;
- the `kauppa_lang` cookie and `POST /language`;
- the string changes this conversion needs on security paths:
  `crates/kauppa-web/src/routes/auth/` (`INVALID_LOGIN`, `EMAIL_TAKEN`,
  `INVALID_INVITATION`, the throttle message) and
  `crates/kauppa-web/src/routes/admin/`, with no logic change;
- the `AppState` registry change in `main.rs` (not the session middleware
  block);
- the `CoreError` reshaping in sub-answer 6 (34 sites, 9 handler arms);
- the `templating.md` roll-back below.

P6: **no English output changes.** English strings, dates and error bodies
stay byte-for-byte. The only response change is the added `Vary` header.

Does not cover:

- languages other than `en` and `fi` (each later one is a catalog file, plus
  a plural rule function if it needs one);
- any new dependency, or a `chrono` feature change;
- a session field for the language, or any change to session or cookie
  settings;
- local time zones (a separate brief, `chrono-tz` or a per-user zone);
- translating user data, translated URLs, or `?lang=` links;
- the host CLI (`admin/bootstrap.rs`) and `tracing` messages, which stay
  English;
- error-page design (MB-0029);
- a reverse migration that drops `users.language`, which needs its own RR.

## Rationale and trade-offs

- **In-house helper (Q1 B) — chosen.** The only option with zero dependency
  changes. Escaping, strictness and fallback are ours and unit-tested, and a
  new language is one JSON file. About 150 lines to own, and no CLDR plural
  data, which en/fi don't need.
- **`fluent-templates` (A) — not chosen.** The trial showed it writes
  arguments unescaped (XSS with user data), adds Unicode isolation marks
  (English output changes), renders `Unknown localization key` instead of
  failing, and `.expect`s `lang`. It also has a single owner and adds 15
  packages.
- **`fluent-bundle` (A′) — not chosen.** It is sound, but it adds 10
  packages and a dependency-bump RR for grammar en/fi don't use.
- **`rust-i18n` (C) — not chosen.** It adds 31 packages and 5 new duplicate
  versions, and still needs a handlebars helper.
- **JSON over TOML (Q2) — chosen.** No manifest change. Revisit if
  translators other than the owner edit the files.
- **Column plus cookie (Q3 A) — chosen.** Meets P3's "across sessions and
  devices" and lets signed-out visitors switch. The cost is an additive
  nullable column and one primary-key read per signed-in request. Not chosen:
  B (cookie only: per device), C (session field: purged at sign-out and on
  browser close), D (`Accept-Language` only: no switch, fails P3).
- **Unsigned whitelisted cookie (Q4) — chosen.** A tampered value can only
  select another supported language. Signing it would buy nothing.
- **POST plus 303 (Q5) — chosen.** Keeps safe methods side-effect free and
  rides RR-0010 with no new wiring. The path-only `return` rules out an open
  redirect.
- **Typed reasons (Q6 A) — chosen.** Core stays language-free and the
  exhaustive `match` turns a missing translation into a compile error. Not
  chosen: B (core knows catalog keys; only a scan checks them), C (fragile).
- **Footer switcher (Q7) — chosen.** It is reachable signed out, needs no
  JavaScript and stays clear of MB-0040's panel. Not chosen: the top bar
  (competes with MB-0040), a settings page (none exists; unreachable signed
  out).
- **MB-0035 last (Q8) — chosen.** The other briefs restructure the same
  markup, so translating first would mean rebasing each one and reviewing the
  Finnish twice.
- **No inflection (Q9) — chosen.** The catalog can't inflect, and the
  head-noun style reads naturally.
- **Catalog date formats (Q10) — chosen.** English stays unchanged (P6) with
  no crate. Not chosen: `chrono/unstable-locales` (a feature labelled
  unstable, and it would turn English into `11/09/2026`).

## Approvals and authority

Approved by platform-owner @juuso.kalliomaki (Q1 mechanism and no dependency, Q2
catalogs); by data-owner @juuso.kalliomaki (Q3: `users.language` and
`migrations/0009_user_language.sql`); by security-owner @juuso.kalliomaki (Q4
`kauppa_lang` cookie, Q5 `POST /language`, and the string-only changes under
`routes/auth/` and `routes/admin/`); and by owner @juuso.kalliomaki (Q6 core errors,
Q7 switcher, Q8 order, Q9 Finnish style, Q10 dates and error pages).
Risk tier medium.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0023-translations.md` (branch
  `agentic/mb-0035-i18n`, `72e69cd`)
- Mission Brief: `.agentic/missions/MB-0035-finnish-and-english.md`
- Continuity Pack: `.agentic/continuity/MB-0035.md`
- Related: RR-0010 (cross-site check, pinned wiring in
  `csrf/tests/coverage.rs`); RR-0006 (`page_` layout parameters); RR-0009
  (test paths: `crates/kauppa-web/src/i18n.rs`, `i18n/tests.rs`); RR-0012
  (`routes/auth/` and `routes/admin/` are security paths; register
  transaction); RR-0015 (an already-locked crate is still a manifest change);
  MB-0024 (embedded templates, `build.rs`); MB-0029, MB-0034, MB-0036–MB-0040
  (Q8).
- Evidence links: CRP-0023 → Current state and Evidence bundle:
  - inventory: 203 literal template strings (152 distinct, plus 8 literal
    `page_title`s) and about 45 Rust messages, 3 of them plural;
  - `fluent-templates` 0.15.1 trial output (unescaped `<b>x</b>`, isolation
    marks, `Unknown localization key`);
  - `cargo tree` deltas (+10 / +15 / +31 / +1) and `cargo deny` results;
  - `actix-identity` 0.8 `PurgeSession` and `actix-session` 0.10
    `BrowserSession` defaults;
  - CLDR `one`/`other` for `en` and `fi`;
  - latest migration `0008_invitations.sql`; no branch adds a `0009`
    (checked 2026-11-09).

## Constraints and follow-on rules (Hardened)

1. **All user-facing text through the catalog.** Templates contain no
   literal prose. Text and `placeholder`, `aria-label`, `title`, `alt`,
   `label`, `hx-confirm` values and `page_title` arguments come from `{{t}}`
   (or `(t …)` as a subexpression). P1's scan has a short allowlist (`kauppa`,
   `name@example.com`, symbols); adding to it is a reviewed change.
2. **Arguments escaped, catalogs markup-free.** The helper HTML-escapes every
   argument. No catalog value contains `<` or `&`. No `{{{raw}}}` around a
   translation.
3. **Strict, no silent fallback.** A missing key, argument or plural form is
   a render error, never English fallback or key text. All catalogs have the
   same keys and placeholders (P2).
4. **Core stays language-free.** Core returns typed reasons with English
   `Display`. Only `kauppa-web` maps them to keys, in one exhaustive `match`.
   A string-carrying `Invalid`/`Conflict` variant added later is a review
   failure.
5. **Never inflect or translate user data.** The case goes on a head noun or
   the name follows a colon or quotes.
6. **Language storage.** `users.language` (nullable, validated in core) and
   the `kauppa_lang` cookie are the only places the language is stored. No
   session field. Cookie attributes as in sub-answer 4. Changing them, or
   signing the cookie, needs a security consult.
7. **Switching.** Only `POST /language`, behind the RR-0010 check, changes
   the language. The redirect target is a validated same-site path, never
   `Referer` or an absolute URL.
8. **No new dependency** for translation, plurals or dates without a
   dependency-bump consult.
9. **Schema.** `0009_user_language.sql` is the only migration this RR allows.
10. **Later briefs.** Once the infrastructure is on `main`, a brief that adds
    or changes user-facing text adds it to every catalog in the same PR, and
    the owner reviews the Finnish there.
11. **Rollback.** Revert the PR(s). The `language` column can stay (unused
    and NULL-tolerant), and a stale `kauppa_lang` cookie is then ignored.
    Dropping the column needs a later migration with its own RR.

## Roll-back into the system (Hardened)

To be applied in MB-0035's (first) implementation PR:

- `.agentic/mentorship/templating.md`: add after the "Handlebars" section:

  > ## Translations
  >
  > - All user-facing text comes from the catalogs `locales/<tag>.json`
  >   through the `{{t "key" name=value}}` helper (MB-0035, RR-0023). No
  >   literal prose in templates: text nodes, `placeholder`, `aria-label`,
  >   `title`, `alt`, `label`, `hx-confirm` and `page_title` all use `{{t}}`
  >   or `(t "…")`. `i18n::tests::templates_have_no_literal_text` enforces
  >   it.
  > - Keys are flat and dotted by area (`lists.empty`), placeholders are
  >   `{name}`, and plurals are `{"one": "…", "other": "…"}`. Every catalog
  >   has the same keys and placeholders. Values contain no `<` or `&`: a
  >   sentence around a link is split into before / link / after keys.
  > - One key per whole sentence. Never build a message by concatenating
  >   translated pieces. Never inflect user data: put the case on a head
  >   noun, or put the name after a colon or in quotes.
  > - Messages built in Rust use `I18n::text`. Core errors are typed reasons
  >   mapped to keys in `kauppa-web`, never English text shown as is.
  > - A change that adds or edits user-facing text updates every catalog in
  >   the same PR.

  and add to "Red flags":

  > - A literal string in a template, or a `format!` that builds user-facing
  >   text in a handler (RR-0023).

- `.agentic/missions/MB-0035-finnish-and-english.md`:
  - `resolutions` gains `RR-0023`;
  - P3's "as the RR decides" points at RR-0023 (column → cookie →
    `Accept-Language` → `en`);
  - P4's error pages follow RR-0023 sub-answer 10;
  - P6: RR-0023 lists no English string changes.
- `CRP-0023`: `state: open` → `answered`.
- No change to `security.md` or `persistence.md`: the CRP proposes none.
  There is no session or cookie-settings change, and `UserRepo` is already
  in the trait list.
- Record `kauppa_lang` under security.md's session-store rule.

## Supersession and history

Status: active once signed. Supersedes nothing. The first translation rule
in `templating.md`.
