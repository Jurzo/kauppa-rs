-- 0010_email_nocase — email addresses unique regardless of ASCII case (MB-0044).
-- Resolution: RR-0028 sub-answer 1, see
-- .agentic/resolutions/RR-0028-case-insensitive-email.md.
-- Intent: `Pat@x.fi` and `pat@x.fi` are one address. Every comparison on
-- `users.email` names `COLLATE NOCASE` and uses this index; the address is
-- stored as typed. The existing `UNIQUE` on `email` stays.
--
-- Purely additive: no new column, no UPDATE, no DROP. If two existing rows
-- differ only by ASCII case, this statement fails and the migration stops
-- with nothing changed (RR-0028 sub-answer 3); run the pre-check in
-- deploy/README.md before deploying.

CREATE UNIQUE INDEX users_email_nocase ON users (email COLLATE NOCASE);
