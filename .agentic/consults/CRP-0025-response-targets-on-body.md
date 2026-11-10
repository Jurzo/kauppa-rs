# Consultation Request Pack — Declare the response-targets extension on `<body>` (amend RR-0018)

```yaml
---
id: CRP-0025
mission: MB-0029
owner: "@juuso-kalliomaki"
target_role: platform
risk_tier: low
urgency: medium
state: answered
runbook_trigger: "RR-0018 constraint 5 (`hx-ext` never on `<body>`) blocks the only placement that works; templates under `templates/auth/` and `templates/admin/` and their route tests (security trigger paths, RR-0018 sub-answer 2)"
---
```

## Decision statement

Decide where `hx-ext="response-targets"` is declared so that MB-0029's
boosted form errors actually show. RR-0018 put it on the three forms, but
htmx never sees it there. The working placement, `<body>`, is one that
RR-0018 constraint 5 forbids.

## Current state

- MB-0029 merged in #58 (2026-11-10) with RR-0018's markup: each of the
  login, register and create-invitation forms carries
  `hx-ext="response-targets" hx-target-4xx="body"`, and `layout.hbs` loads
  the extension after htmx.
- The owner's P4 browser check (2026-11-10) failed: a wrong password, a bad
  invitation and a duplicate email still show nothing. Everything else on
  MB-0029's and MB-0031's checklists passed.
- **Root cause.** htmx 2.0.11 fires `htmx:beforeSwap` on the swap target,
  not on the element that made the request. In `handleAjaxResponse`
  (minified `Vn`), `r = responseInfo.target` and
  `ae(r, "htmx:beforeSwap", …)`. `triggerEvent` (`ae`) then calls
  `onEvent` only for extensions declared on that element or its ancestors
  (`getExtensions`, minified `Kn`, which reads `hx-ext` walking up). A
  boosted form's target is `<body>`, and the forms sit inside `<body>`,
  not above it, so the extension never receives the event. htmx then
  treats the 4xx as an error and doesn't swap.
- The extension finds `hx-target-<code>` by walking up from
  `requestConfig.elt`, which is the form. So the target attribute is
  already in the right place; only `hx-ext` is not.
- MB-0029's P1/P2 tests check markup, not browser behaviour, so CI passed.
  CRP-0018's analysis missed the event's dispatch element.

## Options and trade-offs

### Option A — `hx-ext="response-targets"` on `<body>`; `hx-target-4xx="body"` stays on the three forms only

- Pros:
  - Smallest change: one attribute moves, and handlers stay untouched.
  - Verified to work (Evidence).
  - The extension does nothing for an element unless that element or an
    ancestor carries an `hx-target-<code>`; only the three forms do (P2's
    test keeps it so).
- Cons:
  - The extension is active page-wide, so a future `hx-target-<code>`
    anywhere takes effect. P2's test and RR-0018 constraint 5 (reworded)
    still limit those attributes to full-page forms.
  - The one other path is `HX-Retarget`: on a non-200 response that carries
    an `HX-Retarget` header, the extension forces a swap
    (`responseTargetPrefersRetargetHeader`, default true). The app sends no
    `HX-Retarget` (grep of `crates/` and `templates/`: none).
- Risk: low.
- Cost / timeline: one template line, three attributes removed, P1/P2
  tests and the same two route-test assertions as #58 updated.

### Option B — global `responseHandling` in the `htmx-config` meta (CRP-0018 option A)

- Pros: no extension.
- Cons:
  - It changes every htmx request, including fragments.
  - Plain-text 4xx answers would replace the page.
  - It needs MB-0031's meta tag (RR-0019 constraint 4) and likely a server
    change.
- Risk: medium.
- Cost / timeline: larger; CRP-0018 rejected it.

### Option C — revert the swap

- Remove the extension and the attributes, and keep `hx-history="false"`.
- Pros: back to the known state.
- Cons: MB-0029's goal stays unmet; the errors are dropped as before.
- Risk: none.
- Cost / timeline: small, but MB-0029 reopens.

## Recommendation and question

Recommended option: A.

Because: it is the documented way to use htmx extensions (declare on an
ancestor of the swap target), keeps RR-0018's scope rule where it matters
(`hx-target-<code>` only on the three forms), and is verified.

Crisp question to answer: may `hx-ext="response-targets"` be declared on
the layout's `<body>`, with RR-0018 constraint 5 reworded so that it limits
`hx-target-<code>` (not `hx-ext`) to full-page boosted forms?

Owner's answer in session (2026-11-10): Option A ("hx-ext on <body>"), to be
recorded in RR-0025.

## Blast radius and rollback

What can break:
- Any element with an `hx-target-<code>` attribute, or any non-200
  response with `HX-Retarget`, gets swapped on error. Today that means only
  the three forms.

Who is affected:
- Users of the login, register and create-invitation forms with htmx
  loaded: they now see the error page.
- Plain POST is unchanged.

Rollback plan:
- Revert the PR. That returns to #58's state, where the errors are not
  shown and nothing else is affected.

## Minimal context

- Brief: MB-0029 P1 (forms show their 4xx page under htmx), P2 (fragments
  don't), P4 (browser), P5 (history, unaffected).
- RR-0018: sub-answer 1 (placement on the forms), sub-answer 3 (exactly
  three forms), constraint 5 (never `hx-ext`/`hx-target-<code>` on
  `<body>`, a section, or an `hx-post`/`hx-get` element).
- RR-0019 (MB-0031, merged in #59): `script-src 'self'` covers the
  extension. It adds no new `htmx-config` key.
- Impacted files:
  - `templates/layout.hbs`, `templates/auth/login.hbs`,
    `templates/auth/register.hbs`, `templates/admin/invitations.hbs`;
  - `crates/kauppa-web/src/templates/tests.rs`;
  - `crates/kauppa-web/src/routes/auth/tests/register_requires_invite.rs`
    and `crates/kauppa-web/src/routes/admin/tests/invites.rs`, one
    assertion each, matching the exact form tag.

## Evidence bundle (Hardened)

- **htmx source** (`assets/vendor/htmx-2.0.11.min.js`):
  - `function Vn(t,e){const n=e.xhr;let r=e.target;…if(!ae(r,"htmx:beforeSwap",m))return;…}`;
  - `function ae(e,t,n){…Vt(le(e),function(e){o=o&&(e.onEvent(t,r)!==false…)})…}`;
  - `Kn` reads `hx-ext` on the element and its parents.
- **Extension** (`response-targets.js` 2.0.4): `onEvent('htmx:beforeSwap')`
  reads `requestConfig.elt` and walks up for `hx-target-<code>`. With no
  match it changes nothing; with `HX-Retarget` on a non-200 it forces the
  swap.
- **Repro** (jsdom 24.1.3 against the release binary built from `main` at
  `e20ebcf`). The script submits a boosted wrong-password login. jsdom
  needs stubs for `XPathEvaluator` (`hx-on` lookup, unused here) and
  `CSS.escape`.
  - As merged: `htmx:beforeSwap` fires on `BODY` with 401, then
    `htmx:responseError` fires on `FORM` and nothing is swapped. The
    message is not shown.
  - With `hx-ext="response-targets"` added to `<body>` at runtime:
    `htmx:afterSwap` fires on `BODY` with 401. The message
    "Invalid email or password." is shown and the URL is `/auth/login`.
- **Grep:** no `HX-Retarget` / `retarget` in `crates/` or `templates/`.
- The owner's P4 run (2026-11-10): the error-message lines failed; every
  other MB-0029 and MB-0031 line passed.
