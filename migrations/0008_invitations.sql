-- 0008_invitations — invite-only registration and administrators (MB-0023).
-- Resolution: RR-0012 (admins table, host bootstrap, bearer invitations,
-- hashed tokens), see
-- .agentic/resolutions/RR-0012-invite-only-registration.md.
-- Intent: `admins` marks the users who may open the admin portal (`/admin`);
-- `invitations` holds the invitations an admin (or the host command
-- `kauppa-web admin invite`) creates. An account is created only by using a
-- pending invitation, in the same transaction (`InvitationRepo::register`).
--
-- Tokens: only `token_hash` is stored, the lower-case hex SHA-256 of a
-- 256-bit random token. The plaintext is never stored or logged.
--
-- State is derived from the timestamps, never stored: pending (not used, not
-- revoked, `expires_at` in the future), used, revoked or expired.
--
-- Timestamps: every timestamp in these tables is truncated to whole seconds
-- before it is bound, so the RFC3339 text has a fixed width and
-- `expires_at > ?` compares correctly as text (RR-0012 constraint 7).
--
-- FK divergence (RR-0012): `admins.user_id` is the default ON DELETE CASCADE,
-- but the three audit references to the acting user, `admins.granted_by`,
-- `invitations.created_by` and `invitations.used_by`, are ON DELETE SET NULL
-- and nullable. An invitation or a grant is an audit record: deleting the
-- acting user must neither delete it nor silently demote other admins. NULL
-- in `granted_by` / `created_by` also means "the host command". No CHECK ties
-- `used_by` to `used_at`: SET NULL would make it block deleting the user.
--
-- Purely additive: two new tables and an index. `users` is untouched, no
-- existing row changes, and no existing account becomes an admin.

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
