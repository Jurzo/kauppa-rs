# Mission Brief — Content-Security-Policy

```yaml
---
id: MB-0031
owner: "@juuso-kalliomaki"
goal: "Every HTML response carries a Content-Security-Policy that lets pages load scripts, styles and other resources only from the app's own origin, so injected markup can't run script or exfiltrate data, while every page keeps working."
non_goals:
  - "Report-only rollouts with a reporting endpoint (the RR may choose report-only first, but no collector service)."
  - "Other security headers beyond what the RR bundles with the CSP (HSTS, `X-Content-Type-Options`, `Referrer-Policy` may be named there)."
  - "Upgrading htmx (MB-0030)."
acceptance:
  - id: P1
    property: "Every response with an HTML body (full pages, fragments, error pages, 404s from the catch-all) carries the approved `Content-Security-Policy`, with at least `default-src 'self'`, `script-src 'self'` (no `'unsafe-inline'`, no `'unsafe-eval'`), `object-src 'none'`, `base-uri 'none'`, `form-action 'self'` and `frame-ancestors 'none'`."
    check: cargo test -p kauppa-web csp::tests::every_html_response_has_csp
  - id: P2
    property: "htmx runs under the policy: `htmx-config` sets `allowEval` false and `includeIndicatorStyles` false, the indicator rules live in `app.css`, and no template has inline `<script>`, `<style>`, `style=` or `on*=` attributes."
    check: cargo test -p kauppa-web templates::tests::csp_compatible_templates
  - id: P3
    property: "The header is added in one place that every route passes through (including `/assets` and the 404 catch-all) and can't be bypassed by a new route; the existing route tests pass unchanged."
    check: cargo test -p kauppa-web --all-targets
  - id: P4
    property: "In a browser, every page (lists, store view, recipes, favorites, categories, auth, admin) loads with no CSP violation in the console, boosted navigation works, and the ingredient autocomplete works."
    check: "manual: browser check on the release binary, recorded in MB-0031's Continuity Pack"
context:
  - crates/kauppa-web/src/main.rs
  - crates/kauppa-web/src/routes/mod.rs
  - templates/layout.hbs
  - assets/app.css
  - deploy/Caddyfile
  - .agentic/resolutions/RR-0015-vendor-htmx.md
  - .agentic/consults/CRP-0015-vendor-htmx.md
  - .agentic/mentorship/security.md
autonomy:
  may_decide:
    - "the module and test names for the header (record a rename in the brief)"
    - "moving htmx's indicator CSS into `app.css`"
  escalate:
    - { when: "the policy itself, report-only vs enforcing, and which other headers ship with it", to: security }
    - { when: "where the header is set: app middleware (`main.rs`, a security trigger path) or Caddy (`deploy/Caddyfile`, which would leave local and test runs without it)", to: security }
    - { when: "adding the `htmx-config` meta tag (RR-0015 Does not cover)", to: platform-owner }
    - { when: "any page needs `'unsafe-inline'`, `'unsafe-eval'`, a nonce or a hash", to: security }
  forbidden:
    - "inline scripts or styles, or `'unsafe-inline'` / `'unsafe-eval'` in `script-src`"
    - "loading anything from another origin"
    - "change session, cookie or cross-site check settings"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/security.md@v1
  - .agentic/mentorship/templating.md@v1
  - .agentic/mentorship/actix-web.md@v1
resolutions: []
---
```

## Context

RR-0015 constraint 5 opens this follow-up (security) after MB-0027 made
every script same-origin. Its starting point is CRP-0015 Q5's sketch:
`script-src 'self'`, and `{"allowEval":false,"includeIndicatorStyles":false}`
in an `htmx-config` meta tag, with htmx's indicator rules moved into
`app.css`. By default htmx 2 injects a `<style>` element for indicators
(blocked by a strict `style-src`) and evaluates `hx-on`/`js:` expressions
(blocked without `'unsafe-eval'`). The templates have no inline scripts,
styles or `style=` attributes today (`templating.md` forbids inline
scripts).

Where the header goes is a security decision: middleware in `main.rs`
covers tests and local runs, Caddy covers only the deployed stack.

Order: after MB-0030 (htmx upgrade) and MB-0029 (form errors under htmx),
which both touch `<head>` and htmx's configuration.

## Conceptual plan

1. **Clarify (S1).** CRP to security (policy, placement, other headers),
   with the `htmx-config` question to platform-owner.
2. Header in the approved place; `htmx-config`; indicator CSS.
3. Tests for coverage and template compatibility; browser check (P4).

## Validation plan

- P1: a test sending requests to a representative route per area, the
  assets service and the catch-all, asserting the header.
- P2: a template scan for inline script/style/handlers and the config.
- P3: the full suite. P4: browser console on every page.
