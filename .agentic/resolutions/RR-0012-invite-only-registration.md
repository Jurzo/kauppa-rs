# Resolution Record — Invite-only registration: admins table, host bootstrap, bearer invitations, hashed query tokens

```yaml
---
id: RR-0012
closes: CRP-0012
mission: MB-0023
decided_by: "@juuso.kalliomaki"
role: security
risk_tier: high
decided_at: 2026-11-08T18:13:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/mentorship/security.md
  - .agentic/mentorship/persistence.md
  - .agentic/runbooks/feature.md
  - scripts/agentic/check_triggers.py
  - .github/CODEOWNERS
  - .agentic/missions/MB-0023-invite-only-registration.md
  - .agentic/consults/CRP-0012-invite-only-registration.md
---
```

## Decision and outcome

Approve CRP-0012's recommendation on every question: an **`admins` table**
and an **`invitations` table** in one additive migration, a **host-only
bootstrap** through two subcommands, **bearer** invitations, the token in a
**`?invite=` query that moves into a hidden field**, and tokens stored as
**hex SHA-256** through a direct `sha2` dependency.

Sub-answers:

1. **Admin status (Q1 B, data-owner).** A separate `admins` table. `users`
   is not altered.
2. **Migration (Q2, data-owner).** Exactly one new file,
   **`migrations/0008_invitations.sql`**, containing:

   ```sql
   CREATE TABLE admins (
       user_id    TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
       granted_at TEXT NOT NULL,
       granted_by TEXT REFERENCES users(id) ON DELETE SET NULL  -- NULL: host command
   ) STRICT;

   CREATE TABLE invitations (
       id         TEXT PRIMARY KEY,
       token_hash TEXT NOT NULL UNIQUE,
       note       TEXT,
       created_by TEXT REFERENCES users(id) ON DELETE SET NULL,  -- NULL: host command
       created_at TEXT NOT NULL,
       expires_at TEXT NOT NULL,
       revoked_at TEXT,
       used_at    TEXT,
       used_by    TEXT REFERENCES users(id) ON DELETE SET NULL,
       CHECK (revoked_at IS NULL OR used_at IS NULL)
   ) STRICT;

   CREATE INDEX idx_invitations_created ON invitations(created_at DESC);
   ```

   - **Foreign keys.** `admins.user_id` cascades (the default). The three
     audit references, `admins.granted_by`, `invitations.created_by` and
     `invitations.used_by`, are **`ON DELETE SET NULL`**, a divergence from
     `persistence.md`'s CASCADE default. The comment header names RR-0012
     and the reason: an invitation or grant is an audit record, and deleting
     the acting user must neither delete it nor silently demote other
     admins.
   - **No email column, no `state` column.** Pending, used, revoked and
     expired are derived from the timestamps. No `CHECK` ties `used_by` to
     `used_at` (SET NULL would make such a check block user deletion).
   - **No other index.** `token_hash UNIQUE` serves the consume lookup;
     `idx_invitations_created` serves the portal list.
   - **Whole-second timestamps.** Every invitation and admin timestamp is
     truncated to whole seconds before binding, so sqlx's `AutoSi` text has a
     fixed width and `expires_at > ?` compares correctly in SQL. The rule is
     written in the migration header and covered by a repo test.
   - **No backfill.** Existing users are untouched and none becomes an admin
     (P6). The migration contains no `DROP`, `DELETE` or `UPDATE`.
   - **Consume (P2).** One repo-owned transaction,
     `InvitationRepo::register(token_hash, email, display_name, password_hash, now)`:
     1. `INSERT INTO users …` (a unique violation is a duplicate email: roll
        back, the token stays usable);
     2. `UPDATE invitations SET used_at = ?now, used_by = ?uid WHERE
        token_hash = ? AND used_at IS NULL AND revoked_at IS NULL AND
        expires_at > ?now`;
     3. rows affected ≠ 1 → roll back → the one generic invalid-invitation
        error;
     4. commit. Two concurrent registrations with one token create one
        account.
