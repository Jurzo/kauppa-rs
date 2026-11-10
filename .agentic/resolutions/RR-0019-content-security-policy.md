# Resolution Record — Content-Security-Policy and companion headers

```yaml
---
id: RR-0019
closes: CRP-0019
mission: MB-0031
decided_by: "@juuso.kalliomaki"
role: security
risk_tier: medium
decided_at: 2026-11-10T08:17:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/mentorship/security.md
  - .agentic/missions/MB-0031-content-security-policy.md
  - .agentic/consults/CRP-0019-content-security-policy.md
---
```

## Decision and outcome

Approve CRP-0019's **recommended set, Q1–Q6**. The app sends an enforcing
Content-Security-Policy on every response, together with three companion
headers. The headers come from a `csp` module wrapped outermost in
`main.rs`. htmx runs under the policy through one `htmx-config` meta tag.

Sub-answers:

1. **Policy (Q1).** Exactly this one-line header, identical for every
   response:

   ```
   Content-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self'; object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'
   ```

   - No `data:` in `img-src`, no inline styles, and no `'unsafe-inline'`,
     `'unsafe-eval'`, nonce or hash anywhere.
   - `connect-src` and `manifest-src` fall back to `default-src 'self'`.
   - No `require-trusted-types-for` and no `upgrade-insecure-requests`.
2. **Placement (Q2, Option A).** The headers are set in the app, not in
   Caddy.
   - A `crate::csp` module exposes one builder, for example
     `csp::headers(&cfg)`. It returns actix-web's
     `middleware::DefaultHeaders`, so no new dependency is needed.
   - `main.rs` adds `.wrap(csp::headers(…))` as the **last** `.wrap`, which
     makes it the outermost middleware. It then also covers responses from
     the session and identity middleware, the cross-site `403`, `/assets`,
     extractor `400`s, `WebError` bodies and the catch-all `404`.
   - `DefaultHeaders` never overwrites a header the response already has.
     The register page's `Referrer-Policy: no-referrer` (RR-0012
     sub-answer 5) therefore stays.
   - `test_support::wired_app!` gets the same `.wrap`. A guard test (P3)
     asserts that both `main.rs` and `wired_app!` carry it.
   - `deploy/Caddyfile` gets no `header` directive.
3. **Mode (Q3, (i)).** The policy enforces from the first release. There is
   no `Content-Security-Policy-Report-Only` phase and no env switch.
