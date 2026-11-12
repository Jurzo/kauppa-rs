# Resolution Record — Case-insensitive email through a `NOCASE` unique index

```yaml
---
id: RR-0028
closes: CRP-0028
mission: MB-0044
decided_by: "@Jurzo"
role: data-owner
risk_tier: medium
decided_at: 2026-11-12T08:00:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/mentorship/persistence.md
  - .agentic/mentorship/security.md
  - .agentic/missions/MB-0044-case-insensitive-email.md
  - .agentic/consults/CRP-0028-case-insensitive-email.md
---
```

## Decision and outcome

Approve CRP-0028's **recommended set, Q1–Q4**.

Sub-answers:

1. **Schema (Q1 B, data-owner).** Exactly one new migration,
   `migrations/0010_email_nocase.sql`, with a comment header naming
   MB-0044 and this RR, and one statement:

   ```sql
   CREATE UNIQUE INDEX users_email_nocase ON users (email COLLATE NOCASE);
   ```

   No new column, no `UPDATE`, no `DROP`. The existing `UNIQUE` on `email`
   stays.
2. **Folding (Q2).** ASCII case only (`NOCASE`). Sign-in and registration
   trim surrounding whitespace from the submitted email before any use.
   The address is stored and shown as typed (after trimming).
3. **Collisions (Q3).** If two existing rows differ only by ASCII case,
   the migration fails and the app doesn't start on the new image. Nothing
   merges or deletes accounts automatically. `deploy/README.md` gains the
   pre-check query, and the owner runs it on the droplet before deploying.
4. **Comparisons (Q4, security).** Every comparison of a user's email
   names the collation: `find_by_email`, the duplicate check in
   `InvitationRepo::register`, and the sharing lookup (replacing its
   `lower(email) = lower(?)`). A guard test fails on an email comparison
   without it. The unknown-email and wrong-password answers, their equal
   cost (RR-0013) and the duplicate-email rule (RR-0012 sub-answer 4) are
   unchanged.

Covers:

- MB-0044's implementation PR, P1–P4, under the migration runbook;
- `migrations/0010_email_nocase.sql` exactly as above, and no other
  migration;
- the comparison and trimming changes in `repo/sqlite/user.rs`,
  `repo/sqlite/invitation.rs`, `repo/sqlite/list.rs` and
  `routes/auth/mod.rs` (security trigger path);
- the `persistence.md`, `security.md`, `deploy/README.md` and brief
  changes below.

Does not cover:

- Unicode case folding or normalization, or provider rules (dots, plus
  tags);
- changing, merging or deleting any account;
- a reverse migration dropping the index (its own RR);
- the limiter's account key, which already folds ASCII case.

## Rationale and trade-offs

- **The index (B), chosen.** One additive statement; SQLite enforces the
  rule; the address stays as typed. The cost is naming the collation in
  each comparison, which the guard test checks.
  - **A (`email_key` column), not chosen:** a data-changing migration and
    a second column to keep in sync, for folding rules nobody needs yet.
- **ASCII.** It matches the 2026-10-29 decision, the limiter and the
  sharing lookup. Email local parts with non-ASCII letters are rare.
- **Fail loudly.** A collision means two real accounts; only a person can
  decide between them.

## Approvals and authority

Approved by data-owner (Q1–Q3: the migration and the collision rule) and
security-owner (Q4: the changes under `routes/auth/`).

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0028-case-insensitive-email.md`
- Mission Brief: `.agentic/missions/MB-0044-case-insensitive-email.md`
- Related decisions: RR-0012 (registration transaction, duplicate-email
  answer); RR-0013 (limiter, equal cost); RR-0002 (sharing by email);
  RR-0003 (`name_key`, a different rule for names).
- Evidence links: CRP-0028 → Current state and Evidence bundle.

## Constraints and follow-on rules (Hardened)

1. **One rule everywhere.** Sign-in, registration, sharing and the limiter
   treat emails that differ only by ASCII case as the same address.
2. **Named collation.** An SQL comparison on `users.email` uses
   `COLLATE NOCASE`; the guard test enforces it.
3. **Stored as typed.** The app never rewrites a stored email's case.
4. **No automatic resolution** of colliding accounts.
5. **Schema.** `0010_email_nocase.sql` is the only migration this RR
   allows.
6. **Rollback.** After the migration, an older image needs the pre-update
   backup (`deploy/README.md` §8.2). Dropping the index needs an RR.

## Roll-back into the system (Hardened)

To be applied in MB-0044's implementation PR:

- `.agentic/mentorship/persistence.md` → Queries: add
  "Email addresses are unique and compared ignoring ASCII case: the
  `users_email_nocase` index, and `COLLATE NOCASE` in every comparison on
  `users.email` (MB-0044, RR-0028). The address is stored as typed."
- `.agentic/mentorship/security.md` → Rules (must): add after
  **Registration**: "**Email addresses.** Sign-in, registration, sharing
  and the rate limiter treat addresses that differ only by ASCII case as
  one, and trim surrounding whitespace (MB-0044, RR-0028)."
- `MB-0044`: `resolutions: []` → `[RR-0028]`; plan step 1 gains "Answered
  by RR-0028."
- `CRP-0028`: `state: open` → `answered`.

No Runbook change.

## Supersession and history

Status: active once approved. Supersedes nothing. Makes the owner's
2026-10-29 decision for sharing the rule for every email comparison.
