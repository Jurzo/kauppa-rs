# Resolution Record — App-wide cross-site request protection: Origin check with Fetch Metadata

```yaml
---
id: RR-0010
closes: CRP-0010
mission: MB-0018
decided_by: "@juuso.kalliomaki"
role: security
risk_tier: high
decided_at: 2026-11-07T10:39:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/mentorship/security.md
  - .agentic/missions/MB-0018-cross-site-request-protection.md
  - .agentic/consults/CRP-0010-csrf-mechanism.md
---
```

## Decision and outcome

Approve CRP-0010 **Option A with D as a reject-only layer**: one app-wide
check on every non-safe request (anything but GET, HEAD and OPTIONS), in this
order.

1. **Fetch Metadata (D, reject only).** `Sec-Fetch-Site: cross-site` or
   `same-site` → 403. `same-origin` or `none`, or no header at all, falls
   through to step 2. Browsers only send it over HTTPS and to localhost, so it
   never decides alone.
2. **Origin (A).** If `Origin` is present, it must equal the app's origin, or
   the request gets 403. `Origin: null` is a mismatch.
3. **Referer fallback (A).** With no `Origin`, the `Referer`'s scheme, host
   and port must equal the app's origin.
4. **No proof → 403** (MB-0018 P3). This is stricter than Go's
   `CrossOriginProtection`, on purpose.

Sub-answers:

1. **Public origin.**
   - Approve a new optional `KAUPPA_PUBLIC_ORIGIN`: read in `config.rs`,
     validated and normalized at startup (scheme, host and optional port; no
     path), and documented in `.env.example`.
   - When unset, the fallback is (i): the request's own `Host`, with the
     scheme from `KAUPPA_COOKIE_SECURE`.
   - It takes one origin. A comma-separated list would be a later amendment
     through an RR.
2. **`security.md` should → must.** The CSRF rule moves to "Rules (must)",
   worded as in the roll-back below, and "double-submit cookie" is dropped.
3. **Wiring.** One middleware in the `App` wiring, applied where
   `test_app!` also gets it (e.g. `routes::configure` or a shared
   constructor), plus `main.rs` if needed. Session middleware and cookie
   attributes are unchanged: `SameSite=Lax` stays.

Covers:

- MB-0018's implementation PR, with P1–P6 as written;
- the new configuration variable;
- the `security.md` change below;
- test-support changes so route tests send valid proof by default.

Does not cover:

- a synchronizer or double-submit token (B or C);
- exempting any route;
- CORS for third-party clients;
- changing cookie attributes or session storage;
- more than one public origin.

## Rationale and trade-offs

- **A+D — chosen.**
  - One control in one place covers all 34 POST routes, including login,
    register and logout, with no session, no template change and no
    dependency.
  - D adds an early, cheap rejection wherever browsers send Fetch Metadata.
  - Route tests change mechanically: `test_support` sends a matching
    `Origin`.
- **B — not chosen.**
  - It is independent of browser headers, but it would need:
    - anonymous sessions before login;
    - a token rotation in `auth.rs`;
    - hidden fields in 35 forms;
    - changes to about 111 test call sites.
  - Its natural htmx placement (`hx-headers` on `<body>`) goes stale after
    boosted navigation.
- **C — not chosen now.** It can be added later behind the same middleware,
  without undoing A, if a deployment needs it.
- **Same-host fallback.** It keeps "one binary, one `.env`" self-hosting
  working out of the box (AGENTS.md goal 3). `KAUPPA_PUBLIC_ORIGIN` is for
  installs behind a proxy that rewrites `Host`.

## Approvals and authority

Approved by security-owner @juuso.kalliomaki; the configuration variable
(sub-answer 1) by platform-owner @juuso.kalliomaki

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0010-csrf-mechanism.md`
- Mission Brief: `.agentic/missions/MB-0018-cross-site-request-protection.md`
- Earlier decisions: RR-0002 constraint 6; RR-0008 (deferral to MB-0018)
- Evidence links: CRP-0010 → Current state and Evidence bundle:
  - 34 POST and 13 GET routes, no GET route that writes, no
    PUT/PATCH/DELETE;
  - 35 forms, all `method="post"`;
  - `test_app!` wiring;
  - htmx's `hx-boost` body swap;
  - nothing in the code reads `Origin`, `Referer` or `Sec-Fetch-*` today.

## Constraints and follow-on rules (Hardened)

1. **One control, no exemptions.** Every non-safe request passes the same
   check. Exempting a route, such as a future webhook, needs an RR.
2. **Safe methods stay side-effect free.** A GET, HEAD or OPTIONS handler
   never writes. That is what makes skipping the check for them safe.
3. **Fail closed.** No proof, a mismatch, or `Origin: null` → 403 with no
   side effects. Rejections are logged at WARN, with method, path and reason,
   and without cookies or emails (MB-0018 P6; MB-0020's levels).
4. **Tests and smoke runs send proof.** `test_support` sends a matching
   `Origin` by default, and only the CSRF tests strip or forge it. Curl smoke
   tests send `-H 'Origin: …'`. The MB-0018 route-coverage guard (P4) stays
   in the suite.
5. **Closes the deferrals.** When MB-0018 merges, RR-0002 constraint 6 is
   met and RR-0008 constraint 3 (keep the app on localhost) lifts.

## Roll-back into the system (Hardened)

To be applied in MB-0018's implementation PR:

- `.agentic/mentorship/security.md`:
  - remove "Add CSRF protection (double-submit cookie or origin check) before
    any deployment that is reachable outside localhost" from Rules (should);
  - add to Rules (must): "Every non-safe request passes the app-wide
    cross-site check in one place (`Origin`/`Referer` against the app's
    origin, with `Sec-Fetch-Site` as a reject-only layer; MB-0018, RR-0010).
    No route is exempt without an approved RR. Safe methods stay side-effect
    free."
- `MB-0018`:
  - `resolutions` gains `RR-0010`;
  - P3's "per the decision in this brief's CRP" points at RR-0010.
- `CRP-0010`: `state: open` → `answered`.

## Supersession and history

Status: active. Supersedes nothing. Completes RR-0002 constraint 6 and the
deferral in RR-0008.
