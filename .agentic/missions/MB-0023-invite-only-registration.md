# Mission Brief — Invite-only registration with an admin portal

```yaml
---
id: MB-0023
owner: "@juuso-kalliomaki"
goal: "Close open sign-up before the app goes public: a new account can only be created with a valid invitation, invitations are created and revoked by an administrator in a small admin portal, and the first administrator is set up from the host, with no default credentials."
non_goals:
  - "Sending email. The admin copies the invitation link and passes it on themselves."
  - "Inviting someone to a shared list who has no account yet (MB-0008's \"pending invites\" follow-up). An invitation here creates an account and nothing else."
  - "Managing users beyond what invitations need: no password resets, user deletion, disabling or role editing in the portal (later briefs)."
  - "More roles than admin and regular user."
  - "Changing how login, sessions or password hashing work."
acceptance:
  - id: P1
    property: "`POST /auth/register` creates an account only with a valid invitation token. A missing, unknown, expired, revoked or already used token creates nothing and gets the same response and error message for all five cases, so the form doesn't reveal which tokens exist. `GET /auth/register` without a token shows that registration needs an invitation, and the login page no longer links to open sign-up."
    check: cargo test -p kauppa-web routes::auth::tests::register_requires_invite
  - id: P2
    property: "An invitation token has at least 128 bits from the OS RNG, is stored only as a hash, expires (default 7 days), and is used up atomically in the same transaction that creates the user: two concurrent registrations with one token create one account."
    check: cargo test -p kauppa-core repo::invite::single_use_and_expiry
  - id: P3
    property: "Only administrators reach `/admin/*`. Anonymous requests are redirected to login; signed-in non-admins get 404, the same as an unknown page. Admin status is checked on every request from the database, not cached in the session cookie, so removing it takes effect at once."
    check: cargo test -p kauppa-web routes::admin::tests::access::admin_only
  - id: P4
    property: "In the portal an admin can create an invitation (with an optional note, such as who it's for), sees its link once on creation, and can list invitations with their state (pending, used, expired, revoked) and revoke a pending one. The portal works on a 320 px screen and with htmx disabled."
    check: cargo test -p kauppa-web routes::admin::tests::invites::create_list_revoke
  - id: P5
    property: "The first administrator is set up from the host (e.g. a subcommand run with `docker compose exec`, per the RR). It needs no web request, there are no default credentials or default admin, and running it again for the same user changes nothing."
    check: cargo test -p kauppa-web admin::bootstrap::tests::grants_admin_once
  - id: P6
    property: "Existing users are unaffected: the migration only adds to the schema, every existing account can log in and keeps its lists, recipes and shares, and no existing account becomes an admin."
    check: cargo test -p kauppa-core repo::invite::existing_users_unaffected
  - id: P7
    property: "Creating, revoking and using an invitation, and granting admin, are logged at INFO with the acting user's id and the invitation id, never the token or a full email (`security.md` → Logging)."
    check: cargo test -p kauppa-web routes::admin::tests::invites::audit_logging
  - id: P8
    property: "Every new POST route goes through MB-0018's cross-site check; the coverage guard sees them."
    check: cargo test -p kauppa-web csrf::tests::coverage::every_write_route_is_covered
context:
  - crates/kauppa-web/src/routes/auth.rs
  - crates/kauppa-web/src/routes/mod.rs
  - crates/kauppa-web/src/routes/request.rs
  - crates/kauppa-web/src/main.rs
  - crates/kauppa-core/src/repo/mod.rs
  - crates/kauppa-core/src/repo/sqlite/user.rs
  - migrations/0001_init.sql
  - templates/auth/register.hbs
  - templates/auth/login.hbs
  - .agentic/mentorship/security.md
  - .agentic/mentorship/persistence.md
  - .agentic/runbooks/migration.md
autonomy:
  may_decide:
    - "the portal's page layout and markup, following `templating.md`"
    - "the default expiry, within 1–14 days, as a constant"
    - "test placement within the named areas"
  escalate:
    - { when: "the schema shape: an `is_admin` column, a roles table, the `invitations` table's columns (migration runbook: CRP before any file in `migrations/`)", to: data-owner }
    - { when: "how the first admin is bootstrapped (host subcommand, `KAUPPA_ADMIN_EMAILS`, a one-time bootstrap invitation printed at startup)", to: security }
    - { when: "whether an invitation is bound to one email address or usable by whoever holds the link", to: security }
    - { when: "the token's form in the link (path or query) and whether it stays in the registration form as a hidden field", to: security }
    - { when: "adding a dependency (e.g. for random tokens or hashing beyond what's in the tree)", to: platform-owner }
    - { when: "any route besides `/admin/*` and the changed `/auth/register` is added or removed", to: api-owner }
    - { when: "an admin should see or manage users beyond invitations", to: owner }
  forbidden:
    - "default credentials, a default admin account, or an admin granted through a web request without an existing admin"
    - store invitation tokens in plain text or log them
    - "edit a committed migration, or add a migration the RR doesn't name"
    - "change `password.rs`, session middleware or cookie settings"
    - lower lint strictness
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/security.md@v1
  - .agentic/mentorship/persistence.md@v1
  - .agentic/mentorship/actix-web.md@v1
  - .agentic/mentorship/templating.md@v1
resolutions: []
---
```

## Context

Requested by the owner (2026-11-08), as a precondition for the first public
deployment (MB-0022). Today `POST /auth/register` lets anyone with the URL
create an account (`routes/auth.rs`), and the login page links to it. The
`users` table (`0001_init.sql`) has no notion of roles.

This mission touches three trigger paths: `routes/auth.rs` (security),
`migrations/` (data) and probably `main.rs` for a subcommand (security). So
it starts with a Consultation Request Pack and an approved RR, and the
migration follows `.agentic/runbooks/migration.md`.

Threat notes for the CRP:
- An invitation link is a bearer credential until it's used. That argues for
  hashing it at rest, single use, short expiry and revocation.
- The register form must not act as an oracle for which tokens or emails
  exist (MB-0008 P5 set the same bar for sharing).
- The admin portal is a new privileged surface. Hiding it from non-admins
  (404) and checking admin status per request limit what a stolen
  non-admin session can learn or do.

## Conceptual plan

1. **Clarify (S1).** One CRP to security and data-owner: schema shape,
   bootstrap, email binding, token placement. Stop until the RR is approved;
   the RR names the migration file.
2. **Core (P2, P6).** Migration, `InvitationRepo` (create, list, revoke,
   consume-in-transaction), admin flag on `User` or a separate lookup.
3. **Registration (P1).** The register handler requires and consumes a
   token; templates updated.
4. **Admin (P3–P5, P7).** An `admin` extractor or guard checking the
   database; `routes/admin/` with the invitation pages; the bootstrap path.
5. **Verify (P8).** Coverage guard, smoke test on the built binary.

Checkpoints: P2 and P6 green at the repo layer before the web slice.

Slow-mode triggers:
- the user creation and token use can't be one transaction;
- the admin check would need to live in the session cookie;
- any existing test needs weakening to pass.

## Validation plan

- **P2, P6:** `kauppa-core` tests in `repo/invite.rs`: token entropy and
  hashing, expiry with a fixed clock, revoke, concurrent consume (one
  winner), and migrating a database with existing users.
- **P1:** `routes::auth::tests` covers each invalid-token case giving an
  identical response, and a valid token creating the user exactly once.
- **P3, P4, P7:** `routes::admin::tests::{access, invites}`, with
  `LogCapture` for the audit lines.
- **P5:** unit test of the bootstrap function, plus a smoke run of the
  subcommand on the built binary.
- **P8:** the existing MB-0018 coverage guard.
- **Smoke:** bootstrap an admin, create an invitation, register with it,
  re-use it (rejected), revoke another, log in as a non-admin and get 404 on
  `/admin`.
