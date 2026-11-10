# Mission Brief — Check password length on the server

```yaml
---
id: MB-0034
owner: "@juuso-kalliomaki"
goal: "Registration rejects passwords outside the allowed length on the server, not only through the form's `minlength`, so a hand-made request can't create an account with a weak password or make the server hash an oversized one."
non_goals:
  - "Password strength rules beyond length (character classes, breached-password lists)."
  - "Password change or reset flows (none exist yet)."
  - "Changing the KDF or its parameters in `password.rs`."
  - "Forcing existing accounts to change their password."
acceptance:
  - id: P1
    property: "`POST /auth/register` with a password shorter than the minimum (15 characters, RR-0022) answers 400 with the register page and a message naming the rule; no user is created, the invitation stays pending, and the password is not hashed."
    check: cargo test -p kauppa-web routes::auth::tests::password_length::too_short_rejected
  - id: P2
    property: "A password longer than the maximum the RR sets answers 400 the same way, before any Argon2 work."
    check: cargo test -p kauppa-web routes::auth::tests::password_length::too_long_rejected
  - id: P3
    property: "Passwords at the minimum and at the maximum are accepted; the length is counted as the RR defines (Unicode scalar values on the string as submitted, RR-0022)."
    check: cargo test -p kauppa-web routes::auth::tests::password_length::bounds_accepted
  - id: P4
    property: "Login doesn't apply the rule: an existing account keeps working whatever its password's length."
    check: cargo test -p kauppa-web routes::auth::tests::password_length::login_unaffected
  - id: P5
    property: "The register page's `minlength` comes from the same constant as the server check, and the page has no `maxlength` (RR-0022)."
    check: cargo test -p kauppa-web routes::auth::tests::password_length::form_matches_rule
context:
  - crates/kauppa-web/src/routes/auth/mod.rs
  - crates/kauppa-core/src/password.rs
  - crates/kauppa-core/src/domain/
  - templates/auth/register.hbs
  - .agentic/resolutions/RR-0012-invite-only-registration.md
  - .agentic/resolutions/RR-0013-auth-rate-limiting.md
  - .agentic/mentorship/security.md
autonomy:
  may_decide:
    - "the message wording"
    - "test placement inside `routes::auth::tests`"
  escalate:
    - { when: "the minimum and maximum, and whether length counts characters, bytes or grapheme clusters", to: security }
    - { when: "putting the rule in `password.rs` (KDF module, security trigger) rather than the domain layer or the handler", to: security }
    - { when: "the order of the check relative to the rate limiter and the invitation check (RR-0012 sub-answer 2 and step 0, RR-0013)", to: security }
  forbidden:
    - "change Argon2 parameters or the hash format"
    - "log or echo the password, its length, or any part of it"
    - "apply the rule at login"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/security.md@v1
resolutions: [RR-0022]
---
```

## Context

RR-0012 "Does not cover" lists server-side password length validation as a
follow-up: `templates/auth/register.hbs` has `minlength="12"`, but
`routes::auth::register` checks only the form's shape before
`password::hash` and `InvitationRepo::register`. Any client can skip the
attribute. There is no maximum either; actix-web's form size limit bounds
the input, but Argon2 still hashes whatever arrives.

MB-0026's limiter counts every 4xx register answer as a failure, so a
rejected password counts toward the per-client limit like other bad forms.
`routes/auth/` is a security trigger path.

## Conceptual plan

1. **Clarify (S1).** CRP to security: bounds, counting, placement, order.
2. Constants, the check in the approved place, the template using them.
3. Tests P1–P5, red then green.

## Validation plan

- P1–P5: route tests in `routes::auth::tests::password_length`.
- Smoke: register with a short password on the release binary (400, the
  invitation still works afterwards).
