# Consultation Request Pack — Content-Security-Policy and companion headers (MB-0031)

```yaml
---
id: CRP-0019
mission: MB-0031
owner: "@juuso-kalliomaki"
target_role: security
risk_tier: medium
urgency: medium
state: answered
runbook_trigger: "Feature runbook escalation trigger `crates/kauppa-web/src/main.rs` (session middleware; the header wrap goes there) → security. MB-0031 autonomy.escalate: the policy, report-only vs enforcing and companion headers → security; placement (app middleware in `main.rs` vs Caddy `deploy/Caddyfile`) → security; adding the `htmx-config` meta (RR-0015 Does not cover) → platform-owner; any `'unsafe-inline'`, `'unsafe-eval'`, nonce or hash → security."
---
```

## Decision statement

Approve the Content-Security-Policy the app sends, where it is set, whether
it enforces from the start, which other security headers ship with it, and
the `htmx-config` that lets htmx run under it.

Routing: security for Q1–Q4; platform-owner for Q5 (`htmx-config`, indicator
CSS). Both are @juuso-kalliomaki, so one RR can answer everything.

## Current state

Verified on `50322c0` (2026-11-09), htmx 2.0.3 vendored.

- **No security headers app-wide.** The only ones set anywhere are on the
  register responses: `Referrer-Policy: no-referrer` and `Cache-Control:
  no-store` (`routes/auth/mod.rs:341-350`, RR-0012 sub-answer 5). Caddy
  adds none (`deploy/Caddyfile`: `reverse_proxy` only) and it doesn't set
  HSTS by default.
- **Wiring.** `main.rs:87-104`: `App` with `TracingLogger`,
  `IdentityMiddleware`, `SessionMiddleware`, then
  `.service(routes::assets::Assets)` (outside the cross-site scope,
  RR-0010/RR-0011) and `.configure(routes::configure)` (one catch-all
  `web::scope("")` wrapped in `csrf::guard`, `routes/mod.rs:21-27`).
  - Unknown paths fall into that scope's default 404 (empty body); there
    is no `default_service` (the coverage guard forbids builder calls).
  - `csrf::tests::coverage` rule 2 pins `main.rs`'s `.service`/`.configure`
    lines (a `.wrap` line doesn't trip it); rule 3 pins `configure`'s body,
    so the header can't be added there, and that scope wouldn't cover
    `/assets` anyway.
  - Route tests build the app with `test_support::wired_app!`
    (`routes/test_support.rs:230-245`), which mirrors `main.rs`'s
    middleware by hand.
- **Templates are already CSP-clean** (grep over `templates/`, 27 files):
  - one `<script>`: the vendored htmx, same-origin, with `integrity`
    (`layout.hbs:10`); no inline script;
  - no `<style>`, no `style=` attribute, no `on*=` handler, no
    `javascript:` URL, no `hx-on*`, no `js:` value, no `hx-trigger` filter
    (`[...]`), no `<img>`, `<iframe>`, `<object>`, `<embed>` or `<base>`;
  - stylesheet `/assets/app.css` (no `url(`, `@import` or `@font-face`) and
    manifest `/assets/manifest.webmanifest` (`"icons": []`);
  - `hx-confirm` (6) uses `window.confirm`, which CSP doesn't restrict.
- **What htmx does by default that a strict CSP blocks** (2.0.3 and 2.0.11
  `dist/htmx.js`, same defaults):
  - `includeIndicatorStyles: true` → at load it inserts a `<style>` element
    into `<head>` (2.0.3 l.4958; 2.0.11 l.5135). `style-src 'self'` blocks
    it and logs a violation on **every page**, which fails P4 even though
    the app uses no indicator.
  - `allowEval: true` → `eval`/`Function` via `maybeEval` (2.0.11 l.4022)
    only for features the app doesn't use (`hx-on`, trigger filters,
    `js:`/`hx-vars`). Without `'unsafe-eval'` they would fail; setting it
    `false` makes htmx refuse them explicitly
    (`htmx:evalDisallowedError`).
  - `allowScriptTags: true` → htmx re-creates `<script>` elements found in
    swapped content so they run. The app swaps none.
  - `selfRequestsOnly: true` (already same-origin only).
  - Config comes from one `<meta name="htmx-config">`, parsed with
    `JSON.parse`: no eval, unaffected by CSP.