3. **Bootstrap (Q3 A, security).** `kauppa-web admin <command>`, dispatched
   in `main.rs` before the server starts, with hand-parsed `std::env::args`
   (no `clap`). It loads the same `Settings`, connects and runs migrations
   first.
   - `kauppa-web admin invite [--note TEXT]` creates an ordinary invitation
     with `created_by = NULL`, prints its link once to **stdout** (absolute
     with `KAUPPA_PUBLIC_ORIGIN` when set, else the path), and logs only the
     invitation id at INFO.
   - `kauppa-web admin grant <email>` inserts into `admins` with
     `ON CONFLICT DO NOTHING` (idempotent, P5) and logs the user id at INFO,
     never the email. The same message prints whether the email is unknown
     or the user is already an admin.
   - Unknown subcommands or arguments exit non-zero and never start the
     server.
   - Not approved: A′ (`--admin` invitations), B (`KAUPPA_ADMIN_EMAILS`),
     C (a startup bootstrap token).
4. **Binding (Q4 A, security).** Bearer invitations: whoever holds the link
   registers with any email; `used_by` records the account, and the portal
   list shows who used each invitation (display name and email; the portal
   is admin-only).
   - **Duplicate email.** The message is kept for invitation holders (the
     oracle is accepted, bounded by invitations and MB-0026's register
     limit), but:
     - it renders on the register page as a form error, with the hidden
       `invite` field still set, not as today's plain 409 body;
     - the `CoreError::Conflict` text in `kauppa-core/src/repo/sqlite/user.rs:69`
       no longer contains the email, so `error.rs`'s WARN line can't log it
       (P7, `security.md` → Logging).
5. **Token placement (Q5 A, security).**
   - The link is `<origin>/auth/register?invite=<token>`. `GET
     /auth/register?invite=…` renders the form with `<input type="hidden"
     name="invite">` and does not check the token. The form posts to
     `/auth/register` with no query.
   - `GET /auth/register` with no or an empty `invite` shows "registration
     needs an invitation" (P1).
   - **Path-only request spans, app-wide.** A custom `RootSpanBuilder` for
     `TracingLogger` records `http.target` as the request path only, for
     every request. A `LogCapture` test asserts the token never appears in
     any log line. The loss of `q=` and the sharing page's query in logs is
     accepted.
   - Both register responses (GET and POST) send `Referrer-Policy:
     no-referrer` and `Cache-Control: no-store`.
6. **Token crypto (Q6 A, security; platform for the dependency).**
   - 32 bytes from `argon2::password_hash::rand_core::OsRng` (256 bits),
     encoded as 64 lower-case hex characters by a small helper in a new
     `kauppa-core` module. The plaintext token leaves core only as the one
     string returned on creation.
   - Stored as `hex(SHA-256(token))` and looked up by the unique index; no
     constant-time compare is needed.
   - **Dependency (platform sub-approval).** `sha2 = "0.10"` as a direct
     dependency of `kauppa-core`, default features. It resolves to the
     `sha2` 0.10.9 already in `Cargo.lock` (via sqlx and `cookie`), so the
     lock file gains an edge and no new package or version.
   - **The same approval covers `sha2` as a `[dev-dependencies]` entry of
     `kauppa-web` for MB-0027** (CRP-0015 Q3 option (i), `sha2 = "0.10.9"`),
     on the same terms: the locked 0.10.9, no new package, not in the release
     binary. RR-0015 cites this sub-answer for it.
   - The fallback (Argon2 selector/verifier, Q6 B) is not needed.
7. **Enumeration and response shape (Q7, security).**
   - Missing, unknown, expired, revoked and used tokens all get **400**,
     re-rendering `auth/register` with "This invitation link isn't valid. Ask
     for a new one." and the same body.
   - A missing token is rejected before any DB work. A present token's shape
     (64 hex characters) is checked before `password::hash`, so garbage costs
     no Argon2. A well-formed token always pays one hash before the
     transaction (hash first, for uniformity).
   - In the portal, revoking or naming an unknown or already-used invitation
     id gives 404.
