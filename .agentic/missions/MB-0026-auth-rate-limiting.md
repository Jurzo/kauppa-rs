# Mission Brief — Rate-limit login and registration

```yaml
---
id: MB-0026
owner: "@juuso-kalliomaki"
goal: "Slow down credential stuffing and password guessing on a public deployment (`security.md` threat 1): repeated failed logins per client and per account, and repeated registration attempts per client, are throttled with a 429, without locking out a legitimate user for long."
non_goals:
  - "Account lockout that needs an admin to undo."
  - CAPTCHA, two-factor authentication or email verification.
  - "Rate limiting other routes (later, if needed)."
  - "Shared or persistent limiter state across processes or restarts: the app is one process (MB-0022)."
acceptance:
  - id: P1
    property: "After N failed logins from one client within the window, further `POST /auth/login` from that client get 429 with a `Retry-After` header until the window passes, even with the correct password. N and the window are constants named in RR-0013."
    check: cargo test -p kauppa-web routes::auth::tests::rate_limit::per_client_login
  - id: P2
    property: "After M failed logins for one account within the window, from any mix of clients, further attempts for that account are throttled. The response is the same whether the account exists or not, so the limiter isn't an account oracle."
    check: cargo test -p kauppa-web routes::auth::tests::rate_limit::per_account_login
  - id: P3
    property: "`POST /auth/register` is limited per client, so invitation tokens (MB-0023) can't be guessed at speed."
    check: cargo test -p kauppa-web routes::auth::tests::rate_limit::per_client_register
  - id: P4
    property: "A successful login doesn't count toward the limits, and limiter memory is bounded: entries expire, and the map can't grow without limit under a flood of distinct clients or emails."
    check: cargo test -p kauppa-web routes::auth::tests::rate_limit::bounded_and_expiring
  - id: P5
    property: "The client address comes from the connection peer unless a trusted-proxy setting says otherwise. A forwarded-for header from an untrusted peer is ignored, so a client can't pick its own key."
    check: cargo test -p kauppa-web routes::auth::tests::rate_limit::forwarded_header_needs_trusted_proxy
  - id: P6
    property: "Throttling is logged at WARN with the reason and limiter key kind (client or account), never the email or password; no other log level or line changes, except the lines RR-0013 approves."
    check: cargo test -p kauppa-web routes::auth::tests::rate_limit::throttling_is_logged
context:
  - crates/kauppa-web/src/routes/auth.rs
  - crates/kauppa-web/src/main.rs
  - crates/kauppa-web/src/config.rs
  - crates/kauppa-web/src/csrf/mod.rs
  - .agentic/mentorship/security.md
  - .agentic/resolutions/RR-0010-csrf-mechanism.md
autonomy:
  may_decide:
    - "the limiter's data structure (e.g. a fixed-window or token-bucket map behind a mutex), if it has no dependencies"
    - test names inside the paths above
  escalate:
    - { when: "the limits N and M, the window and the backoff", to: security }
    - { when: "reading `X-Forwarded-For` behind Caddy and how a trusted proxy is configured (RR-0010 never reads `X-Forwarded-*`; this would be the first exception)", to: security }
    - { when: "a dependency (e.g. `actix-governor`)", to: platform-owner }
    - { when: "limiting in Caddy instead of the app (custom Caddy build, MB-0022)", to: security }
    - { when: "a new configuration variable", to: platform-owner }
  forbidden:
    - "change password hashing, session middleware or cookie settings"
    - log emails, passwords or full client addresses at INFO or above
    - "make the response differ between existing and unknown accounts"
    - lower lint strictness
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/security.md@v1
  - .agentic/mentorship/actix-web.md@v1
resolutions:
  - RR-0013
---
```

## Context

`security.md` → Rules (should) asks to "rate-limit `/auth/login` and
`/auth/register` (per-IP + per-account)". Nothing does today, and Argon2id
verification is the only cost per guess. That was fine on localhost; it
isn't on a public URL (MB-0022).

Behind Caddy, every request's peer address is Caddy's, so a per-client
limit needs the forwarded client address. Reading `X-Forwarded-For` safely
means trusting it only from a configured proxy; RR-0010 deliberately never
reads forwarded headers for the cross-site check. The CRP should decide how
these two fit together.

`routes/auth.rs` is a security trigger path: this mission starts with a CRP
and needs an approved RR.

## Conceptual plan

1. **Clarify (S1).** One CRP to security: limits, client identification
   behind a proxy, in-app versus Caddy. Stop until the RR is approved.
2. A small in-process limiter in `kauppa-web`, consulted by the login and
   register handlers before password verification.
3. Route tests drive the limiter with a fake clock.

Slow-mode trigger: the limiter needs to wrap routes outside `/auth`.

## Validation plan

- P1–P6 in `routes::auth::tests::rate_limit`, with a fake clock and
  `LogCapture`.
- Smoke: on the built binary behind a local Caddy, script N+1 failed logins
  and see 429 with `Retry-After`; then wait out the window and log in.
