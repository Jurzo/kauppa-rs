# Resolution Record — Hashing on the blocking pool, cache headers, a health check that reaches the database

```yaml
---
id: RR-0029
closes: CRP-0029
mission: MB-0046
decided_by: "@Jurzo"
role: security
risk_tier: low
decided_at: 2026-11-12T08:00:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/mentorship/security.md
  - .agentic/mentorship/actix-web.md
  - .agentic/resolutions/RR-0019-content-security-policy.md
  - .agentic/missions/MB-0046-request-handling-hardening.md
  - .agentic/consults/CRP-0029-request-handling.md
---
```

## Decision and outcome

Approve CRP-0029's **recommended set, Q1–Q4**.

Sub-answers:

1. **Hashing (Q1, security).** A helper in `routes/auth` runs every
   `password::hash` and `password::verify` call, the dummy verify
   included, inside `actix_web::web::block`. `password.rs` is unchanged.
   A pool failure is a 500. No concurrency cap in this mission.
2. **Pages (Q2, security).** `csp::headers` adds
   `Cache-Control: no-store` to every response that doesn't set its own.
   This amends RR-0019 constraint 1: the one place security headers come
   from now also sets this header.
3. **Assets (Q3, platform).** The `/assets` service sets its own
   `Cache-Control`:
   - files under `vendor/`: `public, max-age=31536000, immutable`;
   - every other file: `no-cache`, with an `ETag`, answering
     `304 Not Modified` when `If-None-Match` matches.
   `build.rs` computes each `ETag` from the file's bytes with the standard
   library's hasher. No new dependency.
4. **Health check (Q4, owner).** `/healthz` runs `SELECT 1` through a new
   `SqlitePool::ping()` and answers `200 ok`, or `503 unavailable` with no
   detail.

Covers:

- MB-0046's implementation PR, P1–P4;
- the helper and call-site changes in `routes/auth/mod.rs` (security
  trigger path), with answers and timing behaviour unchanged;
- the `csp.rs`, `routes/assets.rs`, `build.rs`, `routes/health.rs` and
  `repo/sqlite/mod.rs` changes;
- the roll-backs below.

Does not cover:

- any change to `password.rs` or the Argon2 parameters;
- a cap on concurrent hashing;
- fingerprinted asset file names;
- any other header, or any change to the CSP;
- metrics, readiness endpoints or more health detail.

## Rationale and trade-offs

- **`web::block`.** Argon2 is CPU-bound sync code; on the async workers a
  burst of sign-ins delays every other request. The blocking pool is what
  actix provides for this. One Argon2 call per attempt stays, so RR-0013's
  equal cost holds.
- **`no-store` in `csp::headers`.** Pages are per-user and per-language
  and cheap to rebuild, so nothing is lost by not caching them, and
  private pages don't linger in a shared browser's cache. Keeping the
  header in the one headers module follows RR-0019's rule instead of
  breaking it.
- **Immutable `vendor/`, revalidated rest.** Vendor file names carry their
  version and RR-0015 forbids editing them in place, so a year is safe.
  `app.css` keeps its name, so it must be revalidated: an `ETag` makes
  that a small 304 instead of a stale stylesheet after a deploy.
- **Health check.** One trivial read per probe lets Compose see a
  database that can't be opened; callers still learn only up or down.

## Approvals and authority

Approved by security-owner (Q1, Q2, and the `routes/auth/` change),
platform-owner (Q3) and the owner (Q4).

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0029-request-handling.md`
- Mission Brief: `.agentic/missions/MB-0046-request-handling-hardening.md`
- Related decisions: RR-0019 (constraint 1, amended here); RR-0013 (equal
  cost for unknown emails); RR-0015 (vendored files are never edited);
  RR-0012 (register and invitation responses' own `no-store`).
- Evidence links: CRP-0029 → Current state and Evidence bundle.

## Constraints and follow-on rules (Hardened)

1. **Hashing never runs on an async worker.** Handlers call the helper,
   never `password::hash` or `password::verify` directly; a guard test
   scans for direct calls.
2. **Same cost, same answers.** Every sign-in attempt makes exactly one
   Argon2 verification, known email or not.
3. **`no-store` is the default; a handler may not weaken it.** A response
   that should be cacheable needs an RR. `/assets` is the one exception.
4. **`immutable` only for versioned names.** A file outside `vendor/`
   never gets a long `max-age` unless its name changes with its content.
5. **`/healthz` reveals nothing but up or down.**

## Roll-back into the system (Hardened)

To be applied in MB-0046's implementation PR:

- `.agentic/mentorship/security.md` → **Security headers** rule: after
  "…and HSTS (`max-age=31536000`) when `KAUPPA_COOKIE_SECURE=true`.", add
  "Responses also carry `Cache-Control: no-store` unless they set their
  own; only `/assets` does (RR-0029)."
- `.agentic/mentorship/security.md` → **Password hashing** rule: add
  "Handlers call it through the `routes/auth` helper that runs it on the
  blocking pool (RR-0029)."
- `.agentic/mentorship/actix-web.md` → Red flags: the `web::block` line
  becomes "`web::block(...)` wrapping async code — don't; call the async
  API directly. CPU-bound sync work (password hashing) does go through
  `web::block` (RR-0029)."
- `RR-0019`: under "Supersession and history", add "Amended by RR-0029:
  `csp::headers` also sets `Cache-Control: no-store` by default
  (constraint 1)."
- `MB-0046`: `resolutions: []` → `[RR-0029]`; plan step 1 gains "Answered
  by RR-0029."
- `CRP-0029`: `state: open` → `answered`.

No Runbook change.

## Supersession and history

Status: active once approved. Amends RR-0019 constraint 1; supersedes
nothing.
