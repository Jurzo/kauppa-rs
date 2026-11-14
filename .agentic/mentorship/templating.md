# Mentorship Pack — templating (handlebars + htmx)

Version: v1 · Scope: `templates/**`, `assets/**`, and any route rendering HTML.

## Handlebars

- One shared `layout.hbs` using the partial-block pattern. Every page is
  `{{#> layout page_title="…" page_signed_in=…}}…{{/layout}}`.
- Pass data through typed structs serialized to JSON via `serde_json::json!`
  only for scaffolds — prefer `#[derive(Serialize)]` structs for anything
  non-trivial. Strict mode is on, so missing fields are errors at render time.
- No inline JavaScript in templates. If an interaction needs scripting beyond
  htmx, write a small vanilla-JS module in `assets/js/` and link it.

## Translations

- All user-facing text comes from the catalogs `locales/<tag>.json`
  through the `{{t "key" name=value}}` helper (MB-0035, RR-0023). No
  literal prose in templates: text nodes, `placeholder`, `aria-label`,
  `title`, `alt`, `label`, `hx-confirm` and `page_title` all use `{{t}}`
  or `(t "…")`. `i18n::tests::templates_have_no_literal_text` enforces
  it.
- Keys are flat and dotted by area (`lists.empty`), placeholders are
  `{name}`, and plurals are `{"one": "…", "other": "…"}`. Every catalog
  has the same keys and placeholders. Values contain no `<` or `&`: a
  sentence around a link is split into before / link / after keys.
- One key per whole sentence. Never build a message by concatenating
  translated pieces. Never inflect user data: put the case on a head
  noun, or put the name after a colon or in quotes.
- Messages built in Rust use `I18n::text`. Core errors are typed reasons
  mapped to keys in `kauppa-web`, never English text shown as is.
- Exception (RR-0023 sub-answer 10, RR-0036): the plain error bodies from
  `WebError::error_response` and the cross-site 403 are English in every
  language. They are built without the request, so they can't know its
  language, and the forms catch rejected input and show it translated in
  the page. A translated error page is its own brief.
- A change that adds or edits user-facing text updates every catalog in
  the same PR.

## htmx

- Server returns a **fragment** when the request has `HX-Request: true`,
  otherwise a full page. Keep fragment templates as separate `.hbs` files
  under `templates/<section>/_<fragment>.hbs` (prefix `_`).
- Prefer `hx-boost="true"` on `<body>` for progressive enhancement.
- Use `hx-target` + `hx-swap` explicitly on each interactive element — don't
  rely on implicit defaults.
- Validate server-side; htmx attributes are untrusted display hints.

## Accessibility and mobile

- Every form field has a visible `<label>`.
- Interactive elements are focusable and have accessible names.
- Pages must be usable at 320 px wide, one-handed, in portrait. Test with a
  mobile emulator (DevTools "Device toolbar").

## Red flags

- Markup duplication across pages → extract a partial.
- A page template emitting its own `<head>` or `<html>` → use the layout.
- Server-side templating mixing HTML and JSON responses in one handler.
- A layout parameter named like a view field. Partial hash params become
  block params and shadow same-named body fields on full pages (RR-0006).
- A literal string in a template, or a `format!` that builds user-facing
  text in a handler (RR-0023).
