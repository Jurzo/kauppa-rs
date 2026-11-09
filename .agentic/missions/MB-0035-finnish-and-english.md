# Mission Brief — Finnish alongside English

```yaml
---
id: MB-0035
owner: "@juuso-kalliomaki"
goal: "Every page, message and email-free user-facing string is available in English and Finnish, each user sees the app in their chosen language, and adding a third language later means adding one translation file, not touching templates or handlers."
non_goals:
  - "Languages other than English and Finnish (the mechanism must allow them later)."
  - "Translating user data: list names, item text, product, category and recipe names stay as typed."
  - "Translated URLs or per-language routes."
  - "Right-to-left layouts, plural rules beyond what English and Finnish need."
  - "Locale-specific parsing of quantities or multipliers (quantities are free text; the multiplier keeps today's rules)."
acceptance:
  - id: P1
    property: "Every user-facing string in `templates/` and every message a handler or core error shows to the user comes from the translation catalog: no template contains hard-coded English prose, and a test fails on a new literal string."
    check: cargo test -p kauppa-web i18n::tests::templates_have_no_literal_text
  - id: P2
    property: "The English and Finnish catalogs have exactly the same keys, every placeholder used in one is used in the other, and every key a template or handler uses exists."
    check: cargo test -p kauppa-web i18n::tests::catalogs_complete
  - id: P3
    property: "The language is chosen as the RR decides (e.g. a saved user preference, then a cookie, then `Accept-Language`, then English); `<html lang>` matches it; a signed-out visitor can switch language on the login page, and a signed-in user's choice persists across sessions and devices if the RR stores it per user."
    check: cargo test -p kauppa-web i18n::tests::language_selection
  - id: P4
    property: "Each page renders in Finnish without missing keys or English fallbacks, for full pages and htmx fragments alike (lists, store view, recipes, favorites, categories, auth, admin, error pages)."
    check: cargo test -p kauppa-web i18n::tests::every_page_renders_in_finnish
  - id: P5
    property: "Dates and times shown to the user (e.g. the list's \"Updated\" line) use the chosen language's format."
    check: cargo test -p kauppa-web i18n::tests::dates_localized
  - id: P6
    property: "English output is unchanged for existing pages: the existing route tests pass without edits to their expected strings, or the RR lists every string that changes."
    check: cargo test --workspace --all-targets
context:
  - templates/
  - crates/kauppa-web/src/templates.rs
  - crates/kauppa-web/src/error.rs
  - crates/kauppa-web/src/routes/
  - crates/kauppa-core/src/error.rs
  - crates/kauppa-core/src/domain/user.rs
  - .agentic/mentorship/templating.md
  - .agentic/mentorship/security.md
autonomy:
  may_decide:
    - "translation key names and the catalog files' layout"
    - "the Finnish wording, which the owner reviews in the PR"
    - "the order the templates are converted in (one area per commit)"
  escalate:
    - { when: "the translation mechanism: a crate (e.g. Fluent) or a small in-house handlebars helper over embedded catalog files; any new dependency goes through the dependency-bump runbook", to: platform-owner }
    - { when: "where the language preference lives: a `users` column (schema change, new migration), a cookie, the session, or only `Accept-Language`", to: data-owner }
    - { when: "a language cookie or session field, or any change to session or cookie settings", to: security }
    - { when: "how core errors become user text: core keeps returning `CoreError` variants and the web layer maps them to keys, or core gains message keys", to: owner }
    - { when: "the language switcher's place in the layout (it interacts with MB-0040's navigation)", to: owner }
  forbidden:
    - "translate or alter user data"
    - "inline scripts, or JavaScript beyond htmx"
    - "edit committed migrations"
    - lower lint strictness
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
  - .agentic/mentorship/actix-web.md@v1
  - .agentic/mentorship/persistence.md@v1
resolutions: []
---
```

## Context

Requested by the owner (2026-11-09): multi-language support, starting with
Finnish alongside English. Users are Finnish speakers (the placeholders
already say "Kynttilät", "Maito", "Pannukakku"), but every label, button,
message and error is English and hard-coded in `templates/*.hbs`, in
handlers (e.g. `INVALID_LOGIN` in `routes/auth/`), and in `CoreError`
messages that reach the page (e.g. "choose a favorite to add").

Templates are embedded at build time (MB-0024) and rendered in strict mode,
so a missing helper argument already fails a test. The layout's
`<html lang="en">` is fixed.

Other open briefs change strings too: MB-0029 (form errors), MB-0034
(password length message), MB-0036–MB-0040 (list, shop, recipe and
navigation UI). Landing those first and this one after means each string is
translated once; if this one lands first, later briefs must add both
languages.

## Conceptual plan

1. **Clarify (S1).** One CRP: mechanism and dependency (platform), where
   the preference lives (data-owner, security), core error mapping and
   switcher placement (owner).
2. Infrastructure: catalog loading, the helper, language resolution per
   request, `<html lang>`, tests P2 and P3.
3. Convert templates area by area with P1's scan catching what's left;
   handler and core messages; dates.
4. Finnish review by the owner in the PR.

Slow-mode triggers: a string that can't be translated without changing
handler logic (e.g. built by concatenation); a key needing grammatical case
inflection of user data (Finnish cases), which the catalog can't do.

## Validation plan

- P1, P2: unit tests scanning templates and comparing catalogs.
- P3–P5: route tests with and without a preference, cookie and header.
- P6: the full suite.
- Smoke: every page in both languages on the release binary, screenshots
  or text dumps in the Merge-Readiness Pack.
