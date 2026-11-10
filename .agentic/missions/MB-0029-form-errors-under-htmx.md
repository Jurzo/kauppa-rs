# Mission Brief — Show form errors on boosted pages

```yaml
---
id: MB-0029
owner: "@juuso-kalliomaki"
goal: "When a boosted form is answered with an error page (4xx), the user sees that page and its message, with or without htmx, instead of nothing happening."
non_goals:
  - "Changing which status codes the handlers return (tests pin them, and the rate limiter counts 4xx answers)."
  - "Client-side validation beyond the HTML attributes already in the templates."
  - "Upgrading htmx (MB-0030) or adding a Content-Security-Policy (MB-0031)."
acceptance:
  - id: P1
    property: "Every boosted full-page form whose handler answers a 4xx with a rendered page shows that page under htmx: at least login (401, 429) and register (400, 409, 429). The mechanism (htmx configuration, attributes, or an extension) is the one the RR approves (RR-0018, RR-0025), and a test pins it so it can't be dropped silently."
    check: cargo test -p kauppa-web templates::tests::boosted_error_pages_swap
  - id: P2
    property: "Fragment requests that target part of a page (`hx-post` with `hx-target`, e.g. list items) don't swap a full error page into the fragment target: they keep today's behaviour or get an answer the RR defines."
    check: cargo test -p kauppa-web templates::tests::fragment_errors_not_swapped
  - id: P3
    property: "Without htmx (plain POST) every error page renders exactly as today; the existing route tests pass unchanged."
    check: cargo test -p kauppa-web --all-targets
  - id: P4
    property: "In a browser, a wrong password, a bad invitation and a duplicate email each show their message without a reload, and the URL stays the form's."
    check: "manual: browser check on the release binary, recorded in MB-0029's Continuity Pack"
  - id: P5
    property: "Pages that render a secret in their HTML (the register page's hidden `invite` token, the admin invitations page's one-time link) carry `hx-history=\"false\"`, so htmx never snapshots them into the history cache (RR-0018)."
    check: cargo test -p kauppa-web templates::tests::secret_pages_skip_history
context:
  - templates/layout.hbs
  - templates/auth/
  - templates/admin/invitations.hbs
  - crates/kauppa-web/src/routes/auth/mod.rs
  - crates/kauppa-web/src/templates/tests.rs
  - assets/vendor/
  - .agentic/resolutions/RR-0015-vendor-htmx.md
autonomy:
  may_decide:
    - "the test names inside `templates::tests`, if the RR's mechanism needs others (record the rename in the brief)"
    - "which other forms are in scope, from an inventory of handlers that answer 4xx with a full page"
  escalate:
    - { when: "choosing the mechanism: a global `htmx-config` `responseHandling` rule, per-form attributes, or the response-targets extension (a new vendored file)", to: platform-owner }
    - { when: "any change to an auth handler's status code, headers or body (`routes/auth/` is a security trigger path)", to: security }
    - { when: "the mechanism needs inline script or eval, which MB-0031's CSP would forbid", to: security }
  forbidden:
    - "commit executable files outside `assets/vendor/`, or modify a vendored file"
    - "inline `<script>` in templates"
    - "change status codes to 200 to make htmx swap them"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
  - .agentic/mentorship/security.md@v1
resolutions: [RR-0018, RR-0025]
---
```

## Context

Found during MB-0023 (2026-11-08). `layout.hbs` puts `hx-boost="true"` on
`<body>`, so forms post through htmx. htmx 2's default `responseHandling`
swaps 2xx and ignores 4xx/5xx, so the login page's 401 ("Invalid email or
password."), and the register page's 400, 409 and 429 answers, are dropped:
the user clicks and nothing changes. Without htmx (or after a reload) the
pages render correctly. MB-0026's 429 pages (login and register) have the
same problem.

The candidate mechanisms each have a cost: a global `responseHandling`
rule in an `htmx-config` `<meta>` tag also changes fragment requests
(P2); per-form `hx-*` attributes need one per form; the response-targets
extension is a second vendored file (RR-0015's rules apply). MB-0030
(htmx upgrade) and MB-0031 (CSP) touch the same `<head>`; doing MB-0030
first avoids writing the config twice.

## Conceptual plan

1. Inventory: which handlers answer 4xx with a full page, and which with a
   fragment.
2. **Clarify (S1).** CRP to platform-owner (mechanism) with security on
   the auth pages and CSP compatibility. Answered by RR-0018.
3. Implement the approved mechanism with tests that pin it.
4. Browser check (P4).

## Validation plan

- P1, P2: tests over the rendered layout or templates asserting the
  approved mechanism is present and scoped.
- P3: the full web suite.
- P4: a browser session on the release binary.
