# Mentorship Pack — templating (handlebars + htmx)

Version: v1 · Scope: `templates/**`, `assets/**`, and any route rendering HTML.

## Handlebars

- One shared `layout.hbs` using the partial-block pattern. Every page is
  `{{#> layout title="…" signed_in=…}}…{{/layout}}`.
- Pass data through typed structs serialized to JSON via `serde_json::json!`
  only for scaffolds — prefer `#[derive(Serialize)]` structs for anything
  non-trivial. Strict mode is on, so missing fields are errors at render time.
- No inline JavaScript in templates. If an interaction needs scripting beyond
  htmx, write a small vanilla-JS module in `assets/js/` and link it.

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