4. **Companion headers (Q4).** The same `DefaultHeaders` sends:

   | Header | Value | When |
   | --- | --- | --- |
   | `X-Content-Type-Options` | `nosniff` | every response |
   | `Referrer-Policy` | `same-origin` | every response that doesn't set its own |
   | `Strict-Transport-Security` | `max-age=31536000` | only when `KAUPPA_COOKIE_SECURE=true`; no `includeSubDomains`, no `preload` |

   - HSTS comes from the app, not from Caddy (the CRP's sub-question).
   - No `X-Frame-Options`, because `frame-ancestors 'none'` covers it. No
     `Cross-Origin-Opener-Policy` and no `Permissions-Policy`.
5. **`htmx-config` (Q5, platform).** `templates/layout.hbs` gets exactly
   one meta tag in `<head>`, placed before the htmx script:

   ```html
   <meta name="htmx-config" content='{"allowEval":false,"allowScriptTags":false,"includeIndicatorStyles":false}' />
   ```

   - `allowScriptTags:false` is a **brief delta to P2** (roll-back below).
   - htmx's indicator rules move into `assets/app.css`. They are copied
     from the htmx version actually served, which is 2.0.11 once MB-0030
     merges:
     `.htmx-indicator{opacity:0;visibility:hidden}`
     `.htmx-request .htmx-indicator,.htmx-request.htmx-indicator{opacity:1;visibility:visible;transition:opacity 200ms ease-in}`.
   - P2's test asserts there is exactly one `htmx-config` meta, with these
     keys. If MB-0029 adds keys, they go into this same tag, whichever PR
     lands second.
6. **Ordering (Q6).** Implementation starts after MB-0030 merges. It does
   not depend on MB-0029 or MB-0040.
   <!-- check: MB-0031's Context says "Order: after MB-0030 (htmx upgrade) and MB-0029"; CRP-0019 Q6 and this answer drop the MB-0029 dependency. Roll-back below rewords that line. Confirm. -->

This RR is the approval that RR-0015's "Does not cover" left open: a CSP
header, `htmx-config`, and a change to htmx's behaviour. RR-0015
constraint 5's CSP follow-up is closed by MB-0031 under this RR.

Covers:

- MB-0031's implementation PR, with P1–P4 and the P2 delta below;
- the new `csp` module and the one `.wrap` line it adds to `main.rs`, a
  security trigger path (session middleware configuration);
- the matching `.wrap` in `wired_app!` and the guard test that pins both;
- the `htmx-config` meta in `layout.hbs` and the indicator rules in
  `app.css`;
- the `security.md`, brief and CRP changes below.

Does not cover:

- any other directive or value, including `data:` images, `'unsafe-*'`,
  nonces, hashes, report endpoints or another origin;
- a CSP or any other header in `deploy/Caddyfile`;
- HSTS `includeSubDomains` or `preload`, or HSTS without
  `KAUPPA_COOKIE_SECURE=true`;
- `X-Frame-Options`, COOP, `Permissions-Policy` or other headers;
- any other `main.rs` change: session, identity, cookie and cross-site
  settings, and the order of the existing middleware, stay as they are;
- any `htmx-config` key besides the three above (MB-0029's keys need
  their own approval);
- changing the vendored htmx file or its version (MB-0030, RR-0015).

## Rationale and trade-offs

- **Q1, chosen.**
  - The templates are already clean: the only script is the same-origin
    vendored htmx with `integrity`, and there is no inline style, handler
    or `javascript:` URL.
  - The strictest policy that works costs nothing. `img-src data:` would
    only widen it with nothing using it, so it waits for a use.
  - Trusted types were rejected because htmx swaps through `innerHTML` and
    would break.
- **Q2 A, chosen.**
  - It is the one place every response passes through, including
    `/assets` and the catch-all `404`.
  - Local runs, tests and self-hosters without Caddy all get the headers
    (AGENTS.md goal 3), and P1 can test them.
  - `DefaultHeaders` is in actix-web already, and it keeps headers a
    handler set.
  - **B (Caddy only), rejected.** No header in tests or for self-hosters
    without Caddy. RR-0015 Q5 C already rejected it.
  - **C (both), rejected.** Browsers enforce the intersection of every CSP
    they receive, so two copies that drift break pages in production only.
- **Q3 (i), chosen.**
  - Without a reporting endpoint (a brief non-goal), report-only shows
    violations only in each user's console. That adds little over P4's
    browser check before merge.
  - Rollback is a revert.
  - **(ii) and (iii), rejected.** A second PR, or an env switch and its
    test matrix, for a one-off.
- **Q4, chosen.**
  - `nosniff` matches the exact `SERVED_AS` content types.
  - `same-origin` loses nothing, since nothing links off-site.
  - HSTS is tied to the deployment's HTTPS signal and stays off the
    self-hoster's other subdomains. A browser that saw it can't be made to
    forget it for a year, which is why its scope is narrow.
  - XFO only duplicates `frame-ancestors` in every target browser.
- **Q5, chosen.**
  - Without `includeIndicatorStyles:false`, htmx injects a `<style>` on
    every page, which `style-src 'self'` blocks (P4).
  - `allowEval:false` makes htmx refuse eval-based features explicitly.
    The app uses none of them.
  - `allowScriptTags:false` stops htmx from running a script that injected
    markup smuggles into a swap. The app swaps no scripts.
  - The config is read with `JSON.parse`, which CSP doesn't affect.
- **Q6, chosen.** The indicator CSS and P2's script path follow the htmx
  version actually served, so MB-0030 goes first. The defaults the policy
  depends on are the same in 2.0.3 and 2.0.11.

## Approvals and authority

Approved by security-owner @juuso.kalliomaki (Q1–Q4, Q6, and the `main.rs` change
under the feature runbook's security trigger). Approved by platform-owner
@juuso.kalliomaki (Q5: `htmx-config` and the indicator CSS, which RR-0015 did not
cover).

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0019-content-security-policy.md`
- Mission Brief: `.agentic/missions/MB-0031-content-security-policy.md`
- Continuity Pack: `.agentic/continuity/MB-0031.md`
- Branch: `agentic/mb-0031-csp`
- Related decisions:
  - RR-0015: Q5 deferred the CSP here, rejected Caddy-only, and its
    constraint 4 forbids external script origins;
  - RR-0010 and RR-0011: `/assets` is registered first, outside the
    cross-site scope;
  - RR-0012 sub-answer 5: the register page's headers.
- Siblings: CRP-0017 (MB-0030), CRP-0018 (MB-0029), MB-0040.
- Evidence links (CRP-0019 → Evidence bundle):
  - a template grep finding no inline script, style, handler or
    `javascript:`;
  - htmx 2.0.3 and 2.0.11 config defaults and `insertIndicatorStyles`;
  - actix-web 4.18.2 `default_headers.rs:179-180`, which only inserts
    headers the response lacks;
  - `deploy/compose.yaml:30` (`KAUPPA_COOKIE_SECURE: "true"`).

## Constraints and follow-on rules (Hardened)

1. **One policy, one place.** The CSP and the companion headers come only
   from `csp::headers`, wrapped outermost in `main.rs` and mirrored in
   `wired_app!`. The guard test enforces both. Setting them per route, in
   `routes::configure` or in Caddy needs an RR.
2. **The policy only tightens without an RR.** Adding a source, `data:`,
   `'unsafe-inline'`, `'unsafe-eval'`, a nonce or a hash to any directive,
   or switching to report-only, needs security approval through an RR.
3. **Templates stay CSP-clean.** There is no inline `<script>`, `<style>`,
   `style=`, `on*=`, `javascript:` URL, `hx-on*`, `js:` value or
   `hx-trigger` filter. P2's scan enforces this. A feature that needs one
   is escalated, not worked around.
4. **One `htmx-config` tag.** It has exactly one meta, and it keeps
   `allowEval:false`, `allowScriptTags:false` and
   `includeIndicatorStyles:false`. Another mission's keys join the same
   tag under that mission's own approval. Turning one of these three back
   on needs an RR.
5. **Handlers may only tighten headers.** A handler may set a stricter
   header of its own, such as `Referrer-Policy: no-referrer`, and
   `DefaultHeaders` keeps it. A handler must not weaken or remove the
   policy.
6. **HSTS stays narrow.** It is sent only when `KAUPPA_COOKIE_SECURE=true`,
   with `max-age=31536000` and no `includeSubDomains` or `preload`.
   Widening it needs an RR.
7. **Indicator CSS follows htmx.** When the vendored htmx version changes,
   the `app.css` indicator rules are re-copied from that version in the
   same PR.

## Roll-back into the system (Hardened)

To be applied in MB-0031's implementation PR:

- `.agentic/mentorship/security.md` → Rules (must): after
  "**Cross-site requests.**", add:
  "**Security headers.** Every response carries the Content-Security-Policy
  `default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self';
  object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors
  'none'`, plus `X-Content-Type-Options: nosniff`, `Referrer-Policy:
  same-origin`, and HSTS (`max-age=31536000`) when
  `KAUPPA_COOKIE_SECURE=true`. All of them come from one `csp::headers`
  middleware, wrapped outermost in `main.rs` and mirrored in the test app.
  A handler may tighten a header but never weaken it. Templates have no
  inline script, style or event handler, and htmx runs with `allowEval`,
  `allowScriptTags` and `includeIndicatorStyles` off (MB-0031, RR-0019).
  Loosening the policy needs an RR."
- `MB-0031`:
  - `resolutions: []` → `resolutions: [RR-0019]`;
  - P2's property: "`htmx-config` sets `allowEval` false and
    `includeIndicatorStyles` false" → "`htmx-config` sets `allowEval`,
    `allowScriptTags` and `includeIndicatorStyles` false (RR-0019)";
  - Context: "Order: after MB-0030 (htmx upgrade) and MB-0029 (form errors
    under htmx), which both touch `<head>` and htmx's configuration." →
    "Order: after MB-0030 (htmx upgrade); independent of MB-0029 and
    MB-0040, which share the one `htmx-config` tag if they add keys
    (RR-0019).";
  - Conceptual plan step 1: append "Answered by RR-0019."
- `CRP-0019`: `state: open` → `answered`.
- `.agentic/continuity/MB-0031.md`: record the RR under Decisions and
  constraints.

No Runbook change. The feature runbook's `main.rs` trigger already routes
this to security.

## Supersession and history

Status: active once approved. Supersedes nothing. It fills the gap RR-0015
"Does not cover" left (CSP, `htmx-config`, htmx behaviour) and closes
RR-0015 constraint 5's CSP follow-up. RR-0010 and RR-0011's wiring (the
`/assets` service first, then the one catch-all scope) is unchanged. The
new `.wrap` adds no `.service` or `.configure` line.
