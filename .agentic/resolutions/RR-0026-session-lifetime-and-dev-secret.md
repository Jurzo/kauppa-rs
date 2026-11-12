# Resolution Record — Session lifetime through identity deadlines; the dev session secret only on loopback

```yaml
---
id: RR-0026
closes: CRP-0026
mission: MB-0042
decided_by: "@Jurzo"
role: security
risk_tier: medium
decided_at: 2026-11-12T08:00:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/mentorship/security.md
  - .agentic/missions/MB-0042-session-lifetime-and-secret-check.md
  - .agentic/consults/CRP-0026-session-lifetime-and-dev-secret.md
---
```

## Decision and outcome

Approve CRP-0026's **recommended set, Q1–Q5**.

Sub-answers:

1. **Mechanism (Q1 A).** `main.rs` builds the identity middleware with
   `IdentityMiddleware::builder().login_deadline(…).visit_deadline(…)`
   in place of `IdentityMiddleware::default()`. `wired_app!` mirrors it.
   No server-side session store, no new dependency, no migration.
2. **Limits (Q2).** **30 days** since the last visit and **90 days** since
   sign-in, as named constants in `kauppa-web` (not settings). A test app
   may be built with shorter deadlines so tests don't wait.
3. **Cookie (Q3 (a)).** The session cookie stays as it is: a
   browser-session cookie named `kauppa_sid`, `HttpOnly`, `SameSite=Lax`,
   `Secure` with `KAUPPA_COOKIE_SECURE=true`. No `Max-Age`.
4. **Dev secret (Q4).** `Settings::load` returns an error when
   `session_secret` equals the compiled-in development value, unless the
   bind address is a loopback address **and** `cookie_secure` is `false`.
   The error names `KAUPPA_SESSION_SECRET` and never prints a secret. The
   container entrypoint's own refusal stays.
5. **Existing sessions (Q5).** Sessions issued before this change carry no
   sign-in time and are signed out by the middleware. Every user signs in
   once after the deploy. Accepted.

Covers:

- MB-0042's implementation PR, P1–P4;
- the one-builder change to the session middleware block in `main.rs`
  (security trigger path) and its mirror in `test_support.rs`;
- the startup check in `config.rs`;
- the `security.md` and brief changes below.

Does not cover:

- server-side sessions, revocation or "sign out everywhere" (a later brief
  if wanted);
- any change to the cookie's name, flags or lifetime, or to the session
  store or its key;
- making the limits configurable;
- password change, reset or account deletion.

## Rationale and trade-offs

- **Deadlines (A), chosen.** The smallest change that makes the server
  refuse an old session. The timestamps live inside the encrypted cookie,
  so nothing is stored. A stolen cookie still works until a deadline, and
  using it keeps the idle deadline alive; the 90-day limit bounds that.
  - **B (session table), not chosen:** real revocation, but a migration, a
    custom store and a write per request.
  - **C (cookie `Max-Age` only), not chosen:** a copied cookie would still
    work forever.
- **30/90 days.** A household app used weekly rarely hits the idle limit;
  a forced sign-in once a quarter is tolerable. Constants keep the rule
  reviewable, like RR-0013's limits.
- **Dev secret on loopback only.** Keeps `cargo run` with `.env.example`
  working, and stops the one dangerous case: a reachable server whose
  sessions anyone can forge.

## Approvals and authority

Approved by security-owner (Q1–Q5, and the `main.rs` session-middleware
change under the feature runbook's security trigger).

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0026-session-lifetime-and-dev-secret.md`
- Mission Brief: `.agentic/missions/MB-0042-session-lifetime-and-secret-check.md`
- Related decisions: RR-0014 (the container refuses the dev secret);
  RR-0013 (limits as constants); RR-0019 (`wired_app!` mirrors `main.rs`).
- Evidence links: CRP-0026 → Evidence bundle (actix-session's cookie store
  ignores the TTL; actix-identity's deadline defaults and enforcement).

## Constraints and follow-on rules (Hardened)

1. **Both deadlines stay on.** Removing or lengthening either needs an RR.
2. **Same wiring in tests.** `wired_app!` builds the identity middleware
   as `main.rs` does; only the durations may differ, and only in tests.
3. **The dev secret never serves a reachable address.** The check runs
   before the server binds and before the database is opened for serving.
4. **No secret in messages.** The startup error and logs never contain a
   session secret or a cookie.
5. **Nothing else in the session block changes** under this RR.

## Roll-back into the system (Hardened)

To be applied in MB-0042's implementation PR:

- `.agentic/mentorship/security.md` → Rules (must) → **Session store**:
  after "…`Secure` behind TLS (`KAUPPA_COOKIE_SECURE=true`).", add:
  "A sign-in ends 30 days after the last visit and 90 days after it was
  made, enforced by the identity middleware's deadlines in `main.rs`
  (MB-0042, RR-0026); there is no server-side revocation. The app refuses
  to start with the development session secret unless it binds to a
  loopback address with `KAUPPA_COOKIE_SECURE=false`."
- `MB-0042`: `resolutions: []` → `[RR-0026]`; Conceptual plan step 1 gains
  "Answered by RR-0026."
- `CRP-0026`: `state: open` → `answered`.

No Runbook change.

## Supersession and history

Status: active once approved. Supersedes nothing.
