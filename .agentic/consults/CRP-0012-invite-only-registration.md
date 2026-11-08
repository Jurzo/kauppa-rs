# Consultation Request Pack — Invite-only registration: schema, bootstrap and token handling (MB-0023)

```yaml
---
id: CRP-0012
mission: MB-0023
owner: "@juuso-kalliomaki"
target_role: security
risk_tier: high
urgency: high
state: open
runbook_trigger: "MB-0023 autonomy.escalate: schema shape (data-owner; migration.md entry gate 2, a CRP before any file in `migrations/`); first-admin bootstrap (security); email-bound vs bearer invitations (security); token form in the link and as a hidden field (security); a dependency for hashing (platform-owner, sub-question Q6). feature.md escalation paths `migrations/**`, `crates/kauppa-web/src/routes/auth.rs` and `main.rs`. Routed to security, with Q1 and Q2 also to data-owner and Q6 to platform."
---
```

## Decision statement

Decide how MB-0023 closes open sign-up. That means four things:

- the schema for admins and invitations, including the migration's exact
  filename, which the RR must name;
- how the first administrator is set up from the host;
- whether an invitation is tied to an email address;
- how the token is generated, carried, stored and kept out of logs.

**Routing.** The CRP goes to security. Q1–Q2 also need data-owner sign-off
and Q6 platform-owner sign-off. One RR can cover all three roles, as RR-0010
did.

## Current state

Verified on `63da727` (the accepted briefs, on top of `main` `e63b4f1`).
No code was changed. The only build was a release run of `password`'s test
to time Argon2id (Evidence bundle).

**Registration** (`crates/kauppa-web/src/routes/auth.rs`):

- `register_submit` (`auth.rs:79-97`) hashes the password with Argon2id
  (`auth.rs:85`), then calls `UserRepo::create` (`auth.rs:86-89`), then
  `Identity::login`. There is no invitation, rate limit or validation beyond
  serde.
- **Email oracle today.** A duplicate email gives `CoreError::Conflict("email
  already registered: {email}")` (`kauppa-core/src/repo/sqlite/user.rs:67-69`).
  `WebError::error_response` returns that message as a plain 409 body
  (`error.rs:49-50`), so anyone can test whether an address has an account.
- **A full email in the logs.** The same handler logs `error = ?self` at WARN
  (`error.rs:44-45`), so the full email goes to the log. `security.md` →
  Logging forbids that at INFO and above.
- The register page links to login. The login page (`templates/auth/login.hbs:12`)
  and the home page (`templates/index.hbs:7`, "Create an account") link to
  `/auth/register`.

**Login** (`auth.rs:39-60`). An unknown email returns before
`password::verify` (`auth.rs:46-48`). The response is identical, but the
timing differs by one Argon2id verification. That is MB-0026's concern
(CRP-0013 Q7); it's noted here because the register path has the same shape.

**Users** (`migrations/0001_init.sql:5-11`):

- `users(id, email UNIQUE, display_name, password_hash, created_at)`,
  `STRICT`, with no role column.
- `email` uses the default `BINARY` collation, and `find_by_email` compares
  exactly (`user.rs:18-22`). So `Pat@x.fi` and `pat@x.fi` are two accounts.
- `UserRepo` has just `find_by_email` and `create` (`repo/mod.rs:35-44`).

**Migrations.**

- The last is `migrations/0007_list_shares.sql`.
- No other open branch (`agentic/mb-0022-deploy`, `-0024-embed-files`,
  `-0025-sqlite-wal`) adds one.
- So the next is **`0008`**.

**Request pipeline** (`main.rs:68-85`, `routes/mod.rs:19-25`):

- `TracingLogger` → identity → cookie session → `/assets` → one catch-all
  scope wrapped in `csrf::guard`. Every handler must be registered in
  `routes::protected`, and `csrf::tests::coverage` checks that.
- `main.rs` takes no arguments today (`main.rs:31-42`). It loads settings,
  connects, migrates and serves.
- **Where the URL gets logged:**
  - tracing-actix-web 0.7.25's root span records
    `http.target = path_and_query` at INFO
    (`tracing-actix-web-0.7.25/src/root_span_macro.rs:110`).
  - Both formatters in `telemetry.rs` print the current span's fields with
    every event: `compact()` in debug builds, `json()` in release.
  - So a token anywhere in the URL, path or query, appears in every log line
    emitted during that request, including WARN errors.
  - `csrf::guard` logs `path = req.path()` on rejection (`csrf/mod.rs:198-205`),
    without the query.
- **Anonymous requests.** There is no redirect-to-login helper.
  `request::current_user` gives `Unauthorized`, which renders a plain 401
  (`routes/request.rs:5-11`, `error.rs:31`). P3's "anonymous → redirect to
  login" is new behaviour for `/admin`.
- **Third-party script.** `templates/layout.hbs:10` loads htmx from
  `unpkg.com`. No `Referrer-Policy` is set anywhere, so browsers use their
  default, `strict-origin-when-cross-origin`: unpkg receives only the origin,
  never the path or query.

**Randomness and hashing in the tree** (`cargo tree`):

- **OS RNG.** `kauppa-core` already uses `argon2::password_hash::rand_core::OsRng`
  (`password.rs:9,15`). That is rand_core 0.6.4 backed by getrandom 0.2,
  re-exported by password-hash 0.5.0 (`src/lib.rs:29`) through `argon2` 0.5.3
  (`src/lib.rs:111-114`).
  - `OsRng.fill_bytes(&mut [u8; 32])` gives 256 bits from the OS, with no
    manifest change.
  - `uuid` v4 (getrandom 0.4) gives only 122 random bits per value, short of
    P2's 128.
- **SHA-256.** `sha2` 0.10.9 and `hex` 0.4.3 are in `Cargo.lock`, but only
  through `sqlx-core` and `sqlx-macros-core`, so neither is a direct
  dependency.
  - `blake2` is used inside `argon2` but not re-exported.
  - No SHA-256 or BLAKE2 function is reachable from our code without a
    manifest change.
- **Not in the tree:** `clap` and any line-editing or password-prompt crate.

**SQLite details that affect the design:**

- **Timestamp text varies.** sqlx encodes `DateTime<Utc>` with
  `to_rfc3339_opts(SecondsFormat::AutoSi, false)`
  (`sqlx-sqlite-0.8.6/src/types/chrono.rs:69`). The number of fraction digits
  depends on the value: `…:00+00:00` vs `…:00.5+00:00`.
  - Text comparison in SQL (`expires_at > ?`) is therefore wrong for values
    that differ only in the fraction: `.` sorts before `+`.
  - Timestamps the comparison depends on must be truncated to whole seconds
    before binding, or compared in Rust.
- **Busy timeout.** sqlx's default `busy_timeout` is 5 s
  (`sqlx-sqlite-0.8.6/src/options/mod.rs:201`). A second concurrent
  registration transaction waits for the first rather than failing, so P2's
  "one winner" comes from the conditional `UPDATE`, not from a lock error.
  MB-0025 (WAL) doesn't change this.

**Tests that the change touches:**

- `routes/test_support.rs:72-78` `make_user` creates users with
  `UserRepo::create` directly. About 36 `test_app!` users depend on it and
  none of them go through registration, so they're unaffected as long as
  `UserRepo::create` stays.
- **`csrf/tests/same_origin.rs:58-83` `browse` will break.** It registers
  through the rendered `GET /auth/register` form and expects 303. It needs an
  invitation created through the repo, the page fetched with the invite link,
  and the hidden field posted. The test's purpose stays the same.
- **`csrf/tests/mod.rs:312-317` must change too.** `World::form` posts
  `/auth/register` as `NEWCOMER`, and `snapshot` (`mod.rs:351`) checks that
  no newcomer appears. Once registration needs a token, that test passes
  without proving anything unless `World` holds a valid invitation and the
  form sends it. The forged request must be refused by the cross-site check,
  not by the missing token.
  - `World::uri` maps an `admin` scope to `Uuid::nil()`. The revoke route
    needs a real invitation id there, and `snapshot` should include the
    `invitations` table.
- **No tests module for `routes::auth` yet.** P1's check
  (`routes::auth::tests::register_requires_invite`) implies `auth.rs` →
  `auth/mod.rs` + `auth/tests.rs`, or an inline module, under RR-0009's
  layout rules. MB-0026 adds `routes::auth::tests::rate_limit` beside it.

## Questions

Each question has options, a recommendation, and the trade-offs. The crisp
questions are collected at the end.

### Q1 — Schema for admin status (data-owner)

#### Option A — `users.is_admin` column

```sql
ALTER TABLE users ADD COLUMN is_admin INTEGER NOT NULL DEFAULT 0
    CHECK (is_admin IN (0, 1));
```

- Pros:
  - one column;
  - the admin check is part of a user lookup;
  - SQLite accepts `ADD COLUMN … NOT NULL DEFAULT <constant>` on a `STRICT`
    table, and every existing row reads 0 (P6).
- Cons:
  - it alters the credential table;
  - every `SELECT` that builds a `User` may need the column;
  - it records nothing about when or how admin was granted, which P7's audit
    then has to carry in the logs alone;
  - removing admin later is an `UPDATE` on `users`.
- Risk: low. `ALTER TABLE ADD COLUMN` is additive, with no backfill and no
  table rebuild.

#### Option B — `admins` table (recommended)

```sql
CREATE TABLE admins (
    user_id    TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    granted_at TEXT NOT NULL,
    granted_by TEXT REFERENCES users(id) ON DELETE SET NULL  -- NULL: host command
) STRICT;
```

- Pros:
  - **Purely additive.** `users` and every existing query are untouched, and
    P6 holds trivially because the table starts empty.
  - **A cheap per-request check.** P3's check is a primary-key lookup,
    `SELECT 1 FROM admins WHERE user_id = ?`, run by the `/admin` guard only.
  - **A record of the grant.** It says when admin was granted and whether
    from the host (`granted_by IS NULL`) or by another admin (a later brief).
  - **Simple revocation later.** Taking admin away is deleting the row, the
    same as list shares (`0007`).
- Cons:
  - one more table;
  - `granted_by` diverges from `ON DELETE CASCADE` (SET NULL, so deleting the
    granting admin doesn't silently demote others) and needs an RR line in
    the migration header.
- Risk: low.

#### Option C — General `roles` / `user_roles` tables

- Pros: room for more roles.
- Cons: "more roles than admin and regular user" is a non-goal; it adds a
  lookup table and a join for one bit.
- Risk: low, but it's scope creep. Not recommended.

**Recommendation: B.**

### Q2 — The `invitations` table and the migration file (data-owner)

Proposed file: **`migrations/0008_invitations.sql`**. It contains both the
`admins` table (Q1 B) and `invitations`. With Q1 A it would carry the
`ALTER TABLE` instead and could be named `0008_invitations_and_admins.sql`.
The RR names whichever is chosen.

```sql
CREATE TABLE invitations (
    id         TEXT PRIMARY KEY,               -- UUID v4; in portal URLs and logs
    token_hash TEXT NOT NULL UNIQUE,           -- hex SHA-256 of the 32-byte token (Q6)
    note       TEXT,                           -- optional, admin-written, length-capped in the handler
    created_by TEXT REFERENCES users(id) ON DELETE SET NULL,  -- NULL: host command (Q3)
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    revoked_at TEXT,
    used_at    TEXT,
    used_by    TEXT REFERENCES users(id) ON DELETE SET NULL,
    CHECK (revoked_at IS NULL OR used_at IS NULL)
) STRICT;

CREATE INDEX idx_invitations_created ON invitations(created_at DESC);
```

**Columns:**

- **State is derived, not stored.** Pending = not used, not revoked, and not
  expired. Used, revoked and expired follow from the timestamps. That avoids
  a `state` column that could disagree with them.
- **No email column** under Q4's recommendation. With Q4 B, add
  `email TEXT NOT NULL` (normalized, see Q4).
- **The note** is free text the admin writes ("for Mum"). It's shown only in
  the portal, never logged. P7 logs the invitation id.
- **No `CHECK` ties `used_by` to `used_at`.** With `ON DELETE SET NULL` on
  `used_by`, deleting that user would violate such a check, and the delete
  would fail.

**Foreign keys.** Both are `SET NULL`, a divergence from `persistence.md`'s
CASCADE default:

- An invitation is an audit record. Deleting the admin who created it, or the
  user who used it, should leave it showing "used" or "revoked", not remove
  it.
- No user deletion exists yet, so this is about future behaviour. The header
  names the RR, as `0003` does for RR-0001.
- **Alternative:** CASCADE on both. That's simpler and default-conforming,
  but deleting an admin would also delete their pending invitations. That is
  arguably a feature (an implicit revoke), but it loses history.

**Indexes:**

- `token_hash UNIQUE` serves the consume lookup.
- `created_at DESC` serves the portal list. The table will hold tens of rows,
  so the index is optional; it's included for parity with `0001`.
- None on `used_by` or `created_by`. Nothing queries by them, and the
  `SET NULL` cascade on a tiny table doesn't need one.

**Timestamps.**

- Every invitation timestamp is truncated to whole seconds before binding, so
  sqlx's `AutoSi` text has a fixed width and compares correctly in SQL (see
  Current state). That's a repo-layer rule, written in the migration header,
  with a test.
- The alternative is comparing expiry in Rust, inside the transaction. That
  works too, but leaves the conditional `UPDATE` with only part of the guard.

**Consuming the token (P2).** One repo method,
`InvitationRepo::register(token_hash, email, display_name, password_hash, now)`,
owns the transaction:

1. `INSERT INTO users …`. A unique violation means a duplicate email: roll
   back, and the token is still usable.
2. `UPDATE invitations SET used_at = ?now, used_by = ?uid WHERE token_hash = ?
   AND used_at IS NULL AND revoked_at IS NULL AND expires_at > ?now`.
3. Rows affected ≠ 1 → roll back the user insert → the one generic
   "invalid invitation" error (P1).
4. Commit.

The user row goes first because `used_by` references it. Two concurrent
registrations with one token:

- the second transaction waits on the write lock (5 s busy timeout);
- its `UPDATE` then matches no row;
- it rolls back. One account is created.

**Backfill and compatibility.**

- Nothing is backfilled. Existing users keep working, and none becomes an
  admin (P6).
- The old binary ignores the new tables, so rolling back the binary is safe.

**Recommendation:**

- `0008_invitations.sql`, with `admins` and `invitations` as above;
- SET NULL foreign keys with the reason in the header;
- whole-second timestamps.

### Q3 — First-admin bootstrap (security; `main.rs` is a trigger path)

On a fresh install there are no users, and registration needs an invitation
that only an admin can make. Bootstrap has to break that loop from the host,
with no default credentials.

#### Option A — Host subcommands (recommended)

`kauppa-web admin <command>`, dispatched in `main.rs` before the server
starts. It parses `std::env::args` by hand (no `clap` in the tree). It uses
the same `Settings` and database, and runs migrations first.

- **`kauppa-web admin invite [--note TEXT]`**
  - Creates an ordinary invitation, with `created_by = NULL`.
  - Prints its link once to **stdout**, which is the operator's terminal
    under `docker compose exec`.
  - Logs only the invitation id at INFO (P7).
  - The link uses `KAUPPA_PUBLIC_ORIGIN` when set, else just the path.
- **`kauppa-web admin grant <email>`**
  - Makes an existing user an admin: `INSERT … ON CONFLICT DO NOTHING`, so
    running it again changes nothing (P5).
  - Logs the user id at INFO, not the email.
  - The same message comes whether the email is unknown or the user is
    already an admin. The operator is trusted, but the command still
    shouldn't print the email back into logs.
- **Fresh install:** `admin invite`, then register in the browser, then
  `admin grant <that email>`.
- **Existing install** (the owner's current accounts): `admin grant` only.

Assessment:

- Pros:
  - **Needs host access.** Only someone with shell access can do it, which is
    already root-equivalent for the SQLite file. There's no web path and no
    window in which a stranger can claim it.
  - **No new invitation type.** It reuses `InvitationRepo::create`, with no
    "grants admin" column.
  - **Idempotent and testable.** The grant is a plain function
    (`admin::bootstrap::grant`, P5's check).
  - **The token stays out of the log stream.** stdout of an `exec` session
    isn't the container's log stream.
- Cons:
  - **Two steps on a fresh install.**
  - **New argument handling in `main.rs`.** The server path stays untouched;
    session middleware isn't changed.
  - **`Settings::load` runs for the subcommand too,** so the session secret
    must be valid even though the command doesn't use it. That's harmless:
    the same env is present.
- Risk: low. A mis-parse could start the server instead of running the
  command, so unknown subcommands exit non-zero.

**Variant A′.** `admin invite --admin`: the invitation itself grants admin,
so a fresh install takes one step. It needs a `grants_admin INTEGER NOT NULL
DEFAULT 0` column and the consume path has to honour it. A leaked bootstrap
link would then create an admin rather than a user. Not recommended; the
second step is cheap.

#### Option B — `KAUPPA_ADMIN_EMAILS`

Users whose email is listed become admin at startup, or are checked per
request.

- Pros: declarative and survives rebuilds.
- Cons:
  - **It doesn't solve the first user.** Someone still needs an invitation,
    unless listed emails may register without one.
  - **An email squatting race.** If listed emails may register without an
    invitation, there's no email verification: whoever first registers the
    listed address becomes admin.
  - **Privilege lives in config.** Removing an email leaves the flag set
    unless startup also demotes, and then a typo demotes the owner.
  - It's a new config variable (platform).
- Risk: medium-high. Not recommended.

#### Option C — One-time bootstrap token printed at startup when no admin exists

- Pros: no subcommand, one step.
- Cons:
  - **It prints a credential to the log stream.** Under Docker the startup
    output is the container log: kept by `docker logs`, possibly shipped
    elsewhere. The brief forbids logging tokens.
  - **Anyone can claim it.** From startup until it's claimed, whoever reads
    the logs, or races the owner on the public URL, becomes admin.
  - **It runs on every restart** while no admin exists.
- Risk: high. Not recommended.

**Recommendation: A** (`admin invite` + `admin grant <email>`), with unknown
subcommands and arguments failing closed.

### Q4 — Email-bound or bearer invitations (security)

#### Option A — Bearer, with who used it recorded (recommended)

Whoever holds the link registers with any email. `used_by` records the
account, and the portal shows it. Issuance stays an admin decision: one link
makes one account, and the link expires, is single-use and can be revoked.

- Pros:
  - **The admin types no email**, so the invitation stores none and nothing
    can leak one into logs (P7).
  - **Binding adds little.** There's no email verification (non-goal: no
    email), so a link thief can register *as* the bound address anyway.
    Binding only stops them choosing a different address.
  - **Misuse shows up.** The portal shows which account used each invitation.
- Cons: a leaked link creates an account the admin didn't intend, until the
  invitation expires or is used. The mitigation is short expiry, revocation,
  and the "used by" display.
- Risk: medium, bounded by single use and expiry.

#### Option B — Bound to one email

The admin enters the invitee's email; registration must use it.

- **Comparison rule:** trimmed and ASCII-lowercased. `users.email` has
  `BINARY` collation today, so this brief would have to start normalizing on
  register too, or the two compares disagree.
- **The mismatch error must be the same generic error,** or the form tells a
  link holder which email the invitation is for.
- Pros: the account address is what the admin intended. A thief can't open
  an arbitrary account; they can only squat the invitee's address, and the
  invitee notices when their registration fails.
- Cons:
  - stores an email per invitation;
  - adds a normalization rule;
  - more UX friction (a typo in the invite means a new invite);
  - the security gain is small without verification.
- Risk: low-medium.

**Recommendation: A**, with the portal's list showing who used each
invitation (their display name and email; the portal is admin-only). B can
follow in its own brief when email verification exists.

**The duplicate-email oracle.** With a valid token, a duplicate email rolls
back and leaves the token usable, so the holder can try more addresses.

- That oracle is limited to invited people, and to MB-0026's register limit.
  Recommendation: accept it.
- But the message must render on the register page as a form error, not as
  today's plain 409 body. And it must stop logging the full address: fix the
  `Conflict` message in `user.rs:69`, which is in this mission's path, so
  P7's "never a full email" holds.
- **Stricter alternative:** one generic "couldn't create the account" for
  every failure. It hides the oracle entirely, but leaves an invitee stuck on
  a typo'd address.

### Q5 — The token in the link and in the form (security)

#### Option A — Query parameter + hidden field + no query strings in logs (recommended)

- **The link** is `https://<origin>/auth/register?invite=<token>`.
- **`GET /auth/register?invite=…`** renders the form with
  `<input type="hidden" name="invite">`. The form posts to `/auth/register`
  with no query, so the token travels in the body. After a failed attempt
  (duplicate email, short password) the page is re-rendered with the hidden
  field still set, so the invitee can correct the form and resubmit.
- **`GET /auth/register` with no or an empty `invite`** shows "registration
  needs an invitation" (P1).
  - The GET doesn't check the token: it renders the form for any value. A
    GET that answered "invalid" would be an oracle without rate limiting,
    since MB-0026 limits POST.
  - The POST gives the one generic error for missing, unknown, expired,
    revoked or used tokens (P1).
- **Keeping the token out of logs** (forbidden: log them). A custom
  `RootSpanBuilder` for `TracingLogger` records `http.target` as **the path
  only, for every request**.
  - It's a one-line change of type in `main.rs`'s `TracingLogger` wiring,
    with no session or cookie change.
  - It also stops the `q=` of ingredient search (`recipes/ingredients.rs:132`)
    and the `?…` of the sharing page (`lists/shares.rs:126`) from being
    logged, a small loss of debugging detail.
  - A test with `LogCapture` asserts that the token never appears.
- **Response headers** on both register responses:
  - `Referrer-Policy: no-referrer`, so the unpkg script request (and any
    future cross-origin request) can't carry the URL even under a laxer
    browser policy;
  - `Cache-Control: no-store`.
- **Coordination with MB-0022.** Caddy logs the full URI, including the
  query, when access logging is enabled. Its `Caddyfile` should leave access
  logs off, or delete the query with a `log … format filter` rule. To be
  recorded in MB-0022's pack.
- Pros:
  - works without JavaScript;
  - one place to redact;
  - the token never reaches the POST URL or `csrf::guard`'s path-only
    rejection log.
- Cons: the token stays in browser history until it's used (then it's
  worthless) and in the address bar while the form is open.
- Risk: low.

#### Option B — Path segment `/auth/register/{token}`

- Pros: tidy URLs.
- Cons:
  - The path appears in `csrf::guard`'s rejection log, and in any future
    path-logging code, so redaction must happen in more places.
  - A path-only root span would no longer be enough; the span would have to
    log the route pattern instead.
  - And it adds a new route shape (api-owner).
- Risk: medium (redaction spread around).

#### Option C — Fragment `#invite=…`, moved into the form by JavaScript

- Pros: browsers never send the fragment, so it's never in a server or proxy
  log.
- Cons:
  - **Needs JavaScript.** It breaks no-JS registration (AGENTS.md goal 2),
    and the app has no own JS (`assets/` holds none).
  - **Changes the vendoring decision.** Adding script there runs into
    `assets/`'s "nothing executable" boundary and MB-0027.
- Risk: low for leaks, high for the scope it pulls in. Not recommended.

**Recommendation: A.**

### Q6 — Generating and hashing the token (security; platform for the dependency)

**Generation.**

- 32 bytes from `OsRng`: `argon2::password_hash::rand_core::{OsRng, RngCore}`,
  already used by `password.rs:9,15`. That's 256 bits, above P2's 128.
- Encoded as 64 lower-case hex characters by a small helper (no `hex`
  crate needed) in a new `kauppa-core` module, e.g. `domain/invite.rs` or
  `repo/sqlite/invite.rs`. The token never leaves core except as the one
  string returned on creation.
- Not `uuid`: two v4 UUIDs give 244 bits and would do, but `OsRng` says what
  it is.

**Storing it.** A high-entropy token needs a fast cryptographic hash, not a
slow KDF. With 2^256 possibilities a fast hash isn't brute-forceable, and a
deterministic hash allows an indexed lookup.

#### Option A — Add `sha2 = "0.10"` as a direct dependency of `kauppa-core` (recommended)

- `sha2` 0.10.9 is already compiled into the binary (via `sqlx-core`). The
  lock file gains no new crate and no new version, only a direct edge.
- Store `hex(SHA-256(token))` and look it up by the unique index.
- Pros:
  - the standard construction;
  - one-line call site;
  - no new supply-chain surface: same crate, same version, already passing
    `cargo deny`.
- Cons: a manifest change, so it goes through the dependency-bump runbook
  (platform-owner). Supply-chain facts: RustCrypto org, MIT/Apache-2.0,
  and `cargo deny --offline check advisories` says "advisories ok" on
  `63da727` (local advisory DB, which includes `sha2` 0.10.9 as a transitive
  dependency). The implementing PR runs the full `cargo deny check` online.
- Risk: low.

#### Option B — Selector + verifier using the existing Argon2id (`password::hash`/`verify`)

- **Token:** `<invitation id>.<secret>`.
- **Stored:** `password::hash(secret)` (PHC string, random salt).
- **Consume:** look up the row by id, then `password::verify`.
- Pros: no manifest change at all; reuses the reviewed KDF path.
- Cons:
  - **Expensive per attempt.** It costs an Argon2id (about 19 MiB, see
    CRP-0013) per register attempt, which floods amplify. MB-0026's limiter
    becomes load-bearing for memory.
  - **Timing oracle.** An unknown id skips the verify unless a dummy verify
    is added.
  - **Atomicity is harder.** The consume can't be a single conditional
    `UPDATE … WHERE token_hash = ?`: the verify happens in Rust between a
    `SELECT` and an `UPDATE … WHERE id = ? AND used_at IS NULL`. That still
    works atomically in one transaction, but it's more moving parts.
  - **Leaks the id.** The id in the link reveals an invitation's id to its
    holder (harmless, but unusual).
- Risk: medium.

#### Option C — Argon2 as a deterministic hash (fixed salt, minimum params, `hash_password_into`)

- Pros: no manifest change; deterministic, so it can be indexed.
- Cons: misuses a password KDF as a hash with a hard-coded salt. It's
  unusual, a reviewer will rightly question it, and it sits next to
  `password.rs`'s "never roll your own" rule.
- Risk: medium (maintainability rather than strength). Not recommended.

**Recommendation: A** (direct `sha2`, platform-owner approval in the same
RR). If platform declines, B.

**Comparison.** Lookup by `token_hash = ?` on the unique index. No
constant-time compare is needed: an attacker can't steer the hash of their
guess towards a stored hash, so index timing reveals nothing usable.

### Q7 — Enumeration safety and response shape (security)

- **One error for all five invalid cases (P1).** Missing, unknown, expired,
  revoked and used tokens all get the same status (proposed: **400**,
  re-rendering `auth/register` with the message "This invitation link isn't
  valid. Ask for a new one.") and the same body. They take the same path:
  the transaction fails at step 3, so expired, revoked and used aren't told
  apart, not even by timing.
  - **Missing token** is rejected before any DB work. That's distinguishable
    by timing, but it reveals nothing about which tokens exist.
- **Check the token before hashing the password.** The handler checks the
  token's shape (64 hex characters) before `password::hash`, so garbage costs
  no Argon2.
  - A well-formed but invalid token still pays one hash. That keeps
    well-formed tokens indistinguishable from each other, and it's cheap
    enough with MB-0026's register limit.
  - **Alternative:** check validity first with a `SELECT`, outside the
    transaction. That's cheaper under a flood, but valid and invalid tokens
    then differ by one Argon2 in timing. That only tells a holder whether
    their own token is still live; guessing stays infeasible at 2^256.
    Either is acceptable. Recommendation: hash first, for uniformity.
- **Duplicate email:** see Q4.
- **Revoke and the portal's 404s.** Revoking or naming an unknown or
  already-used invitation id gives 404, same as non-admins (P3). The portal
  doesn't distinguish "not yours" because every admin sees every invitation.

### Q8 — Routes, templates and tests implied (information; api-owner only if it changes)

**Routes.**

- `GET /admin` → 303 to `/admin/invitations`.
- `GET /admin/invitations` (list).
- `POST /admin/invitations` (create; responds with the link once).
- `POST /admin/invitations/{id}/revoke`.
- `GET` and `POST /auth/register` change behaviour.
- No other route is added or removed, so there's no api-owner escalation.
  All of the above are registered in `routes::protected` under an `/admin`
  scope, where the MB-0018 guard and coverage test see them (P8).

**Admin guard (P3).** A `RequireAdmin` extractor:

- reads `Identity`;
- no identity → 303 to `/auth/login`;
- a user who isn't in `admins` → `CoreError::NotFound`, the same 404 body as
  any unknown page;
- the lookup runs on every request, never cached in the session.

The 404 for a signed-in non-admin is the plain `"not found"` body, the same
as an unregistered path. The guard doesn't render a special page.

**Templates.**

- New `templates/admin/` pages, following `templating.md`.
- `auth/register.hbs` gains the hidden field and a "needs an invitation"
  state.
- `auth/login.hbs:12` drops "No account? Register".
- `index.hbs:7`'s "Create an account" becomes plain text ("Accounts are by
  invitation") or is removed. That's a `may_decide` layout matter.

**Tests.** As listed in Current state:

- `same_origin::browse` and `World` need invitations;
- `make_user` keeps calling `UserRepo::create`.

**Bypass guard.** To stop a future handler from calling `UserRepo::create`
directly and reopening sign-up, add a source-scan test, like `repo::share`'s
bypass guard (RR-0009 constraint 3): no non-test file under
`crates/kauppa-web/src/` calls `UserRepo::create` or `.create(` on a user
repo.

- **Alternative:** remove `create` from the trait, and give fixtures a
  core-side helper that mints an invitation and registers.
- Recommendation: the guard. It changes no fixture, and no `#[cfg(test)]`
  boundary works across crates anyway.

## Recommendation and question

Recommended answers:

| # | Question | Recommendation |
| --- | --- | --- |
| Q1 | Admin status | `admins(user_id PK, granted_at, granted_by)` table |
| Q2 | Migration | `migrations/0008_invitations.sql`: `admins` + `invitations` as above; SET NULL FKs justified in header; whole-second timestamps; consume = insert user + conditional update in one repo-owned transaction |
| Q3 | Bootstrap | Host subcommands `kauppa-web admin invite [--note]` (link to stdout) and `kauppa-web admin grant <email>` (idempotent); no env list, no startup token |
| Q4 | Binding | Bearer; portal shows who used each invitation; duplicate-email message kept for invite holders but rendered in the form and no longer logged with the full email |
| Q5 | Token placement | `?invite=` query → hidden field; POST URL has no token; root span logs path only (app-wide); `Referrer-Policy: no-referrer` + `Cache-Control: no-store` on register; MB-0022's Caddy logs drop queries |
| Q6 | Token crypto | 32 bytes `OsRng` (already in tree), hex; stored as hex SHA-256 via direct `sha2` 0.10 (already in lock; platform approval); fallback: Argon2 selector/verifier |
| Q7 | Enumeration | One 400 + one message for all five invalid cases; shape check before hashing; 404 for unknown ids in the portal |
| Q8 | Routes/tests | `/admin`, `/admin/invitations` (GET/POST), `/admin/invitations/{id}/revoke`; `RequireAdmin` extractor (303 anon, 404 non-admin); CSRF tests updated to carry invitations; a bypass guard on `UserRepo::create` |

Crisp questions to answer:

1. (data-owner) Q1 **A, B or C**; Q2 the `invitations` shape above, with
   **SET NULL or CASCADE** foreign keys; and the migration filename
   **`0008_invitations.sql`**?
2. (security) Q3 bootstrap: **A (two subcommands)**, A′, B or C?
3. (security) Q4: **bearer** or email-bound? And for duplicate emails, keep
   the message for invitation holders, or use one generic error?
4. (security) Q5: **query + hidden field + path-only request spans +
   `Referrer-Policy`**, B or C? Is the app-wide path-only span acceptable?
5. (security, platform) Q6: **direct `sha2` 0.10** (dependency-bump runbook)
   or the Argon2 selector/verifier?
6. (security) Q7's response shape, and Q8's `UserRepo::create` bypass guard.

## Blast radius and rollback

**What can break:**

- **Registration.** If the consume transaction is wrong, either no one can
  register, or a token is reusable. P2's concurrency test covers the second.
- **Every existing user's login,** if the migration touches `users`. It
  doesn't under Q1 B.
- **The log redaction** in the request span changes every request log line's
  `http.target`.
- **`main.rs`** startup, if subcommand dispatch is wrong. Unknown commands
  exit non-zero instead of serving.

**Who is affected:**

- new users, who need an invitation;
- the owner, as the first admin;
- operators, who need the new bootstrap step in the deploy guide (MB-0022).

**Rollback plan:**

- Revert the code. The old binary reopens open registration, which is only
  acceptable on localhost (MB-0022's ordering).
- `0008` is additive, but **the old binary refuses to start against the
  migrated database**. `run_migrations` uses sqlx's default migrator
  (`kauppa-core/src/repo/sqlite/mod.rs:48-53`), which has
  `ignore_missing: false`. An applied version it doesn't know gives
  `VersionMissing` (`sqlx-core-0.8.6/src/migrate/migrator.rs:32-40,51`).
- So a binary rollback needs one of:
  - restoring the pre-deploy backup (MB-0022 P5);
  - deleting the `_sqlx_migrations` row for version 8 by hand. The tables can
    stay; the old code ignores them.
  - This is true of every migration so far; it's worth one line in MB-0022's
    operator guide.
- To remove the tables: a reverse migration `0009` with `DROP TABLE`, which
  needs its own RR (migration runbook → Forbidden).

## Minimal context

- `.agentic/missions/MB-0023-invite-only-registration.md`: P1–P8,
  `autonomy` (escalate and forbidden), threat notes.
- `.agentic/runbooks/migration.md`: entry gate 2–3 (the RR names the file),
  S2 rules (`STRICT`, a comment header, FK divergence named).
- `.agentic/runbooks/dependency-bump.md` (Q6 A).
- `.agentic/mentorship/security.md`: Logging; Escalation (`auth.rs`,
  `main.rs`).
- `.agentic/mentorship/persistence.md`: Schema (CASCADE default),
  Transactions (the repo owns the transaction).
- RR-0010: one cross-site check, no exempt routes; the new POSTs inherit it.
- CRP-0013 (MB-0026): register rate limit; Argon2 cost per attempt.

## Evidence bundle (Hardened)

All read on `agentic/mb-0023-invites` at `63da727`.

- **Routes:** `routes/mod.rs:27-92`; `auth.rs` handlers at lines 31, 39,
  71, 79, 99.
- **Last migration:** `ls migrations/` shows `0001`…`0007_list_shares.sql`.
  `git ls-tree` of `agentic/mb-0022-deploy`, `-0024-embed-files` and
  `-0025-sqlite-wal` shows the same last file.
- **Dependency paths:**
  - `cargo tree -i sha2`: `sha2 v0.10.9 ← sqlx-core v0.8.6` only.
  - `-i hex`: `← sqlx-macros-core` only.
  - `-i rand_core@0.6.4`: `← password-hash ← argon2 ← kauppa-core`.
  - `-i getrandom@0.4.3`: `← uuid`.
  - No `clap`.
- **Library sources** (cargo registry):
  - tracing-actix-web 0.7.25 `src/root_span_macro.rs:110`
    (`http.target = path_and_query`);
  - sqlx-sqlite 0.8.6 `src/types/chrono.rs:69` (AutoSi encoding) and
    `src/options/mod.rs:201` (5 s busy timeout);
  - password-hash 0.5.0 `src/lib.rs:29` (`pub use rand_core`);
  - argon2 0.5.3 `src/lib.rs:111-114` (re-export) and `src/params.rs:42,52,61`
    (defaults: m = 19 MiB, t = 2, p = 1);
  - sqlx-core 0.8.6 `src/migrate/migrator.rs:32-40,51` (`ignore_missing`
    defaults to false).
- **Argon2id cost:** `cargo test --release -p kauppa-core --lib
  hash_then_verify` (one hash and two verifies) finished in 0.04–0.05 s over
  four runs on the dev machine (32 threads, WSL2). That's about 15 ms and
  19 MiB per operation, single-threaded. Expect more on a small droplet
  vCPU.
- **Log leak paths:**
  - `telemetry.rs` (compact/json formatters include span fields);
  - `error.rs:44-50` (WARN with `?self`; `Conflict` text returned to the
    client);
  - `user.rs:67-69` (email in the `Conflict` text).
- **Tests to adapt:** `csrf/tests/same_origin.rs:58-83`,
  `csrf/tests/mod.rs:205,285-317,351`, `routes/test_support.rs:72-78`.
