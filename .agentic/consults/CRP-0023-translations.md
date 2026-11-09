# Consultation Request Pack — Translations: mechanism, language preference, core errors and the switcher (MB-0035)

```yaml
---
id: CRP-0023
mission: MB-0035
owner: "@juuso-kalliomaki"
target_role: platform
risk_tier: medium
urgency: low
state: open
runbook_trigger: "MB-0035 autonomy.escalate: the translation mechanism, a crate or an in-house helper (platform-owner; dependency-bump.md if a crate is chosen: options A and C add one, the recommended B adds none); where the language preference lives (data-owner; a `users` column is a new migration `0009_user_language.sql`, feature.md escalation path `migrations/**`, migration runbook entry gate); a language cookie or session field (security; the recommendation is a new `kauppa_lang` cookie set by a handler, with no change to the session middleware or its cookie settings); how core errors become user text (owner); the switcher's place in the layout (owner, interacts with MB-0040). feature.md escalation paths touched by the conversion: `crates/kauppa-web/src/routes/auth/` (login/register strings, `INVALID_LOGIN` and friends) and `crates/kauppa-web/src/routes/admin/` (security), and `crates/kauppa-web/src/main.rs` (the registry line in `AppState` changes; the session middleware block does not). Routed to platform, with Q3 to data-owner, Q4–Q5 to security and Q6–Q10 to the owner."
---
```

## Decision statement

Decide how MB-0035 puts English and Finnish side by side:

- the translation mechanism, and whether it brings a dependency (platform);
- the catalog files' format and where they live (platform);
- where the language preference lives, and the resolution order (data-owner);
- the language cookie and the switch endpoint (security);
- how `CoreError` messages become translated text (owner);
- where the language switcher sits, when the dates change, how Finnish
  wording handles user data, and where MB-0035 goes in the queue of UI
  briefs (owner).

**Routing.** All roles are @juuso-kalliomaki, so one RR can answer every
question, as RR-0010 and RR-0012 did.

## Current state

Verified on `origin/main` `50322c0` (2026-11-09). No code was changed; the
dependency trials ran in a scratch copy outside the repository.

### String inventory

Counted with a scratch script over `templates/**/*.hbs`: literal prose in
text nodes, and the literal part of `placeholder`, `aria-label`,
`hx-confirm`, `title` and `label` attributes. A run with an inline
`{{value}}` counts once (it becomes one message with a placeholder); block
helpers (`{{#if}}`) split runs. The brand `kauppa` is excluded.

| Area (templates) | Literal strings | Notes |
| --- | --- | --- |
| `layout.hbs` + `index.hbs` | 11 | nav (4), sign in/out, hero text |
| `auth/` | 13 | login 5, register 8 |
| `lists/` | 77 | `_list` 35, `_index` 12, `_shop` 8, `_share_form` 7, `_grantees` 6, pages 9 |
| `recipes/` | 45 | `_ingredients` 17, `_add_to_list` 9, `_details` 5, `show` 5, others 9 |
| `products/` | 26 | `_catalog` 17, `_product_edit` 6, others 3 |
| `categories/` | 13 | |
| `admin/` | 18 | |
| **Templates total** | **203** | **152 distinct texts**; plus 8 literal `page_title="…"` arguments (211 occurrences) |

Messages built in Rust that reach a page (grep of non-test code for
`CoreError::Invalid(`/`Conflict(`, string constants and `format!`):

| Source | Distinct messages | Sites |
| --- | --- | --- |
| `kauppa-core` `CoreError::Invalid`/`Conflict` | 17 (20 keys: "`{what}` name must not be empty" is 4 nouns) | 21, in `repo/sqlite/{category,list,mod,product,recipe}.rs`, `domain/{invitation,share}.rs` |
| `kauppa-core` `InvitationState::as_str` ("pending", "used", "revoked", "expired"), shown as-is on `/admin/invitations` | 4 | `domain/invitation.rs:42-45` |
| `kauppa-web` handlers, `CoreError::Invalid(...)` | 10 new (3 more duplicate core's) | 13, in `routes/lists/{items,mod,store}.rs`, `routes/products/mod.rs`, `routes/recipes/{add_to_list,ingredients,mod}.rs` |
| `routes/auth/mod.rs` | 4 | `INVALID_INVITATION`, `INVALID_LOGIN`, `EMAIL_TAKEN`, `too_many_attempts` (plural) |
| `routes/products/mod.rs` | 4 | delete confirmation, delete warning, "Not used in any recipe", "Used in N recipe(s)" (plural) |
| `routes/recipes/mod.rs` | 2 | "N item(s)" (plural), "the list" fallback |
| `error.rs` bodies | 4 | "not found", "unauthorized", "forbidden" (also `csrf/mod.rs:211`), "internal server error"; plain text, not a page |
| **Rust total** | **≈45** | 3 of them need plurals |

Not user-facing, left alone: `tracing` messages, the host CLI
(`admin/bootstrap.rs`: `USAGE`, `GRANT_*`, the invitation-link line), and
internal `Invalid` texts ("bad database url", "migration failed", "invalid
uuid"). `assets/manifest.webmanifest` holds only the brand name.

**Dates and times** (two formats, six render points):

- `routes/lists/mod.rs:322`: `%-d.%-m.%Y` (already Finnish-style, e.g.
  `9.11.2026`), shown as "Updated {{updated}}" in `lists/_list` and in both
  row kinds of `lists/_index`.
- `routes/admin/mod.rs:126`: `%Y-%m-%d %H:%M UTC`, for created, expires,
  used and the new invitation's expiry.
- Both are UTC; no time zone conversion exists today.

**`<html lang="en">`** is a literal in `layout.hbs:2`. Note that
`hx-boost="true"` on `<body>` swaps only the body: a boosted navigation
updates `<title>` but **not** `<html lang>`, so the switch must be a full
page load (Q7).

**Strings built by concatenation** (slow-mode trigger in the brief):

- `routes/products/mod.rs:123-133`: confirmation = "Delete “X”?" + " " +
  warning; warning = "Used in " + `recipes(n)`; usage label likewise.
- `routes/recipes/mod.rs:384-392`: "N items" and the fallback "the list",
  then slotted into the template's "Added {{items}} to {{list_name}} —".
- `repo/sqlite/mod.rs:139`: `"{what} name must not be empty"` with `what` in
  {product, category, list, item}: Finnish needs a whole sentence per noun.
- `routes/auth/mod.rs:174-178`: minutes plural.
- Sentences split around a link, 7 of them: "Save <a>favorites</a> to add
  them here in one tap.", "<a>Create a category</a> to group items.",
  "Favorites/Ingredients are grouped by category — <a>create a category</a>
  first.", "No shopping lists yet — <a>create a list</a> first.", "Added … —
  <a>open list</a>", "Already have one? <a>Sign in</a>.".
- `admin/invitations.hbs`: "Used {{used}}{{#if}} by {{name}} ({{email}}){{/if}}."
  and "Invitation created{{#if note}} ({{note}}){{/if}}." span block helpers.

All of these are solvable inside the catalog (one key per whole sentence,
link sentences as before/link/after parts), with only mechanical handler
changes: no handler *logic* changes.

**Finnish grammar on user data.** User data appears inside prose in about 20
messages: "Share {{name}}", "Stop sharing this list with {{name}}?",
"Added {{items}} to {{list}}", "Mark {{text}} found", "Remove {{text}} — not
needed", "Move {{name}} up/down", "Delete {{name}}", "Name of {{name}}",
"Delete the list/recipe “{{name}}” …", "{{title}} has no ingredients",
"{{name}} has no category …", "Uses {{category}} …", "choose a category for
{{name}} …", "category/product already exists: {{name}}", "Used … by
{{name}}". Literal Finnish would inflect the name ("Matin kanssa",
"listaan Viikonloppuun", "Maidolle"). Each one can instead carry the case
on a head noun and leave the name in the nominative, e.g.:

- "Lopetetaanko jakaminen käyttäjän {name} kanssa?"
- "Lisättiin {items} listaan ”{list}”"
- "Valitse tuotteelle {name} kategoria"
- "Reseptissä ”{title}” ei ole aineksia"
- "Kategoria on jo olemassa: {name}"

So the brief's slow-mode trigger ("a key needing grammatical case inflection
of user data") **does not fire**, provided the owner accepts this style
(Q9).

**Plurals.** Three Rust messages (items, recipes, minutes). CLDR plural
rules for both `en` and `fi` are `one` (exactly 1, no fraction) and `other`,
so a `one`/`other` pair with `n == 1` is exact for both. In-template counts
are all in parentheses ("People with access (3)") and need none.

### Code facts

- **Rendering.** `templates.rs` builds one strict `Handlebars<'static>` from
  `TEMPLATES`, which `build.rs` generates from `templates/`. `AppState`
  holds it as `templates: Arc<Registry>` (`main.rs`). Handlers call
  `state.templates.render(name, &view)` (31 call sites).
- **Errors.** Handlers catch `CoreError::Invalid(m)` and put `m` into the
  view's `error` field (9 match arms, e.g. `categories.rs:79`,
  `lists/mod.rs:286`); what isn't caught becomes `WebError::error_response`,
  a plain-text body with no request (so no language) in reach.
  `error.rs`'s `responses_unchanged` pins those bodies.
- **Session.** `CookieSessionStore`, cookie `kauppa_sid`, default lifecycle
  (`BrowserSession`: ends with the browser). `actix-identity` 0.8's default
  `LogoutBehaviour::PurgeSession` purges the whole session on sign-out
  (`identity.rs:189-193`), so any session field is lost at sign-out.
- **Cross-site check (RR-0010).** Every handler registered in
  `routes::protected` is behind `csrf::guard`; `csrf/tests/coverage.rs`
  pins that `routes::configure` registers exactly one wrapped scope and that
  `main.rs` registers nothing else. A new `#[post("/language")]` handler is
  covered automatically; wrapping that scope in another middleware (e.g.
  `ErrorHandlers`) would change pinned wiring.
- **Users.** `users(id, email, display_name, password_hash, created_at)`
  (`0001_init.sql`), `STRICT`. Latest migration `0008_invitations.sql`.
  `UserRepo` has `create` and `find_by_email`; handlers never load the user
  row per request today (`request::current_user` parses the identity id).
- **Dependencies already in the tree.** `serde_json` 1.0.154 is a direct
  dependency of `kauppa-web`. `toml` 1.1.8 is in `Cargo.lock` (through
  `config`) but not a direct dependency. `chrono` 0.4.45 with `clock`,
  `serde` only. `handlebars` 6.4.4.

## Questions, options and recommendations

### Q1 — Translation mechanism (platform)

Supply-chain data from the crates.io API (2026-11-09). Transitive counts
are new packages in `cargo tree -p kauppa-web -e normal,build` over today's
256, measured in a scratch copy with `cargo add`. `cargo deny check
advisories licenses bans` ran in each copy.

| Crate | Version (date) | Owners | Downloads all / recent | License | New packages | `cargo deny` |
| --- | --- | --- | --- | --- | --- | --- |
| `fluent-bundle` (+ `unic-langid`) | 0.16.0 (2025-05-22) | waywardmonkeys, zbraniecki, alerque, nordzilla, dminor, gregtatum (projectfluent) | 23.7 M / 8.6 M | Apache-2.0 OR MIT | +10 | ok |
| `fluent-templates`, features `handlebars,macros,walkdir`, no defaults | 0.15.1 (2026-08-05) | XAMPPRocky (single owner) | 1.33 M / 0.28 M | MIT/Apache-2.0 | +15 (incl. `proc-macro-hack 0.5.20+deprecated`) | ok |
| `rust-i18n` | 4.2.4 (2026-09-29) | huacnlee; support crate also eyeplum, `github:longbridgeapp:rust` | 4.68 M / 1.75 M | MIT | +31 (YAML parser, `toml` 0.8, `ignore`, `globwalk`, …) | ok, but 5 new duplicate warnings (`bitflags`, `toml`, `toml_datetime`, `serde_spanned`, `winnow`) |
| `pure-rust-locales` (via `chrono/unstable-locales`) | 0.8.2 | djc, pitdicker | 8.6 M / 1.5 M | MIT OR Apache-2.0 | +1 | — |

Transitive Fluent crates: `fluent-syntax`, `intl-memoizer`, `fluent-langneg`,
`intl_pluralrules`, `type-map`, `self_cell` (Apache-2.0 OR GPL-2.0-only,
taken as Apache-2.0), `rustc-hash`, `unic-langid-impl`; all with tens of
millions of downloads. No RustSec advisories for any of them.

**Option A — Fluent through `fluent-templates`' handlebars helper**

A trial program (scratch, handlebars 6.4.4, strict mode, `static_loader!`)
rendered `{{fluent "share-title" name=name}}` and friends:

- **The helper doesn't escape its arguments.** With `name = "<b>x</b>"` the
  output was `Share ⁨<b>x</b>⁩`: user data is written raw
  (`loader/handlebars.rs` writes the formatted string straight to `out`).
  That is an XSS hole for every message with a list, product or person name,
  against security.md's "never `{{{raw}}}` on user-provided content". A
  wrapper helper that escapes args first would be needed.
- **Unicode isolation marks** (U+2068/U+2069) wrap every placeable by
  default, so English output changes byte-wise (P6) unless the loader is
  built with `customise: |b| b.set_use_isolating(false)`.
- **A missing key renders `Unknown localization key: "nope"`**, not a
  render error, which defeats strict mode; P4 would have to grep for it.
- `lang` is read from the root context with `.expect(...)`: a render without
  it panics inside the dependency instead of returning an error.
- `macros` without `walkdir` or `ignore` doesn't compile.
- Pros: Fluent's grammar (selectors, plural categories from CLDR, terms) is
  the most capable; `.ftl` files are a standard translators know.
- Cons: the four items above; a single-owner crate on the render path;
  +15 packages; more than en/fi need.
- Risk: medium (escaping must be wrapped correctly everywhere).

**Option A′ — `fluent-bundle` directly, behind an in-house helper**

- Pros: the projectfluent team's crates; our helper controls escaping,
  missing keys (error) and isolation; +10 packages.
- Cons: a dependency and a dependency-bump RR for features B gets with no
  crate; Fluent's `.ftl` syntax for a two-language, one/other app.
- Risk: low.

**Option B — In-house helper over embedded JSON catalogs (recommended)**

- `locales/<lang>.json` per language, embedded by `build.rs` like
  `templates/`; parsed with `serde_json` (already a direct dependency) when
  the registry is built. **No manifest change, no new package.**
- A `{{t "key" name=value}}` helper: looks the key up in that language's
  catalog, substitutes `{name}` placeholders with **HTML-escaped**
  arguments, and returns a `RenderError` for a missing key or a missing
  argument (so strict-mode guarantees carry over). Catalog text is trusted
  and written as-is; P2 rejects `<` and `&` in catalog values so no markup
  hides there.
- One registry per language, each with the helper bound to its own catalog
  (`AppState.templates: Translations` keyed by language), so view structs
  don't need a `lang` field and partials with sub-contexts need nothing.
- Plurals: a value may be `{"one": "…", "other": "…"}`, chosen by
  `count == 1` (exact for en and fi; a later language with other rules gets
  a small rule function).
- Rust-side messages use the same catalog through `I18n::text(lang, key,
  args)`.
- Pros: no supply-chain surface; escaping, strictness and fallback behaviour
  are ours and tested; adding a language is one JSON file (the goal).
- Cons: ~150 lines of code to own; no CLDR plural data (not needed for en/fi,
  see the brief's non-goal).
- Risk: low.

**Option C — `rust-i18n`**

- Pros: popular, a `t!` macro, YAML/JSON/TOML catalogs.
- Cons: +31 packages and 5 new duplicate versions; no handlebars
  integration (a helper still has to be written); its global, macro-time
  catalog loading duplicates what `build.rs` already does.
- Risk: low to medium.

**Recommendation: B.** It is the only option with zero dependency changes,
and the trial shows A needs wrapping on exactly the points that matter
(escaping, missing keys). This CRP doubles as the dependency-bump consult
should the owner prefer A′ (the table above is its supply-chain check).

### Q2 — Catalog format and layout (platform; key names are the agent's)

- **Location:** `locales/en.json`, `locales/fi.json` at the workspace root,
  next to `templates/`; `build.rs` lists every `locales/*.json`, so a third
  language is one new file. The file stem is the BCP 47 tag used in
  `<html lang>`.
- **Shape:** flat dotted keys by area (`lists.empty`, `shop.found_of`,
  `errors.category_exists`), `{name}` placeholders, plural objects as in Q1.
- **JSON vs TOML.** TOML reads better (comments, no quoting) and `toml`
  1.1.8 is already locked, but using it is a `Cargo.toml` line (a
  dependency-bump, as with `sha2` in RR-0015). JSON needs nothing.
- **Recommendation:** JSON, no manifest change. Revisit if translators
  outside the owner ever edit the files.

### Q3 — Where the preference lives, and the resolution order (data-owner)

**Option A — `users.language` column + a `kauppa_lang` cookie (recommended)**

- Migration `0009_user_language.sql`:
  `ALTER TABLE users ADD COLUMN language TEXT;` nullable, no default, no
  `CHECK` (a `CHECK (language IN ('en','fi'))` would make a third language a
  table rebuild). Purely additive; existing rows read NULL = "not chosen".
  Core validates on write (`Language::parse`) and treats an unknown stored
  value as NULL on read.
- `UserRepo::language(user_id)` and `UserRepo::set_language(user_id,
  Option<Language>)`.
- Resolution per request: signed-in user's column → `kauppa_lang` cookie →
  `Accept-Language` (hand-parsed q-values, primary subtag `en`/`fi`) →
  `en`.
- The column is read per request for signed-in users (one primary-key
  lookup, well inside the 100 ms budget), so a change on one device applies
  on the next request on the others.
- On registration, the account's column is set from the cookie if one is
  present (an explicit choice made on the login/register page), never from
  `Accept-Language`.
- Pros: meets P3's "persists across sessions and devices"; signed-out
  visitors can switch; survives sign-out.
- Cons: a schema change; one extra query per signed-in request.

**Option B — Cookie only**

- Pros: no schema change, no per-request query.
- Cons: per device and per browser; lost when cookies are cleared. Meets
  P3 only in its weaker reading.

**Option C — Session field**

- Cons: purged at sign-out (`PurgeSession`) and gone when the browser closes
  (`BrowserSession`), so the login page right after sign-out reverts;
  per device. Changes what the session cookie carries (security).

**Option D — `Accept-Language` only**

- Cons: no switch on the login page; fails P3.

**Recommendation: A.** Alternative if the owner wants no schema change now:
B, with A as a follow-up (the resolution order already has the slot).

### Q4 — The language cookie (security)

Recommended attributes for `kauppa_lang`:

- value exactly one of the supported tags (`en`, `fi`); anything else is
  ignored on read and never echoed back;
- `Path=/`, `SameSite=Lax`, `HttpOnly` (nothing reads it client-side),
  `Secure` when `KAUPPA_COOKIE_SECURE=true`, `Max-Age` one year;
- unsigned: tampering can only pick another supported language;
- set by the switch handler, not by middleware: **no change to the session
  middleware, `kauppa_sid`, or cookie settings in `main.rs`/`config.rs`**;
- HTML responses add `Vary: Cookie, Accept-Language` (no cache sits in
  front today; this keeps a future one honest).

No session field (Q3 C).

### Q5 — The switch endpoint (security)

- `POST /language` with form field `lang`, registered with
  `#[post]` in `routes::protected`, so it is behind RR-0010's cross-site
  check and in `csrf/tests/coverage.rs` automatically. No GET with side
  effects (security.md: safe methods stay side-effect free), so no
  `?lang=fi` links.
- Works signed in and signed out. Signed in: writes the column and the
  cookie. Signed out: the cookie only.
- Answers `303 See Other` to the page it came from: a hidden `return` field
  holding the current request **path** (and query), accepted only if it
  starts with a single `/` (not `//`), else `/`. No `Referer` use, no open
  redirect.
- Not rate-limited (no account or password involved).

### Q6 — How core errors become user text (owner)

**Option A — Typed reasons in core, mapped to keys in web (recommended)**

- `CoreError::Invalid(String)` and `Conflict(String)` become
  `Invalid(Invalid)` / `Conflict(Conflict)` with enums such as
  `Invalid::NameEmpty(Entity)`, `Invalid::TooMany(Entity)`,
  `Invalid::MultiplierRange`, `Invalid::NoIngredients { recipe: String }`,
  `Conflict::Exists { entity, name }`.
- Core stays language-free. Their `Display` keeps today's English text, so
  logs, `error.rs`'s `responses_unchanged` and core tests keep their
  strings.
- `kauppa-web` maps each variant to a key and arguments in one exhaustive
  `match`: a new variant without a translation is a compile error, which
  is P1/P2 for Rust messages at no test cost.
- Web-only messages (`INVALID_LOGIN`, "choose a list to add to", …) become
  a web-side enum mapped the same way.
- `InvitationState` is mapped in web (`invitations.state.pending`), not via
  `as_str()`.
- Cons: touches every `CoreError::Invalid(...)` site (34) and the 9 handler
  match arms; mechanical.

**Option B — Core returns message keys**

- `Invalid { key: &'static str, args: … }`. Less churn, but core then knows
  the catalog's key names, and nothing checks a key exists except a test
  scan.

**Option C — Keep English strings, translate by matching them in web**

- Fragile (any wording edit breaks it); rejected.

**Recommendation: A.**

### Q7 — The switcher's place (owner)

- **Recommended:** a `<footer>` in `layout.hbs`, on every page including
  sign-in and registration: one POST form with two submit buttons, each
  named in its own language with its own `lang` attribute
  (`<button lang="en">English</button> <button lang="fi">Suomi</button>`),
  the current one marked `aria-pressed="true"`.
- `hx-boost="false"` on that form, so the answer is a full page load and
  `<html lang>` updates (a boosted swap would leave it stale).
- No JavaScript, works without htmx, 320 px friendly.
- Independent of MB-0040: the footer is outside the navigation that MB-0040
  moves into a side panel, so neither brief constrains the other.
- Alternatives: in the top bar (competes for the space MB-0040 is freeing;
  would have to move into the panel on phones), or on a settings page
  (none exists; signed-out visitors couldn't reach it).

### Q8 — Order relative to the other UI briefs (owner)

MB-0029 (form errors), MB-0034 (password length message), MB-0036 (one add
field), MB-0037, MB-0038 (shop row), MB-0039 (recipe category picker) and
MB-0040 (navigation panel) all edit the templates and messages above; none
has started.

- **Recommendation: MB-0035 last**, because:
  - each string is translated once, and the owner reviews Finnish once;
  - those briefs restructure exactly the markup the conversion touches
    (MB-0036 replaces the favorites form in `_list.hbs`, MB-0039 the
    ingredient category picker and `_match_hint`, MB-0040 the nav,
    MB-0038 the shop row), so converting first means rebasing every one;
  - MB-0029 decides what error pages look like; localizing them after is
    simpler than designing both at once (Q10).
- **Option:** land the infrastructure early (catalogs, helper, language
  resolution, switcher, `<html lang>`, P2/P3 tests, with only the layout
  converted) and the conversion with P1 and P4 last. Later briefs would
  then add keys to both catalogs as they go. That keeps the conversion PR
  smaller but splits the mission into two PRs.
- If MB-0035 goes first instead: each later brief must add both languages,
  and its CRP and PR need Finnish review.

### Q9 — Finnish wording and plurals (owner)

- **Recommended:** user data never inflected; case goes on a head noun or
  the name follows a colon or quotes (examples under "Finnish grammar"
  above). Plurals by `one`/`other`.
- The owner reviews the Finnish in the PR (brief autonomy).

### Q10 — Dates and error pages (owner)

**Dates.**

- **Recommended:** date formats live in the catalog as chrono format strings
  (`format.date`, `format.datetime`), numeric fields only, so no locale data
  or crate is needed:
  - English unchanged (P6): `%-d.%-m.%Y` and `%Y-%m-%d %H:%M UTC`;
  - Finnish: `%-d.%-m.%Y` and `%-d.%-m.%Y klo %-H.%M UTC`.
- Alternative: `chrono`'s `unstable-locales` feature (+1 package,
  `pure-rust-locales`; a feature change under dependency-bump, and the
  feature is labelled unstable). Its `fi_FI` short date is zero-padded
  `%d.%m.%Y`, and `en_US` would turn English into `11/09/2026`, an English
  change P6 would make the RR list.
- Sub-question: keep English dates as today (`9.11.2026`, unusual for
  English but P6-safe), or switch English to `9 Nov 2026` (month names as
  catalog keys, no crate)? **Recommendation: keep.**
- Time zone stays UTC (as today). Local Helsinki time needs a zone database
  (`chrono-tz`) or a per-user zone: a separate brief if wanted.

**Error pages.** Today `WebError::error_response` answers plain text ("not
found", "forbidden", …) with no access to the request, and the cross-site
check's 403 does the same. Localizing them needs the request's language
there.

- **Recommended:** follow MB-0029. If MB-0029 lands first (Q8), it defines
  a request-aware error page; MB-0035 then translates it. If MB-0035 must
  go first, keep those four bodies English and list them in the RR as P4's
  only exception, rather than adding a middleware around the RR-0010 scope
  (pinned by `csrf/tests/coverage.rs`).

## Test strategy

Test names are the brief's (`i18n::tests::…`, a new
`crates/kauppa-web/src/i18n.rs` with `i18n/tests.rs`, per RR-0009).

- **P1 `templates_have_no_literal_text`.** For every entry in `TEMPLATES`:
  remove `{{!-- --}}` comments and every `{{…}}` expression, then collect
  text nodes and the values of `placeholder`, `aria-label`, `title`, `alt`,
  `label` and `hx-confirm`; fail on any run with a letter, outside a short
  allowlist (`kauppa`, `name@example.com`, symbols `↑ ↓ ☐ ☑ ✕`). The
  scratch prototype of this scan finds today's 203 strings, so the test
  starts red. Literal `page_title="…"` hash arguments are checked too. Rust
  messages are covered by Q6's exhaustive `match` (compile-time) plus a
  test that renders every `Invalid`/`Conflict` variant in both languages.
- **P2 `catalogs_complete`.** All catalogs have the same key set; the same
  placeholder set per key; plural objects have `one` and `other` in all;
  no value contains `<` or `&`; every `{{t "…"}}` key in `TEMPLATES` and
  every key the Rust mapping produces exists; no catalog key is unused.
- **P3 `language_selection`.** Route tests: column beats cookie beats
  `Accept-Language` (`fi;q=0.9, en;q=0.8`, `fi-FI`, `*`, garbage) beats
  `en`; `<html lang>` matches; `POST /language` signed out sets only the
  cookie; signed in sets column and cookie; another session for the same
  user sees it; bad `lang` is 400; `return=//evil.example` redirects to
  `/`; a cross-site POST is 403.
- **P4 `every_page_renders_in_finnish`.** Fixtures for every page state
  (owned, shared view/edit list, removed items, shop view, recipes with
  ingredients and match hints, favorites with hidden products, categories,
  admin invitations in each state, rate-limited login, invalid invitation),
  then every GET page in full and every htmx fragment answer with
  `HX-Request`, under `fi`: 200 (or the expected status),
  `<html lang="fi">`, and no English catalog value (of three or more
  letters, not equal to its Finnish value) in the body. The route list is
  checked against `csrf/tests`' scan of registered handlers so a new page
  can't be missed.
- **P5 `dates_localized`.** A fixed `DateTime` formatted per language; the
  list page and admin page show it.
- **P6.** The full suite unchanged; English route tests keep their
  expected strings.

## Recommendation and question

| Q | Recommendation |
| --- | --- |
| Q1 Mechanism | B: in-house `{{t}}` helper over embedded catalogs; no new crate |
| Q2 Catalogs | `locales/<tag>.json`, embedded by `build.rs`, flat dotted keys, `{name}` placeholders, `one`/`other` plurals |
| Q3 Preference | `users.language` (nullable `TEXT`, migration `0009_user_language.sql`) + `kauppa_lang` cookie; column → cookie → `Accept-Language` → `en` |
| Q4 Cookie | `kauppa_lang`, whitelisted value, `HttpOnly`, `SameSite=Lax`, `Secure` per config, 1 year, unsigned; no session field, no session middleware change |
| Q5 Switch | `POST /language` behind the cross-site check; 303 to a validated same-site path |
| Q6 Core errors | Typed `Invalid`/`Conflict` reasons in core, exhaustive key mapping in web; `Display` keeps today's English |
| Q7 Switcher | Layout footer, two buttons in their own languages, `hx-boost="false"` |
| Q8 Order | MB-0035 after MB-0029, MB-0034, MB-0036–MB-0040 (optionally infrastructure early) |
| Q9 Finnish | No inflection of user data; head-noun / colon style; `one`/`other` plurals |
| Q10 Dates, errors | Format strings in the catalog, English unchanged, UTC; error pages localized after MB-0029 |

**Crisp question to answer:** Approve **option B** (no dependency) with JSON
catalogs, a **`users.language` column in migration
`0009_user_language.sql` plus a `kauppa_lang` cookie** set by
**`POST /language`**, **typed core error reasons** mapped in web, the
**footer switcher**, and **MB-0035 scheduled after the other UI briefs**?
Or name the deviating option per question.

## Blast radius and rollback

- **What can break:**
  - a page failing to render (missing key or argument): strict errors, caught
    by P2 and P4, not silent;
  - English output drifting (P6);
  - an argument rendered unescaped (the helper's escaping is unit-tested,
    and P4 renders user data with `<` in it);
  - the migration: additive nullable column, no data rewritten.
- **Who is affected:** every page; no stored user data changes, and no
  session, auth or authorization rule changes.
- **Rollback plan:** revert the PR(s). The `language` column can stay
  (unused, NULL-tolerant) or be dropped by a later migration; the
  `kauppa_lang` cookie is then ignored.

## Minimal context

- MB-0035 P1–P6; forbidden: translating user data, inline scripts or JS
  beyond htmx, editing committed migrations, lowering lint strictness.
- RR-0010 (cross-site check, pinned wiring), RR-0006 (`page_` layout
  params), RR-0009 (test paths), RR-0012 (`routes/auth/`, `routes/admin/`
  security paths), MB-0024 (embedded templates; `build.rs`), RR-0015
  (`sha2` as precedent for an already-locked crate still being a manifest
  change).
- Sibling briefs: MB-0029, MB-0034, MB-0036–MB-0040 (Q8), MB-0040 (Q7).

## Evidence bundle (Hardened)

All work in the agent's scratchpad; nothing committed but this CRP and the
Continuity Pack.

- **Inventory:** a Python scan of `templates/**/*.hbs` (rules above) →
  203 occurrences, 152 distinct, per-area counts as tabled; `grep` of
  non-test `crates/**/*.rs` for `CoreError::(Invalid|Conflict)(`,
  `const …: &str`, `format!` and `.format(` → the Rust table.
- **crates.io API** (`/api/v1/crates/<name>`, `/owners`), 2026-11-09:
  versions, owners, downloads and licenses as tabled; none yanked.
- **Trial trees** (scratch copies of `50322c0`, `cargo add` then
  `cargo tree -p kauppa-web -e normal,build --prefix none | sort -u`):
  baseline 256 packages; `fluent-bundle`+`unic-langid` +10;
  `fluent-templates` (handlebars, macros, walkdir) +15; `rust-i18n` +31;
  `chrono/unstable-locales` +1 (`pure-rust-locales`).
- **`cargo deny check advisories licenses bans`** in each copy: "advisories
  ok, bans ok, licenses ok"; `rust-i18n` adds duplicate warnings for
  `bitflags`, `serde_spanned`, `toml`, `toml_datetime`, `winnow`.
- **`fluent-templates` 0.15.1 trial** (strict handlebars 6.4.4,
  `static_loader!`, en-US and fi `.ftl`):
  - `en-US: "Share \u{2068}<b>x</b>\u{2069}|Updated \u{2068}1.2.2026\u{2069}|\u{2068}5\u{2069} items|Unknown localization key: \"nope\""`;
  - `fi: "Jaa \u{2068}<b>x</b>\u{2069}|Päivitetty …|\u{2068}5\u{2069} tuotetta|Unknown localization key: \"nope\""`;
  - a partial with a sub-context still found the root `lang`;
  - without `walkdir`/`ignore`, `fluent-template-macros` fails to compile
    ("one of the features `ignore` or `walkdir` must be enabled");
  - source: `src/loader/handlebars.rs` (no escaping; `.expect("Language not
    set in context")`), `src/loader/static_loader.rs:83` (unknown-key
    string).
- **`pure-rust-locales` 0.8.2** `fi_FI::LC_TIME::D_FMT = "%d.%m.%Y"`,
  `D_T_FMT = "%a %e. %Bta %Y %H.%M.%S"`.
- **actix-identity 0.8.0** `identity.rs:189-193` and `config.rs:20`
  (`PurgeSession` default); **actix-session 0.10.1** default
  `BrowserSession`.
- **CLDR plural rules** for `en` and `fi`: `one` (i = 1 and v = 0),
  `other`.
