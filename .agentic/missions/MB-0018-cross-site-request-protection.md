# Mission Brief — App-wide protection against cross-site request forgery

```yaml
---
id: MB-0018
owner: "@juuso-kalliomaki"
goal: Every state-changing request in kauppa-web proves it came from the app's own pages, so a page on another site (or a sibling subdomain) can't make a signed-in user's browser share a list, edit someone else's list, delete data, or sign them out. The protection holds behind a reverse proxy and is guarded so new routes can't skip it.
non_goals:
  - Rate limiting, account lockout or other `security.md` "should" items.
  - Changing session storage, cookie names or the `SameSite=Lax` default (it stays as defence in depth).
  - Protecting safe methods (GET, HEAD, OPTIONS); they must stay side-effect free instead.
  - CORS support for third-party clients (there are none).
acceptance:
  - id: P1
    property: Every route that accepts a non-safe method (POST today; PUT, PATCH or DELETE if added) rejects a cross-site request with 403 and changes nothing, on every route the app registers, whether the request is a plain form post or htmx.
    check: cargo test -p kauppa-web csrf::rejects_cross_site_writes
  - id: P2
    property: Same-origin form posts and htmx requests still succeed, including the login and registration forms, on a direct connection and behind a reverse proxy configured with the app's public origin.
    check: cargo test -p kauppa-web csrf::same_origin_writes_succeed
  - id: P3
    property: A non-safe request that carries none of the chosen proof (no matching `Origin`, `Referer` or token, per the decision in this brief's CRP) is rejected, so stripping headers isn't a bypass.
    check: cargo test -p kauppa-web csrf::missing_proof_is_rejected
  - id: P4
    property: A route-coverage guard fails if a route accepting a non-safe method is registered outside the protection, so future handlers can't opt out by accident.
    check: cargo test -p kauppa-web csrf::every_write_route_is_covered
  - id: P5
    property: MB-0008's deferred P8 holds — share and revoke reject cross-origin posts — as a named test in its original module.
    check: cargo test -p kauppa-web routes::lists::tests::sharing_invites::share_rejects_cross_origin
  - id: P6
    property: Rejections are logged with method, path and the reason (missing or mismatched proof) at `warn`, without cookies, tokens or full emails.
    check: cargo test -p kauppa-web csrf::rejections_are_logged
context:
  - crates/kauppa-web/src/main.rs
  - crates/kauppa-web/src/config.rs
  - crates/kauppa-web/src/routes/mod.rs
  - crates/kauppa-web/src/routes/test_support.rs
  - templates/layout.hbs
  - .env.example
  - .agentic/mentorship/security.md
  - .agentic/consults/CRP-0008-share-csrf-origin-check.md
  - .agentic/resolutions/RR-0002-list-sharing-authz.md
  - .agentic/resolutions/RR-0008-share-csrf-deferral.md
autonomy:
  may_decide:
    - module layout of the protection (middleware vs extractor) once the mechanism is decided
    - test placement and the exact assertions
    - the rejection page's wording
  escalate:
    - { when: "choosing the mechanism (origin check, synchronizer or double-submit token, or both) — open a CRP before writing code", to: security }
    - { when: "adding a configuration variable such as `KAUPPA_PUBLIC_ORIGIN`", to: platform-owner }
    - { when: "any change to session middleware wiring in main.rs or to cookie attributes", to: security }
    - { when: "promoting security.md's CSRF rule from should to must (Mentorship Pack change)", to: security }
    - { when: "the mechanism seems to need a new dependency", to: platform-owner }
  forbidden:
    - add dependencies outside the dependency-bump runbook
    - weaken `SameSite=Lax` or other cookie attributes
    - exempt individual routes from the protection without an approved RR
    - lower lint strictness
    - edit AGENTS.md, Mentorship Packs or Runbooks without an approved RR
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/security.md@v1
  - .agentic/mentorship/actix-web.md@v1
  - .agentic/mentorship/templating.md@v1
resolutions:
  - RR-0002
  - RR-0008
---
```

> Check paths updated by MB-0019 (RR-0009): P5's test goes in the `routes::lists` test area `sharing_invites`, beside the other share and revoke tests; this mission may pick another area and update the check.

## Context

The app has no CSRF protection. The session cookie's `SameSite=Lax` is the
only mitigation (CRP-0008 → Current state). That leaves gaps:

- same-site attackers, such as another app on a sibling subdomain of a
  self-hosted install;
- browsers without `SameSite` support;
- any future move away from `Lax`.

`security.md` asks for protection "before any deployment that is reachable
outside localhost". MB-0008 (list sharing) raises the stakes: a forged post
can now share a list with an attacker's account, or let an edit grantee's
browser be driven against someone else's list.

The owner decided on 2026-11-06 to ship MB-0008 with no CSRF change. P8
(origin check on share and revoke) moves here, and this brief sets up
production-ready protection app-wide instead (RR-0008, closing CRP-0008).

CRP-0008 laid out the candidates:

- **Origin check** (`Origin`, falling back to `Referer`):
  - no template changes, and htmx sends `Origin` too;
  - but it relies on those headers and on knowing the app's own origin. The
    `Host` header is wrong behind a proxy that rewrites it, so a configured
    public origin is likely needed.
- **Synchronizer token in the session:**
  - doesn't depend on headers;
  - but it touches every form and the layout (`hx-headers` on `<body>`), and
    every route test.
- **Both**, as defence in depth.

## Conceptual plan

1. **Decide the mechanism.** Before any code, open a CRP for the security
   owner. It covers:
   - the mechanism;
   - how the app learns its public origin (`KAUPPA_PUBLIC_ORIGIN`, a
     config change);
   - whether `security.md`'s CSRF rule becomes a "must".
   The RR rolls back into `security.md`.
2. **One control for all writes.** Put it in one place, covering every
   non-safe method, so no handler has to remember it. The P4 guard enumerates
   the registered routes and fails on an uncovered one.
3. **Tests send proof by default.** Route tests send valid proof through
   `test_support`. Then the P1/P3 tests strip or forge it, which keeps the
   existing suite meaningful.

Checkpoints: CRP answered → RR approved → P4 guard red → protection → P1–P3,
P5, P6 green → smoke against the built binary, direct and behind a local
reverse proxy.

Slow-mode triggers:
- any route turns out to need an exemption, such as a future webhook;
- login and register can't carry the proof without JavaScript;
- the mechanism needs a dependency.

## Validation plan

- **P1, P3:** for every registered non-safe route, a request with forged
  proof and one with no proof each return 403. A repo read afterwards shows
  nothing changed.
- **P2:**
  - each form's real submission (taken from the rendered page) succeeds;
  - the htmx variants succeed;
  - with a configured public origin, a request whose `Host` differs from it
    still succeeds when its proof matches.
- **P4:** the guard compares the route table with the protected set.
- **P5:** MB-0008's test name, in `routes::lists`.
- **P6:** captured `tracing` output asserts the fields and the absence of
  secrets.
- **Smoke:** curl with and without `Origin` (and the token, if chosen) against
  the built binary.
