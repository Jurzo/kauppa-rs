# Mentorship Pack — security

Version: v1 · Scope: auth paths, session middleware, cookies, secrets.

## Threat model (abridged)

Single tenant per user, trust boundary at the HTTP front door. Primary risks:

1. Credential stuffing / weak passwords.
2. Session hijacking.
3. Horizontal privilege escalation (reading another user's lists/recipes).
4. CSRF on state-changing endpoints.
5. Data loss via accidental SQL or migration mistakes.

## Rules (must)

- **Password hashing.** `kauppa_core::password::{hash, verify}` only. Never
  call Argon2 directly from a handler. Changing the algorithm or parameters is
  a Resolution-Record-required change.
- **Session store.** `actix-session` with `CookieSessionStore`, keyed by a
  >=64-byte secret from `KAUPPA_SESSION_SECRET`. Cookies: `HttpOnly`,
  `SameSite=Lax`, `Secure` behind TLS (`KAUPPA_COOKIE_SECURE=true`).
- **Authorization.** Every per-user resource query authorizes the acting user
  against the resource. For owner-only resources that means `owner_id =
  $current_user`. For shared shopping lists (RR-0002) the predicate is "owner
  OR a `list_shares` grantee with sufficient role (`view` < `edit`)", computed
  by a single centralized helper that every list / list-item read and write
  goes through — a query that filters on `owner_id` alone is a review failure.
  Never trust a client-supplied `owner_id` or role.
- **Input.** Deserialize into typed structs; reject on invalid. For HTML
  rendering, handlebars escapes by default — never use `{{{raw}}}` on
  user-provided content.
- **Logging.** Never log passwords, hashes, session cookies, or full emails
  in error messages at `info` or higher.

## Rules (should)

- Add CSRF protection (double-submit cookie or origin check) before any
  deployment that is reachable outside localhost.
- Rotate `KAUPPA_SESSION_SECRET` on compromise; invalidates all sessions.
- Rate-limit `/auth/login` and `/auth/register` (per-IP + per-account).

## Escalation

Any change to:

- `crates/kauppa-core/src/password.rs`
- `crates/kauppa-web/src/routes/auth.rs`
- Session middleware configuration in `main.rs`
- Cookie flags in `config.rs`

requires a Consultation Request Pack (`CRP-*`) and an approved Resolution
Record (`RR-*`) before merge.
