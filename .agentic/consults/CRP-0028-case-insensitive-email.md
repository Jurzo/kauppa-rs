# Consultation Request Pack — Email addresses that match regardless of case

```yaml
---
id: CRP-0028
mission: MB-0044
owner: "@juuso-kalliomaki"
target_role: data-owner
risk_tier: medium
urgency: medium
state: answered
runbook_trigger: "migration.md: a new file under `migrations/`; feature.md security trigger: `routes/auth/`; MB-0044 autonomy.escalate (schema, collisions → data-owner; auth path → security)"
---
```

## Decision statement

Decide how the schema makes email addresses unique regardless of letter
case, which case folding is used, and what happens if existing accounts
already collide.

## Current state

- `users.email` is `TEXT NOT NULL UNIQUE` (`0001_init.sql:7`): unique by
  exact bytes.
- `UserRepo::find_by_email` compares `email = ?1` exactly; registration
  stores the address as typed, untrimmed.
- Sharing already matches `lower(email) = lower(?1)` and gives the grant
  only when exactly one other account matches (`repo/sqlite/list.rs:740`),
  citing the owner's decision of 2026-10-29 to treat emails
  case-insensitively (ASCII).
- The login limiter's account key is the trimmed, ASCII-lowercased email
  (`limiter/mod.rs:280-284`).
- So `Pat@x.fi` and `pat@x.fi` can be two accounts, and a user who types a
  different case than at registration gets "Invalid email or password".

## Options and trade-offs

### Q1 — Schema

**B (recommended): a case-insensitive unique index.** One new migration:

```sql
CREATE UNIQUE INDEX users_email_nocase ON users (email COLLATE NOCASE);
```

Queries compare with `email = ?1 COLLATE NOCASE`, which uses the index.

- Pros:
  - One additive statement: no new column, no `UPDATE`, no backfill.
  - The address stays stored and shown as typed.
  - SQLite enforces it; code can't forget to fill a key column.
- Cons:
  - `NOCASE` folds ASCII letters only (see Q2).
  - Every email comparison must name the collation; a guard test scans
    for an `email =` without it.
- Risk: low. Cost: small.

**A: a normalized `email_key` column** (`ALTER TABLE … ADD COLUMN`, an
`UPDATE` backfill, a unique index), as `name_key` does for names
(RR-0003).

- Pros: any folding rule (Unicode) can live in Rust.
- Cons: a data-changing migration (the migration runbook's stricter
  path), a second column to keep in sync, more code.
- Risk: medium. Cost: medium.

### Q2 — Folding

- **Recommended: ASCII only**, plus trimming surrounding whitespace at
  sign-in and registration. It matches the 2026-10-29 decision, the
  limiter and the sharing query. Addresses with non-ASCII letters stay
  case-sensitive in those letters; rare, and no worse than today.
- Alternative: Unicode lower-casing (needs option A).

### Q3 — Existing collisions

`CREATE UNIQUE INDEX` fails if two rows already collide. The migration
then fails, and the app refuses to start on the new image (the old image
still runs; nothing was changed).

- **Recommended:** fail loudly, never merge or delete accounts
  automatically. Before deploying, the owner runs on the droplet:

  ```sql
  SELECT lower(email), count(*) FROM users GROUP BY 1 HAVING count(*) > 1;
  ```

  and resolves any hit by hand. `deploy/README.md` gets this pre-check.
  With invite-only registration and few users, a collision is unlikely.

### Q4 — Auth path changes (security)

- `find_by_email` and the duplicate check in `InvitationRepo::register`
  compare with the collation; the handler trims the submitted email
  before the lookup and the limiter key.
- Unchanged: the unknown-email and wrong-password answers and their equal
  Argon2 cost (RR-0013), and the duplicate-email answer only for holders
  of a valid invitation (RR-0012 sub-answer 4).
- The sharing query switches to the same collation, so all four places
  agree.

## Recommendation and question

Recommended: Q1 B, Q2 ASCII with trimming, Q3 fail loudly with a
documented pre-check, Q4 as described.

Crisp questions:

1. Schema: the `NOCASE` unique index in a new migration
   `0010_email_nocase.sql` (B)?
2. Folding: ASCII only, and trim whitespace at sign-in and registration?
3. Collisions: the migration fails and you resolve them by hand after the
   pre-check query?
4. The comparison changes under `routes/auth/` and in the repos, with
   answers and timing unchanged?

## Blast radius and rollback

What can break:
- A database with colliding accounts won't migrate (by design).
- A missed comparison would keep one path case-sensitive; the guard test
  covers it.

Who is affected: users who typed their email in another case (they can
now sign in).

Rollback plan: the previous image refuses a database with an unknown
migration, so rolling back needs the pre-update backup
(`deploy/README.md` §8.2) or a follow-up migration dropping the index
(its own RR). The index changes no data.

## Minimal context

- MB-0044 P1–P4; `persistence.md` (migrations append-only; purely additive
  here).
- Files: `migrations/0010_email_nocase.sql` (new),
  `repo/sqlite/user.rs`, `repo/sqlite/invitation.rs`,
  `repo/sqlite/list.rs`, `routes/auth/mod.rs`.

## Evidence bundle (Hardened)

- `migrations/0001_init.sql:7`; `repo/sqlite/user.rs` (`WHERE email = ?1`);
  `repo/sqlite/list.rs:736-741` (the 2026-10-29 comment and
  `lower(email)`); `routes/auth/limiter/mod.rs:280-284`.
- SQLite: `NOCASE` folds the 26 ASCII upper-case letters; an index on an
  expression or collation is used when the query names the same collation.
- Latest migration on `main`: `0009_user_language.sql`.