8. **Routes, guard and tests (Q8).**
   - `GET /admin` → 303 to `/admin/invitations`; `GET` and `POST
     /admin/invitations`; `POST /admin/invitations/{id}/revoke`; changed `GET`
     and `POST /auth/register`. All registered in `routes::protected` under an
     `/admin` scope (P8). No api-owner escalation.
   - A `RequireAdmin` extractor: no identity → 303 to `/auth/login`; not in
     `admins` → `CoreError::NotFound` (the plain 404 body); looked up on every
     request, never cached in the session.
   - `auth/login.hbs` drops "No account? Register"; `index.hbs`'s "Create an
     account" becomes plain text or is removed (`may_decide`).
   - `csrf/tests/same_origin.rs::browse` and `csrf/tests/mod.rs`'s `World`
     carry a valid invitation, so the forged register request is refused by
     the cross-site check, not by a missing token; `World::uri` uses a real
     invitation id for the revoke route, and `snapshot` includes
     `invitations`. `test_support::make_user` keeps calling
     `UserRepo::create`.
   - **Bypass guard.** A source-scan test: no non-test file under
     `crates/kauppa-web/src/` calls `UserRepo::create`.

Covers:

- MB-0023's implementation PR, with P1–P8 as written;
- `migrations/0008_invitations.sql` exactly as above, and no other migration;
- the `sha2` edge in `crates/kauppa-core/Cargo.toml`, and the `kauppa-web`
  dev-dependency for MB-0027 (sub-answer 6);
- the `main.rs` subcommand dispatch and the `TracingLogger` root span change
  (no session or cookie change);
- the duplicate-email fixes in `user.rs:69` and the register template;
- moving `routes/auth.rs` to `routes/auth/` for its tests (RR-0009 layout)
  and the trigger-path updates below;
- the test changes in sub-answer 8.

Does not cover:

- email-bound invitations (Q4 B), which wait for email verification;
- an `is_admin` column, a roles table, or invitations that grant admin;
- `KAUPPA_ADMIN_EMAILS` or any startup bootstrap token;
- the register rate limit and the login timing oracle (MB-0026, CRP-0013);
- server-side password length validation (`minlength` exists only in the
  template; a follow-up);
- case-insensitive email matching (`users.email` stays `BINARY`);
- a general audit of other `CoreError` messages for emails;
- MB-0022's Caddy access-log handling (follow-up below);
- a reverse migration (`0009` with `DROP TABLE` needs its own RR).

## Rationale and trade-offs

- **`admins` table (Q1 B) — chosen.** Purely additive: `users` and every
  existing query are untouched, so P6 holds trivially. The per-request check
  is a primary-key lookup, and the row records when and how admin was
  granted. Not chosen: A (`users.is_admin`), which alters the credential
  table and records nothing about the grant; C (roles tables), scope creep
  against the "more roles" non-goal.
- **SET NULL audit references — chosen.** Deleting a user should leave
  invitations showing "used" or "revoked". Not chosen: CASCADE on both,
  which conforms to the default but loses history.
- **Whole-second timestamps — chosen.** Keeps the whole guard in the one
  conditional `UPDATE`. Not chosen: comparing expiry in Rust.
- **Host subcommands (Q3 A) — chosen.** Only someone with shell access,
  already root-equivalent for the SQLite file, can bootstrap; there is no
  web path, no claim window and no token in the log stream. The second step
  on a fresh install is cheap. Not chosen: A′ (a leaked bootstrap link
  creates an admin), B (no answer for the first user, email squatting,
  privilege in config), C (a credential in the container log, claimable by
  anyone until used).
