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
  An `edit` grant may match the owner's favorites by name (RR-0024).
  Never trust a client-supplied `owner_id` or role.
- **Input.** Deserialize into typed structs; reject on invalid. For HTML
  rendering, handlebars escapes by default — never use `{{{raw}}}` on
  user-provided content.
- **Logging.** Never log passwords, hashes, session cookies, or full emails
  in error messages at `info` or higher. Request log spans record the path
  only, never the query string (RR-0012).
- **Registration.** Accounts are created only by consuming a valid
  invitation in the same transaction (`InvitationRepo::register`; MB-0023,
  RR-0012). Handlers never call `UserRepo::create`. Invitation tokens are 256
  bits from `OsRng`, stored only as hex SHA-256, and never logged.
- **Password length.** New passwords are 15 to 128 Unicode scalar
  values, counted on the string exactly as submitted (no
  normalization, no trimming), checked on the server by the
  `kauppa-core` domain rule (`PASSWORD_MIN_CHARS`,
  `PASSWORD_MAX_CHARS`) after the token-shape check and before
  `password::hash` (MB-0034, RR-0022). Forms take `minlength` from
  the same constant and set no `maxlength`. Login never applies the
  rule. The password and its length are never logged or echoed.
- **Admin.** Admin status lives in the `admins` table and is checked from
  the database on every `/admin` request, never cached in the session. It is
  granted from the host (`kauppa-web admin grant`), never by a web request
  without an existing admin (RR-0012).
- **Cross-site requests.** Every non-safe request passes the app-wide
  cross-site check in one place (`Origin`/`Referer` against the app's
  origin, with `Sec-Fetch-Site` as a reject-only layer; MB-0018, RR-0010).
  No route is exempt without an approved RR. Safe methods stay side-effect
  free.
- **Rate limiting.** Failed `POST /auth/login` attempts are limited per
  client and per account, and failed `POST /auth/register` attempts per
  client, before any database or Argon2 work (MB-0026, RR-0013). The limits
  are constants named in RR-0013.
- **Client addresses.** Client addresses for security decisions come from
  the TCP peer, or from `X-Forwarded-For` only when the peer is in
  `KAUPPA_TRUSTED_PROXIES` (right-most untrusted entry; RR-0013); never from
  `realip_remote_addr()`.
- **Deployment secrets.** No secret value passes through Terraform
  variables, resources, data sources, `user_data` or plan files. On the
  host, secrets live only in 0600 files under the data volume's `secrets/`
  directory; the session secret is generated there and never leaves it.
  The container refuses the dev session secret (MB-0022, RR-0014).

## Rules (should)

- Rotate `KAUPPA_SESSION_SECRET` on compromise; invalidates all sessions.

## Escalation

Any change to:

- `crates/kauppa-core/src/password.rs`
- `crates/kauppa-web/src/routes/auth/`
- `crates/kauppa-web/src/routes/admin/` and `crates/kauppa-web/src/admin/`
  (admin portal and host bootstrap, RR-0012)
- Session middleware configuration in `main.rs`
- Cookie flags in `config.rs`

requires a Consultation Request Pack (`CRP-*`) and an approved Resolution
Record (`RR-*`) before merge.
