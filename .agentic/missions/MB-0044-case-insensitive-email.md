# Mission Brief — Email addresses match regardless of case

```yaml
---
id: MB-0044
owner: "@juuso-kalliomaki"
goal: "A person signs in, registers and is found for sharing by their email address whatever letter case they type, and two accounts can't differ only by case."
non_goals:
  - "Unicode normalization or provider-specific rules (dots, plus tags)."
  - "Email verification or changing an account's email."
  - "Changing how emails are displayed (the address as typed stays visible)."
acceptance:
  - id: P1
    property: "Sign-in succeeds with the registered address in any letter case, and unknown and wrong-password answers stay identical (RR-0013)."
    check: "cargo test -p kauppa-web routes::auth::tests::email_case::login_ignores_case"
  - id: P2
    property: "Registration refuses an address that differs from an existing account's only by case, with the existing duplicate-email answer."
    check: "cargo test -p kauppa-core repo::invite::email_case::duplicate_differs_only_by_case"
  - id: P3
    property: "Sharing and the login rate limiter's account key treat such addresses as one, consistent with sign-in."
    check: "cargo test -p kauppa-web routes::auth::tests::email_case::limiter_and_sharing_agree"
  - id: P4
    property: "A database that already holds accounts differing only by case migrates without data loss, as the RR decides, and every existing user can still sign in."
    check: "cargo test -p kauppa-core repo::invite::email_case::migration_keeps_existing_users"
context:
  - migrations/0001_init.sql
  - crates/kauppa-core/src/repo/sqlite/user.rs
  - crates/kauppa-core/src/repo/sqlite/invitation.rs
  - crates/kauppa-core/src/repo/sqlite/list.rs
  - crates/kauppa-web/src/routes/auth/mod.rs
  - crates/kauppa-web/src/routes/auth/limiter/mod.rs
  - .agentic/mentorship/persistence.md
autonomy:
  may_decide:
    - "test names and layout"
    - "whether the comparison lives in a domain type (e.g. a normalized email key) or in SQL"
  escalate:
    - { when: "the schema change: a normalized key column with a unique index, or `COLLATE NOCASE`; a new migration either way", to: data-owner }
    - { when: "what happens to existing rows that collide", to: data-owner }
    - { when: "any change under `routes/auth/` or to the limiter's account key", to: security }
    - { when: "which case folding is used (ASCII only, or Unicode lower-casing as `name_key` does, RR-0003)", to: owner }
  forbidden:
    - "edit a committed migration"
    - "delete or merge accounts automatically"
    - "log full email addresses at info or above"
runbook: .agentic/runbooks/migration.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/persistence.md@v1
  - .agentic/mentorship/security.md@v1
resolutions: []
---
```

## Context

Found in the audit of 2026-11-10 (finding 5).

`users.email` is `TEXT NOT NULL UNIQUE` (case-sensitive), `find_by_email`
compares exactly, and registration stores the address as typed. So
`Pat@x.fi` and `pat@x.fi` can be two accounts, and a user who registered
with one spelling can't sign in with the other. Sharing already compares
`lower(email)` and gives up when two accounts match (`LIMIT 2`), so it
treats them as one person where sign-in doesn't.

## Conceptual plan

1. **Clarify (S1).** CRP to data-owner (schema, collisions) and security
   (auth path, limiter key).
2. Migration and repo changes with tests first; then the handler paths.
3. Check the droplet's data for collisions before deploying.

## Validation plan

- P1–P3: route and repo tests.
- P4: a migration test from the previous schema with colliding rows.
