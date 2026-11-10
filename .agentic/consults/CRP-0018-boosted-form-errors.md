# Consultation Request Pack — Show 4xx form pages on boosted forms (MB-0029)

```yaml
---
id: CRP-0018
mission: MB-0029
owner: "@juuso-kalliomaki"
target_role: platform
risk_tier: low
urgency: medium
state: answered
runbook_trigger: "MB-0029 autonomy.escalate: choosing the mechanism (global `htmx-config` `responseHandling`, per-form attributes, or the response-targets extension, a new vendored file) → platform-owner; any change to an auth handler's status, headers or body (`crates/kauppa-web/src/routes/auth/`, security trigger path) → security; a mechanism needing inline script or eval (MB-0031's CSP) → security. RR-0015 constraint 1 (adding another vendored file needs an RR) and Does not cover (`htmx-config`, `hx-*` behaviour changes). AGENTS.md Do-not-touch (adding a vendored file is a platform decision)."
---
```

## Decision statement

Choose how a boosted form whose handler answers 4xx with a full page gets
that page swapped in under htmx, without changing what fragment requests
do, and whether to fix the history-cache exposure of secrets found while
checking it.

Routing: platform-owner for Q1–Q4; security for Q5 and the auth-page
sub-question in Q1. Both are @juuso-kalliomaki, so one RR can answer
everything.

## Current state

Verified on `50322c0` (2026-11-09), htmx 2.0.3 vendored.

### How htmx treats the app's requests

- `templates/layout.hbs:12`: `<body hx-boost="true">`. Every `<form>`
  without its own `hx-post` and every local link is boosted: htmx sends it
  with `HX-Request: true` and `HX-Boosted: true`, swaps the response into
  `<body>` and pushes the URL.
- Every other form has `hx-post` + `hx-target` + `hx-swap` (32 of them) and
  sends `HX-Request` without `HX-Boosted` (a fragment request);
  `routes/request.rs:28` `wants_fragment` tells the two apart.
- htmx 2's default `responseHandling` (2.0.3 `dist/htmx.js` l.264–267; the
  same in 2.0.11): swap `2xx`/`3xx` (except 204), **don't swap** `4xx`/`5xx`.
  No `htmx-config` meta exists, so the default applies.

### Inventory: who answers 4xx, and with what (grep over `routes/`)

**Boosted full-page forms (no `hx-post`), the ones this mission is about:**

| Form | Template | Handler | 4xx answers today | Under htmx today |
| --- | --- | --- | --- | --- |
| Sign in | `auth/login.hbs:7` | `auth/mod.rs:125` `login_submit` | **401** login page "Invalid email or password." (l.181–188); **429** login page + `Retry-After` (l.162–171); 400 plain text if the form can't be parsed (browser `required` prevents it) | dropped: nothing happens |
| Register | `auth/register.hbs:8` | `auth/mod.rs:209` `register_submit` | **400** register page "This invitation link isn't valid…" (l.319–325); **409** register page "An account already uses that email…" (l.291–300); **429** register page (l.242–260); 400 plain text for an unparseable form (`private(Err)`, l.341) | dropped |
| Create invitation | `admin/invitations.hbs:20` | `admin/mod.rs:168` `create_invitation` | **400** invitations page with the error (note too long, l.177–185; the `maxlength="200"` attribute normally prevents it) | dropped |
| Revoke invitation | `admin/invitations.hbs:45` | `admin/mod.rs:221` | 404 **plain text** "not found" (`WebError`) only | dropped |
| Sign out | `layout.hbs:21` | `auth/mod.rs:353` | none (303); only the cross-site check's 403 plain text | — |

So the pages in scope are **login (401, 429), register (400, 409, 429) and
create invitation (400)**: the brief's P1 list plus the admin form.

**Fragment forms (`hx-post`):** every mutation handler answers htmx with
**200 and the fragment, error included** (`lists/mod.rs:293`, `:378`;
`lists/store.rs:115`; `lists/shares.rs:223`; `recipes/mod.rs:474`, `:552`;
`recipes/add_to_list.rs:66`; `categories.rs:87`; `products/mod.rs:316`).
Their full-page 4xx answers (400/409) go only to plain, non-htmx POSTs.
The only 4xx a fragment request can get is a **plain-text** body:

- `WebError` (`error.rs:39-57`): 401 "unauthorized" (expired session), 403
  "forbidden" (weak share role), 404 "not found" (another user's or a
  deleted row), 409/400 from a `?`;
- the cross-site check's 403 "forbidden" (`csrf/mod.rs:211`);
- actix's extractor 400 for an unparseable form.