- **Bearer (Q4 A) — chosen.** Without email verification, binding stops a
  link thief only from choosing a different address; it adds a stored email,
  a normalization rule and typo friction. Misuse shows in the portal. Not
  chosen: B, for now; the stricter "one generic error for every failure",
  which would strand an invitee on a typo'd address.
- **Query + hidden field (Q5 A) — chosen.** Works without JavaScript, keeps
  the token out of the POST URL and `csrf::guard`'s path log, and needs
  redaction in one place. Not chosen: B (path segment; redaction spread over
  several log sites and a new route shape), C (fragment; needs own JS in
  `assets/`).
- **SHA-256 via direct `sha2` (Q6 A) — chosen.** The standard construction
  for a high-entropy token, with no new supply-chain surface (same crate and
  version already compiled in and passing `cargo deny`). Not chosen: B
  (Argon2 per attempt, a timing oracle and a split consume), C (a password
  KDF misused with a fixed salt).
- **Hash before the transaction (Q7) — chosen.** Well-formed tokens are
  indistinguishable by timing. The cheaper `SELECT`-first alternative was
  acceptable but not chosen.
- **Bypass guard (Q8) — chosen.** Changes no fixture. Not chosen: removing
  `UserRepo::create` from the trait.

## Approvals and authority

Approved by security-owner @juuso.kalliomaki; the schema and migration filename
(sub-answers 1–2) by data-owner @juuso.kalliomaki; the `sha2` dependency, including
the MB-0027 dev-dependency (sub-answer 6), by platform-owner @juuso.kalliomaki.
Risk tier high: record any co-approver here.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0012-invite-only-registration.md`
- Mission Brief: `.agentic/missions/MB-0023-invite-only-registration.md`
- Related: CRP-0013 (MB-0026: register rate limit, login timing, shared
  root span); CRP-0015 Q3 (MB-0027: `sha2` dev-dependency, cited by
  RR-0015); RR-0009 (test layout, bypass-guard precedent); RR-0010 (the new
  POSTs inherit the one cross-site check); RR-0001 (SET NULL exception
  precedent).
- Evidence links: CRP-0012 → Current state and Evidence bundle:
  - last migration `0007_list_shares.sql`, no other open branch adds one;
  - `cargo tree -i sha2`: 0.10.9, transitive only; `rand_core` 0.6.4 via
    `argon2`; no `clap`. (Correction while drafting: `cargo tree -i sha2
    --offline` on `b05d51b` shows `sha2` reached through `cookie` 0.16.2 as
    well as `sqlx-core`, as CRP-0015 notes; CRP-0012 lists only `sqlx-core`.
    The conclusion, no new package, is unchanged.)
  - tracing-actix-web 0.7.25 records `http.target = path_and_query`;
  - sqlx `AutoSi` timestamp encoding and the 5 s busy timeout;
  - the email in `user.rs:67-69` and `error.rs:44-50`;
  - sqlx's migrator refuses an unknown applied version (`ignore_missing:
    false`).

## Constraints and follow-on rules (Hardened)

1. **One way to create an account.** Outside tests, accounts are created
   only by `InvitationRepo::register`. A handler that calls
   `UserRepo::create` fails the bypass guard.
2. **Tokens.** At least 256 bits from `OsRng`; stored only as hex SHA-256;
   the plaintext appears once, in the creation response or the subcommand's
   stdout, and is never logged, stored or put in a POST URL.
3. **No query strings in request logs.** The root span records the path
   only. Re-adding the query to any log field needs an RR.
4. **One invalid-invitation response.** The five invalid cases share status,
   body and code path. A change that tells them apart is a review failure.
5. **Admin from the database, every request.** `RequireAdmin` never caches
   in the session; anonymous → 303 to login, non-admin → the plain 404.
   Admin is granted only by `kauppa-web admin grant` (or, in a later brief,
   by an existing admin); never by a web request without one.
6. **Audit logging (P7).** Create, revoke and use of an invitation, and
   admin grants, log at INFO with the acting user id and invitation id;
   never a token or a full email. The invitation note is never logged.
7. **Schema.** `0008_invitations.sql` is the only migration this RR allows.
   Any further column (e.g. an email for Q4 B) needs a new consult and RR.
   Timestamps the SQL compares are bound at whole seconds.
8. **Rollback.** A binary rollback needs the pre-deploy backup or a manual
   delete of `_sqlx_migrations` version 8 (already in MB-0022's operator
   guide). Removing the tables is a reverse migration with its own RR.
9. **Coordination.**
   - MB-0026 lands after MB-0023 and adds `http.client_ip` to the same root
     span builder (CRP-0013 Q6); if MB-0026 lands first, MB-0023 adds
     path-only `http.target` to its builder.
   - MB-0022 follow-up: its Caddyfile leaves access logs off, or drops the
     query with a `log … format filter` rule; record it in MB-0022's
     Continuity Pack.

## Roll-back into the system (Hardened)

To be applied in MB-0023's implementation PR:

- `.agentic/mentorship/security.md`:
  - add to Rules (must): "**Registration.** Accounts are created only by
    consuming a valid invitation in the same transaction
    (`InvitationRepo::register`; MB-0023, RR-0012). Handlers never call
    `UserRepo::create`. Invitation tokens are 256 bits from `OsRng`, stored
    only as hex SHA-256, and never logged."
  - add to Rules (must): "**Admin.** Admin status lives in the `admins`
    table and is checked from the database on every `/admin` request, never
    cached in the session. It is granted from the host
    (`kauppa-web admin grant`), never by a web request without an existing
    admin (RR-0012)."
  - append to the Logging rule: "Request log spans record the path only,
    never the query string (RR-0012)."
  - Escalation: `crates/kauppa-web/src/routes/auth.rs` becomes
    `crates/kauppa-web/src/routes/auth/`; add
    `crates/kauppa-web/src/routes/admin/` and `crates/kauppa-web/src/admin/`.
- `.agentic/mentorship/persistence.md`:
  - Schema, after the RR-0001 exception: "Exception (RR-0012): audit
    references to the acting user (`admins.granted_by`,
    `invitations.created_by`, `invitations.used_by`) use `ON DELETE SET
    NULL` and stay nullable, so deleting a user keeps the record. Timestamps
    that SQL compares (`expires_at > ?`) are truncated to whole seconds
    before binding."
  - Queries: the repository trait list gains `InvitationRepo` (and the
    admin-lookup trait, if the admin check gets its own).
- `.agentic/runbooks/feature.md` → Escalation triggers: the
  `crates/kauppa-web/src/routes/auth.rs` row becomes
  `crates/kauppa-web/src/routes/auth/`; add a row "Admin portal and
  bootstrap in `crates/kauppa-web/src/routes/admin/` and
  `crates/kauppa-web/src/admin/` | @juuso-kalliomaki (security)".
- `scripts/agentic/check_triggers.py`: the `routes/auth.rs` prefix becomes
  `crates/kauppa-web/src/routes/auth` (matching both forms); add
  `("crates/kauppa-web/src/routes/admin/", "security:admin")` and
  `("crates/kauppa-web/src/admin/", "security:admin")`.
- `.github/CODEOWNERS`: `/crates/kauppa-web/src/routes/auth.rs` becomes
  `/crates/kauppa-web/src/routes/auth/`; add
  `/crates/kauppa-web/src/routes/admin/` and `/crates/kauppa-web/src/admin/`.
- `MB-0023`:
  - `resolutions` gains `RR-0012`;
  - P5's "per the RR" points at RR-0012.
- `CRP-0012`: `state: open` → `answered`.

## Supersession and history

Status: active once signed. Supersedes nothing. Adds a second
`persistence.md` SET NULL exception beside RR-0001's. Its `sha2`
sub-approval is cited by RR-0015 (MB-0027).