## Questions, options and recommendations

### Q1 — The policy (security)

Proposed header (one line):

```
Content-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self'; object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'
```

- Covers P1's minimum and adds `style-src 'self'` (no inline style) and
  `img-src 'self'` (CRP-0015's sketch had `data:`; nothing uses it, so it's
  left out until something does).
- `connect-src` (htmx XHR) and `manifest-src` fall back to
  `default-src 'self'`.
- `form-action 'self'` also applies to the 303 after login/register
  (same-origin `/lists`), so it holds.
- `frame-ancestors` works only as a header (not in a `<meta>` CSP), one
  more reason for Q2's placement.
- Not proposed: `require-trusted-types-for 'script'` (htmx swaps via
  `innerHTML`/`insertAdjacentHTML`/`parseHTMLUnsafe` and would break);
  `upgrade-insecure-requests` (every URL is relative); nonces or hashes
  (nothing inline needs one).
- What it doesn't stop: injected `hx-*` attributes still drive same-origin
  htmx requests. Handlebars escaping is the defence there, as today.

**Recommendation:** the header as above, identical for every response.

### Q2 — Placement (security)

**Option A — App middleware, outermost `.wrap` in `main.rs` (recommended)**

- A `crate::csp` module exposes one builder, e.g. `csp::headers(&cfg)`
  returning actix-web's own `middleware::DefaultHeaders` (no new
  dependency); `main.rs` adds `.wrap(csp::headers(…))` as the **last**
  `.wrap`, so it is outermost and also covers responses produced by the
  session/identity middleware, the cross-site 403, `/assets`, extractor
  400s, `WebError` bodies and the catch-all 404.
- `DefaultHeaders` inserts a header only when the response doesn't already
  have it (actix-web 4.18.2 `middleware/default_headers.rs:179`), so the
  register page's stricter `Referrer-Policy: no-referrer` survives.
- `wired_app!` gets the same `.wrap(csp::headers(…))`, so every route test
  sees the header; a guard test (P3) asserts both `main.rs` and
  `wired_app!` carry it, the way `csrf::tests::coverage` pins the wiring.
- Pros: one place every route passes through; local runs, tests and every
  self-hoster get it (AGENTS.md goal 3); testable (P1).
- Cons: edits `main.rs` (security trigger path, hence this CRP);
  `wired_app!` mirrors it by hand (the guard test catches drift).
- Risk: low.

**Option B — Caddy only (`header` directive in `deploy/Caddyfile`)**

- Pros: no app change.
- Cons: no header in local runs or tests (P1 untestable); self-hosters
  without Caddy miss it; RR-0015 already rejected this (Q5 C).

**Option C — Both**

- Cons: a browser enforces every CSP header it receives (the intersection),
  so two copies that drift break pages in production only. Not
  recommended.

**Recommendation: A.**

### Q3 — Report-only first, or enforce (security)

- **(i) Enforce from the first release (recommended).** Every template is
  scanned by P2's test, every script and style is same-origin, and htmx's
  two inline behaviours are switched off in the same PR. P4's browser check
  on the release binary looks for violations before merge. Rollback is a
  revert.
- (ii) `Content-Security-Policy-Report-Only` for one release, then a second
  PR to enforce. Without a reporting endpoint (brief non-goal) violations
  appear only in each user's console, so it buys little beyond P4.
- (iii) An env switch (`KAUPPA_CSP_REPORT_ONLY`). More config and a test
  matrix for a one-off.

### Q4 — Companion headers (security)

Proposed to ship in the same middleware:

| Header | Value | Note |
| --- | --- | --- |
| `X-Content-Type-Options` | `nosniff` | All responses; assets already carry exact types (`SERVED_AS`). Recommended. |
| `Referrer-Policy` | `same-origin` | Nothing links off-site; the register page keeps its own `no-referrer` (not overwritten). Recommended. |
| `Strict-Transport-Security` | `max-age=31536000` | **Only when `KAUPPA_COOKIE_SECURE=true`** (the deployment's HTTPS signal, `config.rs:13`; set in `deploy/compose.yaml:30`). No `includeSubDomains` or `preload`: a self-hoster's other subdomains aren't ours to pin. Recommended (sub-question: here, or in Caddy where TLS terminates?). |
| `X-Frame-Options` | `DENY` | Legacy duplicate of `frame-ancestors 'none'`; every browser the app targets supports the CSP directive. Not recommended (optional). |
| `Cross-Origin-Opener-Policy`, `Permissions-Policy` | — | Out of scope (brief non-goal unless bundled); not recommended now. |

### Q5 — `htmx-config` and indicator CSS (platform-owner)

In `layout.hbs`'s `<head>`, before the htmx script:

```html
<meta name="htmx-config" content='{"allowEval":false,"includeIndicatorStyles":false,"allowScriptTags":false}' />
```

- `allowEval:false`, `includeIndicatorStyles:false`: P2 as written.
- `allowScriptTags:false` (not in P2; sub-question): the app swaps no
  scripts, and it stops htmx from executing a `<script src="/assets/…">`
  that injected markup might smuggle into a swap. Recommended, as a brief
  delta to P2's wording.
- Indicator rules into `assets/app.css`, copied from the htmx version
  actually served. 2.0.11 (MB-0030's target) is:
  `.htmx-indicator{opacity:0;visibility:hidden}`
  `.htmx-request .htmx-indicator,.htmx-request.htmx-indicator{opacity:1;visibility:visible;transition:opacity 200ms ease-in}`
  (2.0.3 lacks `visibility`). No template uses an indicator yet, so this is
  for future use and costs nothing.
- **One tag only.** htmx reads the first `meta[name="htmx-config"]`. If
  MB-0029 chooses CRP-0018 option A (global `responseHandling`) or Q5(b)
  (`historyCacheSize:0`), both missions' keys go in this one tag, whichever
  lands second. P2's test asserts exactly one such meta, with these keys.

### Q6 — Ordering and sibling missions (platform-owner)

- **MB-0030 first** (CRP-0017): the indicator CSS and the script path in
  P2's test follow the served version. The defaults this policy depends on
  are the same in 2.0.3 and 2.0.11.
- **MB-0029** (CRP-0018): its recommended option B adds a second
  same-origin vendored script with `integrity`; `script-src 'self'` covers
  it, and no `'unsafe-*'` is needed under any of its options (A is a JSON
  meta; C is an attribute).
- **MB-0040** (navigation panel): native `<dialog>`/`popover` opened with
  `commandfor`/`popovertarget`, `<details>`, or a CSS-only checkbox are all
  declarative and need nothing from the policy. It must not use `style=`
  or `on*=` (its own P2 already forbids handlers); an SVG icon is fine
  inline as markup, but as a `data:` URL it would need `img-src data:`.
- The CSP PR can land in any order relative to MB-0029 and MB-0040 once
  MB-0030 is in; each later PR keeps P2's template scan green.

## Recommendation and question

| Q | Recommendation |
| --- | --- |
| Q1 Policy | `default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self'; object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'` |
| Q2 Placement | A: `DefaultHeaders` from a `csp` module, outermost `.wrap` in `main.rs`, mirrored in `wired_app!`, pinned by a guard test |
| Q3 Mode | Enforce from the first release |
| Q4 Others | `nosniff`, `Referrer-Policy: same-origin`, HSTS `max-age=31536000` when `cookie_secure`; no XFO/COOP/Permissions-Policy |
| Q5 htmx | `{"allowEval":false,"includeIndicatorStyles":false,"allowScriptTags":false}`, 2.0.11 indicator rules in `app.css` |
| Q6 Order | After MB-0030; independent of MB-0029/MB-0040 |

Crisp questions to answer:

1. **Q1:** Approve the policy as written (in particular `style-src 'self'`
   and `img-src 'self'` without `data:`)?
2. **Q2:** Set it in the app (A: outermost `.wrap` in `main.rs`, mirrored in
   `wired_app!`), not in Caddy?
3. **Q3:** Enforce from the first release, with no report-only phase?
4. **Q4:** Ship `X-Content-Type-Options: nosniff` and `Referrer-Policy:
   same-origin` with it? HSTS `max-age=31536000` from the app when
   `KAUPPA_COOKIE_SECURE=true`, from Caddy, or not at all? Skip
   `X-Frame-Options`?
5. **Q5 (platform):** Add the `htmx-config` meta with `allowEval:false` and
   `includeIndicatorStyles:false`, plus `allowScriptTags:false` as a P2
   brief delta? Move the 2.0.11 indicator rules into `app.css`?
6. **Q6:** Implement after MB-0030 merges?

## Blast radius and rollback

- **What can break:** a page that needs something the policy forbids stops
  working silently in the browser (a blocked script or style shows only in
  the console); the route suite can't see it, hence P4. HSTS, once a
  browser has seen it, can't be revoked for a year: a deployment that later
  drops HTTPS on that host would be unreachable for those browsers.
- **Who is affected:** every page view. No data, schema, session or cookie
  setting changes (brief forbidden).
- **Rollback plan:** revert the PR: the headers, meta and CSS go together.
  HSTS persists in browsers that saw it (`max-age=0` from the app would
  clear it), which is why it's tied to `cookie_secure` and has no
  `includeSubDomains`.

## Minimal context

- MB-0031 P1–P4; forbidden: inline scripts/styles or `'unsafe-*'`, other
  origins, session/cookie/cross-site changes.
- RR-0015 Q5 (CSP deferred here; Caddy-only rejected), constraint 4 (no
  external script origins). RR-0010/RR-0011 (`/assets` registered beside
  the cross-site scope). RR-0012 sub-answer 5 (register headers).
- Siblings: CRP-0017 (MB-0030), CRP-0018 (MB-0029), MB-0040.

## Evidence bundle (Hardened)

- Grep over `templates/` (2026-11-09):
  `grep -rnE "<script|<style|style=|\son[a-z]+=|javascript:|hx-on|js:" templates`
  → only `layout.hbs:10` (the vendored htmx); `grep -rn "<img|<link|<iframe|<object|<embed|<base"`
  → only `layout.hbs:8-9` (manifest, stylesheet); htmx attribute counts:
  `hx-boost` 1, `hx-post` 32, `hx-target` 33, `hx-swap` 33, `hx-confirm` 6,
  `hx-get` 1, `hx-trigger` 1 (`input changed delay:300ms`, no filter).
- `assets/app.css`: no `url(`, `@import`, `@font-face`;
  `assets/manifest.webmanifest`: `"icons": []`.
- htmx `dist/htmx.js` (npm tarballs 2.0.3, 2.0.11): defaults l.118
  (`includeIndicatorStyles`), l.154 (`allowEval`), l.160
  (`allowScriptTags`), l.172 (`inlineStyleNonce`), l.241
  (`selfRequestsOnly`) in both; `insertIndicatorStyles` 2.0.3 l.4958 /
  2.0.11 l.5135; `getMetaConfig` (`querySelector`, `JSON.parse`) 2.0.11
  l.5149; `maybeEval` 2.0.11 l.4022; `parseHTMLUnsafe` 2.0.11 l.528.
- actix-web 4.18.2 `src/middleware/default_headers.rs:179-180`: inserts
  only if the response lacks the header.
- `deploy/Caddyfile`: no `header` directive; `deploy/compose.yaml:30`:
  `KAUPPA_COOKIE_SECURE: "true"`; `.env.example:13`: `false` for local runs.
- `.agentic/mentorship/security.md` has no CSP or header guidance yet (a
  pack update, if wanted, is the RR's roll-back, not this mission's).