Today these are dropped too (nothing happens). P2 asks that they stay
dropped, or get an answer the RR defines.

**Boosted links (GET)** also get plain-text 4xx: a signed-out user clicking
"Lists" gets 401 "unauthorized", a non-admin opening `/admin` gets 404.
Today nothing happens on click. Not in this brief's scope, but option A
changes it (below).

### Constraints on the mechanism

- Brief forbidden: changing status codes to 200, inline `<script>`,
  executable files outside `assets/vendor/`, modifying a vendored file.
  So a hand-written `htmx:beforeSwap` listener in `assets/app.js` is out;
  so is `hx-on::before-swap` (needs eval, which MB-0031 turns off).
- htmx 2.0.x has **no per-element `responseHandling` attribute**; the only
  per-element routes are the response-targets extension's
  `hx-target-<code>` attributes, or opting a form out of boost.
- htmx reads exactly one `<meta name="htmx-config">`
  (`querySelector`, then a shallow merge, 2.0.3 l.4970–4984): MB-0031 adds
  that meta for `allowEval`/`includeIndicatorStyles`, so if this mission
  adds one too, the two missions share a single tag.

## Questions, options and recommendations

### Q1 — Mechanism (platform-owner; security for the auth pages and CSP)

**Option A — Global `responseHandling` in an `htmx-config` meta, plus a
server header that stops fragment swaps**

- `<meta name="htmx-config" content='{"responseHandling":[{"code":"204","swap":false},{"code":"[23]..","swap":true},{"code":"4..","swap":true,"error":false},{"code":"...","swap":false,"error":true}]}'>`.
- Alone, this breaks P2: a fragment request's plain-text 404 would replace
  `#list-…` with "not found" (`hx-swap="outerHTML"`). So the server must
  add `HX-Reswap: none` to every 4xx answer to a fragment request
  (`wants_fragment`), which htmx honours after `responseHandling`
  (2.0.11 l.4917–4918). That needs a response middleware covering
  `WebError`, the cross-site 403 and extractor errors: in `main.rs`
  (security trigger path) and mirrored in `test_support::wired_app!`, since
  `routes::configure`'s body is pinned by `csrf::tests::coverage` rule 3.
- Pros: one place; every current and future boosted form is covered; no
  new vendored file. CSP: JSON in a meta, no eval or inline code.
- Cons: changes every htmx request's handling; a server middleware in a
  security path; boosted *links* that answer plain-text 4xx (signed-out
  "Lists" → 401) would replace the page with the bare word "unauthorized"
  instead of doing nothing.
- Risk: medium (wide effect; P2 depends on the middleware being complete).

**Option B — response-targets extension on the three forms (recommended)**

- Vendor `htmx-ext-response-targets` **2.0.4** (latest, 0BSD,
  `dist/response-targets.min.js`, 1,692 B) under RR-0015's rules, load it
  from `layout.hbs` after htmx, and mark only the in-scope forms:
  `<form method="post" action="/auth/login" hx-ext="response-targets"
  hx-target-4xx="body">` (and register, create invitation). The `4xx`
  spelling avoids `*` in attribute names.
- The extension (read in full, 3,740 B unminified) listens to
  `htmx:beforeSwap`; for a non-200 answer it looks up `hx-target-<code>`
  on the requesting element and its ancestors, and if found sets
  `shouldSwap = true` and the target. No eval, no inline style, no DOM
  script creation.
- Pros: scoped by construction: fragments and boosted links never carry
  the attribute, so P2 holds with no server change; auth handlers
  untouched (template attributes only); CSP-compatible (`script-src
  'self'`); works on 2.0.3 and 2.0.11 (peer `htmx.org ^2.0.2`).
