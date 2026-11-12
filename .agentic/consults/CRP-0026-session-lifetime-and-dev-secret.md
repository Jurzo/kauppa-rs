# Consultation Request Pack — Session lifetime and refusing the dev session secret

```yaml
---
id: CRP-0026
mission: MB-0042
owner: "@juuso-kalliomaki"
target_role: security
risk_tier: medium
urgency: medium
state: answered
runbook_trigger: "feature.md security trigger: session middleware configuration in `main.rs`; MB-0042 autonomy.escalate (lifetimes, dev-secret rule → security)"
---
```

## Decision statement

Decide how long a sign-in lasts and how that is enforced, and when the app
may start with the public development session secret.

## Current state

- `main.rs` wraps `IdentityMiddleware::default()` and a `SessionMiddleware`
  over `CookieSessionStore` with no lifecycle setting.
- actix-session 0.10's cookie store ignores the state TTL
  (`storage/cookie.rs`: `_ttl`), and the default lifecycle is a browser
  session, so the cookie has no `Max-Age` and its contents never expire.
- actix-identity 0.8 defaults `login_deadline` and `visit_deadline` to
  `None` (`config.rs:21-22`).
- Result: a copied `kauppa_sid` works until `KAUPPA_SESSION_SECRET` is
  rotated. Sign-out clears only the browser's copy.
- `config.rs` defaults `session_secret` to the `dev-only-…` value that is
  also in `.env.example`. `deploy/docker-entrypoint.sh` refuses it; the
  binary run directly doesn't.

## Options and trade-offs

### Q1 — Mechanism

**A (recommended): actix-identity's deadlines.**
`IdentityMiddleware::builder().login_deadline(…).visit_deadline(…)`.

- Pros:
  - No new dependency, table or code path: two builder calls in `main.rs`,
    mirrored in `wired_app!`.
  - Enforced on the server: the sign-in time and last-visit time live
    inside the encrypted cookie, and the middleware signs the user out when
    either is too old (`middleware.rs:151-170`).
- Cons:
  - No revocation: a stolen cookie works until a deadline passes, and
    using it keeps the idle deadline alive. The absolute limit bounds that.
  - With a visit deadline the cookie is re-issued on every request.
  - The middleware reads the real clock, so tests need short deadlines in
    the test app rather than a fake clock.
- Risk: low.
- Cost: small.

**B: server-side sessions (a `sessions` table).**

- Pros: real revocation and "sign out everywhere".
- Cons: a migration, a custom session store, a database write per request,
  cleanup of old rows.
- Risk: medium. Cost: large.

**C: a persistent cookie with `Max-Age` only.**

- Not a fix: the browser drops the cookie, but a copy still works forever.

### Q2 — Limits

- **Recommended: 30 days idle, 90 days absolute**, as constants in code
  (like RR-0013's limits). A household app used weekly rarely hits the idle
  limit; everyone signs in again once a quarter.
- Stricter alternative: 7 days idle, 30 days absolute.

### Q3 — Cookie lifetime in the browser

- **(a) (recommended): no change.** The cookie stays a browser-session
  cookie; the deadlines are the server-side limit.
- (b) A persistent cookie (`Max-Age` = the idle limit), so a sign-in
  survives closing a desktop browser. A convenience change to cookie
  settings; can be a later brief.

### Q4 — The dev secret

- **Recommended:** `Settings::load` fails when the secret equals the
  compiled-in default unless the bind address is loopback **and**
  `KAUPPA_COOKIE_SECURE=false`. The error names `KAUPPA_SESSION_SECRET` and
  how to generate one.
  - Local `cargo run` with `.env.example` keeps working.
  - Tests are unaffected: `wired_app!` uses its own key, and the one test
    that loads settings overrides the secret.
  - The container entrypoint's own refusal stays (defence in depth).
- Alternative: refuse it always, and make local development generate a
  secret. Safer by default, but a worse first run (AGENTS.md goal 3).

### Q5 — Existing sessions

Sessions created before the change carry no sign-in timestamp, and the
middleware signs those out (`middleware.rs:183-189`). **Everyone signs in
once after the deploy.** Accept, or keep them with custom code (not
recommended).

## Recommendation and question

Recommended: Q1 A, Q2 30/90 days as constants, Q3 (a), Q4 loopback and
non-secure only, Q5 accept one sign-out at deploy. No "sign out
everywhere" in this mission; open a brief for option B if wanted.

Crisp questions:

1. Mechanism: actix-identity deadlines (A)?
2. Limits: 30 days idle and 90 days absolute, as constants?
3. Keep the browser-session cookie (a)?
4. Dev secret: allowed only on a loopback bind with non-secure cookies?
5. Accept that every existing session ends at the first deploy?

## Blast radius and rollback

What can break:
- Users are signed out earlier than they expect (limits too short).
- A self-hoster running the bare binary with the dev secret on a public
  bind gets a startup error after upgrading. That is the intended effect.

Who is affected: every signed-in user once at deploy; self-hosters without
a secret.

Rollback plan: revert the PR. Sessions issued meanwhile carry timestamps
the old code ignores.

## Minimal context

- MB-0042 P1–P4. `security.md` → Session store rule gains the lifetime and
  the dev-secret rule (roll-back).
- Touches `main.rs` (session middleware block: one builder replaces
  `IdentityMiddleware::default()`), `test_support.rs`, `config.rs`.

## Evidence bundle (Hardened)

- actix-session 0.10.x `src/storage/cookie.rs`: `load`, `save` and
  `update_ttl` take `_ttl` and ignore it; `src/config.rs`: default
  `BrowserSession`, `SameSite::Lax`, `CookieContentSecurity::Private`.
- actix-identity 0.8.0 `src/config.rs:21-22,102-118` (deadlines default
  `None`); `src/middleware.rs:151-170` (logout on a passed deadline),
  `178-250` (missing timestamp → `LogOut`; `OffsetDateTime::now_utc`).
- `crates/kauppa-web/src/config.rs` (default secret; the 64-byte length
  check is the only validation); `scripts/deploy/smoke_image.sh:157-158`
  (the container refuses a missing or dev secret).
