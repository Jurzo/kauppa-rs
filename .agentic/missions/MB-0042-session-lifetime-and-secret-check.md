# Mission Brief — Sessions expire, and the dev session secret is refused

```yaml
---
id: MB-0042
owner: "@juuso-kalliomaki"
goal: "A sign-in stops working after a bounded time, and the app refuses to serve with the publicly known development session secret outside local development, so a copied session cookie or a missing `KAUPPA_SESSION_SECRET` can't give lasting access."
non_goals:
  - "Server-side session storage or a \"sign out everywhere\" feature (the RR may open a follow-up)."
  - "Password change, password reset or account deletion."
  - "Changing the cookie name, `SameSite`, `HttpOnly` or `Secure` behaviour."
acceptance:
  - id: P1
    property: "A session is rejected after the lifetime the RR sets (an absolute limit since sign-in, and an idle limit if the RR sets one), enforced by the server and not only by the cookie's expiry: a request with an older session cookie is treated as signed out."
    check: "cargo test -p kauppa-web routes::auth::tests::session_lifetime::expired_session_is_signed_out"
  - id: P2
    property: "A session inside its lifetime keeps working, and normal use extends an idle limit as the RR decides."
    check: "cargo test -p kauppa-web routes::auth::tests::session_lifetime::active_session_stays_signed_in"
  - id: P3
    property: "With the development session secret, the app starts only in the cases the RR allows (for example a loopback bind with `KAUPPA_COOKIE_SECURE=false`); otherwise it exits with an error naming `KAUPPA_SESSION_SECRET`, before binding."
    check: "cargo test -p kauppa-web config::tests::dev_secret_refused_outside_dev"
  - id: P4
    property: "`security.md` states the session lifetime and the dev-secret rule (RR roll-back), and the test app is wired like `main.rs`."
    check: "cargo test -p kauppa-web --all-targets"
context:
  - crates/kauppa-web/src/main.rs
  - crates/kauppa-web/src/config.rs
  - crates/kauppa-web/src/routes/test_support.rs
  - crates/kauppa-web/src/routes/auth/mod.rs
  - deploy/docker-entrypoint.sh
  - .agentic/mentorship/security.md
autonomy:
  may_decide:
    - "test names and file layout under `routes/auth/tests/`"
    - "how tests move time (a clock, or deadlines shortened through configuration in tests only)"
  escalate:
    - { when: "the lifetimes (absolute and idle), and whether they are constants or settings", to: security }
    - { when: "any change to the session middleware block in `main.rs` or to cookie settings (security trigger path)", to: security }
    - { when: "which combinations may run with the dev secret, and whether that changes local development or the tests", to: security }
    - { when: "server-side sessions or a per-user session version, if the RR wants revocation", to: security }
  forbidden:
    - "weaken any cookie flag"
    - "log the session secret or a session cookie"
    - "a new dependency without the dependency-bump runbook"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/security.md@v1
  - .agentic/mentorship/actix-web.md@v1
resolutions: [RR-0026]
---
```

## Context

Found in the audit of 2026-11-10 (findings 2 and 3).

`main.rs` builds `SessionMiddleware` with `CookieSessionStore` and no
lifetime, and `IdentityMiddleware::default()`. actix-session's cookie store
ignores the state TTL (`_ttl` in `storage/cookie.rs`), the default session is
a browser session, and actix-identity 0.8 defaults `login_deadline` and
`visit_deadline` to `None`. So a `kauppa_sid` cookie copied from a device
stays valid until `KAUPPA_SESSION_SECRET` is rotated, and sign-out only
clears the browser's copy. `security.md` lists session hijacking as a primary
risk but sets no lifetime.

`config.rs` falls back to the `dev-only-…` secret (also in `.env.example`)
when the variable is unset. `deploy/docker-entrypoint.sh` refuses it
(RR-0014), but the binary run directly does not, and anyone who knows the
secret can forge a session for any user id.

## Conceptual plan

1. **Clarify (S1).** CRP to security: lifetimes, mechanism
   (actix-identity deadlines are the smallest change), and the dev-secret
   rule. Answered by RR-0026.
2. Tests first, then the middleware configuration in `main.rs` mirrored in
   `wired_app!`, and the startup check in `Settings::load`.
3. RR roll-back into `security.md`; `.env.example` and `deploy/README.md`
   notes.

## Validation plan

- P1, P2: route tests with a session older and newer than the limits.
- P3: unit tests over `Settings` for each combination the RR names.
- Smoke: the release binary with and without `KAUPPA_SESSION_SECRET`.