- Cons: a second vendored file with its own constants, `SERVED_AS` row and
  `integrity`; `no_external_scripts` asserts exactly one layout script
  today and must allow two; each future full-page form needs the
  attributes (P1's test pins the list); +1.7 KB (≈0.6 KB gzipped) per
  first page load.
- Provenance (2026-11-09): npm `dist.integrity`
  `sha512-Q/yfH0N2A40j903mr6ldGV3qLWMQeufROylYIbYQLBGrCpmydflxXmQwDOEhEWdPnBTgiHofkAo0gQ3S1BmyxQ==`
  matches the tarball; `dist/response-targets.min.js` sha256
  `bde3061dd9b0c7a0b3c7bae63784312bbb7a3a798411b750fe1a5c07a286e63a`,
  sha384 `T41oglUPvXLGBVyRdZsVRxNWnOOqCynaPubjUVjxhsjFTKrFJGEMm3/0KGmNQ+Pg`,
  which **equals the SRI htmx's own docs publish** for
  `htmx-ext-response-targets@2.0.4` at `v2.0.11` (`www/content/docs.md`
  l.1168). `bigskysoftware/htmx-extensions` has no GitHub releases or
  tags, so there is no third digest; RR-0015 constraint 3 asks for one
  only for htmx itself from 2.0.11 on. No advisories.
- Risk: low.

**Option C — Opt the three forms out of boost (`hx-boost="false"`)**

- The browser does a plain POST and renders whatever comes back, 4xx
  included, exactly as without htmx.
- Pros: smallest change (one attribute per form); no JS, no config, no
  vendored file; behaviour identical to the tested no-htmx path.
- Cons: a full page load on submit, so P4's "without a reload" fails and
  the brief needs a delta (P1 "under htmx" → "with htmx loaded"; P4 drops
  "without a reload").
- Risk: lowest.

Not offered: a hand-written listener script (forbidden), `hx-on` (eval),
200 status codes (forbidden).

**Recommendation: B.** It is the only option that meets P1–P4 as written
without touching a handler, the session wiring or fragment behaviour. If
you'd rather not vendor a second file, **C** with the brief delta is the
safe fallback; A only if you want every boosted error shown, including the
bare-text ones.

Security sub-question (auth pages): under B and C the auth handlers'
status codes, headers and bodies don't change; only `login.hbs` and
`register.hbs` gain attributes. Is a template-only change to the auth
pages acceptable without a further security RR beyond this one?

### Q2 — Which forms are in scope (platform-owner; brief: agent may decide)

Proposed: login, register, create invitation (the three that render a 4xx
page). Not: sign out (no 4xx page), revoke invitation (only a plain-text
404), and every `hx-post` form (P2). P1's test
(`templates::tests::boosted_error_pages_swap`) lists the three and asserts
each carries `hx-ext="response-targets"` and `hx-target-4xx="body"`, and
that the layout loads the extension after htmx with its `integrity`.
P2's test (`fragment_errors_not_swapped`) asserts no element with
`hx-post`/`hx-get`, and no ancestor (`<body>`, `<main>`, sections), carries
`hx-ext` or any `hx-target-<code>`, and that no `htmx-config` sets
`responseHandling`.

### Q3 — History and URL after an error swap (platform-owner)

A boosted swap pushes the response URL (`determineHistoryUpdates`, the same
in 2.0.3 and 2.0.11), whichever option swaps it:

- login: `/auth/login` → `/auth/login` (unchanged);
- register: `/auth/register?invite=<token>` → `/auth/register`. The error
  page still carries the token in its hidden field, so resubmitting works;
  a manual reload shows "Registration needs an invitation". The token
  leaves the address bar, which RR-0012 sub-answer 5 favours.
- C does the same (the browser navigates to the form's `action`).

Keeping the old URL would need `HX-Push-Url: false` on the auth error
answers (a header change in `routes/auth/`, security), or `hx-push-url`
on the form, which would also stop the success redirect's URL update.

**Recommendation:** accept the push; read P4's "the URL stays the form's"
as the form's `action`.

### Q4 — Ordering with MB-0030 and MB-0031 (platform-owner)

- `responseHandling`, `codeMatches` and `determineHistoryUpdates` are the
  same in 2.0.3 and 2.0.11, and response-targets 2.0.4 supports both, so
  the decision doesn't depend on the version.
- Implement **after MB-0030 merges** anyway, so P4's browser check runs on
  the version users will get, and the layout's script block changes once.
- Under B, MB-0031's `script-src 'self'` covers the extension; under A,
  MB-0031 must merge its keys into this mission's `htmx-config` meta (one
  tag), whichever lands second.

### Q5 — History cache keeps secrets (security; found during this S1)

On every boosted navigation htmx snapshots the current `<body>` HTML into
the history cache before swapping (`saveCurrentPageToHistory`): in
**`localStorage` on 2.0.3** (persists after the browser closes), in
`sessionStorage` from 2.0.5 (per tab). Two pages hold secrets in attribute
values that land in that snapshot:

- `auth/register.hbs:9`: the hidden `invite` token (any boosted click from
  the register page, e.g. "Sign in", and under this mission the error
  page too);
- `admin/invitations.hbs:12`: the one-time invitation link ("shown only
  this once"), in `value="…"`.

htmx skips the snapshot when any element on the page has
`hx-history="false"` (2.0.3 l.3124–3134; 2.0.11 l.3304–3326); the page
is then refetched on Back instead of restored.

- **(a)** `hx-history="false"` on the register and invitations pages
  (template-only; covered by a test) — **recommended**;
- (b) `historyCacheSize: 0` in `htmx-config` (no snapshots at all; every
  Back refetches; touches MB-0031's meta);
- (c) leave it; rely on MB-0030's move to `sessionStorage`.

Should (a) ship in MB-0029 (it's the same two templates, and the error
swap adds one more path into the cache), or as its own small brief?
**Recommendation:** in MB-0029, recorded as a brief delta (new property
P5, test `templates::tests::secret_pages_skip_history`).

## Recommendation and question

| Q | Recommendation |
| --- | --- |
| Q1 Mechanism | B: response-targets 2.0.4, vendored under RR-0015's rules, on the three forms |
| Q2 Scope | login, register, create invitation; tests as named |
| Q3 URL | Accept the pushed `action` URL |
| Q4 Order | After MB-0030; no `htmx-config` from this mission under B |
| Q5 History | `hx-history="false"` on register and invitations, as MB-0029 P5 |

Crisp questions to answer:

1. **Q1:** Mechanism **B** (vendor `htmx-ext-response-targets` 2.0.4 from
   npm, checked against `dist.integrity` and the docs SRI, no GitHub
   digest available), or C (opt out of boost, with the P1/P4 brief delta),
   or A (global rule plus `HX-Reswap: none` middleware in `main.rs`)?
2. **Q1 security:** are template-only attribute changes to `login.hbs` and
   `register.hbs` acceptable under this RR, with no change to any auth
   handler?
3. **Q2:** Are login, register and create invitation the right set?
4. **Q3:** Accept that a register error leaves the URL at `/auth/register`
   (token gone from the address bar)?
5. **Q4:** Implement after MB-0030 merges?
6. **Q5:** Add `hx-history="false"` to the register and invitations pages
   in this mission (brief delta P5), or (b), (c), or a separate brief?

## Blast radius and rollback

- **What can break:** the three forms' error display under htmx; under B,
  a wrong `integrity` stops the extension (errors are dropped again, as
  today); under A, fragment swaps if the middleware misses a path.
- **Who is affected:** users signing in or registering, and the admin.
  No data, schema or session change. Without htmx nothing changes (P3).
- **Rollback plan:** revert the PR (attributes, script tag, vendored file,
  constants). Under Q5(a), reverting re-enables snapshots of those pages.

## Minimal context

- MB-0029 P1–P4, forbidden list (no 200s, no inline script, nothing
  executable outside `assets/vendor/`, vendored files untouched).
- RR-0015 constraints 1–4 (vendored files), RR-0012 sub-answer 5 (register
  responses: `no-store`, `no-referrer`), RR-0013 (429 pages and the 4xx
  failure count, unchanged).
- Siblings: CRP-0017 (MB-0030, version), CRP-0019 (MB-0031, CSP and the
  shared `htmx-config`).

## Evidence bundle (Hardened)

- Grep: `grep -rn "StatusCode::\|HttpResponse::(BadRequest|…)\|is_hx\|wants_fragment" crates/kauppa-web/src/routes`
  (results summarized in the inventory, file:line above); `grep -rn "<form"
  templates` (37 forms; 5 without `hx-post`: logout, login, register,
  create invitation, revoke invitation).
- htmx source (npm tarballs 2.0.3 and 2.0.11, `dist/htmx.js`):
  `responseHandling` defaults l.264–267 (both); `resolveResponseHandling`
  and `codeMatches` identical; `handleAjaxResponse` 2.0.11 l.4855–4925
  (`HX-Reswap` overrides after `responseHandling`; `htmx:beforeSwap` can
  set `shouldSwap`/`target`); `determineHistoryUpdates` 2.0.11 l.4710
  (boosted → push response path); `saveCurrentPageToHistory` honours
  `hx-history="false"`; history storage `localStorage` (2.0.3) vs
  `sessionStorage` (2.0.11).
- response-targets: `curl https://registry.npmjs.org/htmx-ext-response-targets`
  → `latest: 2.0.4` (2025-10-18, publisher `carsongross`, peer
  `htmx.org ^2.0.2`); tarball sha512 = `dist.integrity`; `LICENSE` 0BSD;
  hashes above; `gh api repos/bigskysoftware/htmx-extensions/releases` and
  `/tags` → empty; advisories → none.
- htmx docs at `v2.0.11` (`www/content/docs.md` l.1167–1168): the
  response-targets example with `integrity="sha384-T41oglUP…GmNQ+Pg"`.
