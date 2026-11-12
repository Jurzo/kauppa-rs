# Mission Brief — Request handling: password hashing off the async threads, cache headers, a real health check

```yaml
---
id: MB-0046
owner: "@juuso-kalliomaki"
goal: "Slow password hashing can't stall other requests, signed-in pages aren't kept by caches while static assets are cached properly, and the health check reports whether the app can reach its database."
non_goals:
  - "Changing Argon2 parameters or `password.rs` (security-tier)."
  - "A CDN or asset fingerprinting for `app.css` beyond what the RR decides."
  - "Metrics or a status page."
acceptance:
  - id: P1
    property: "Password hashing and verification in the login and registration handlers run on the blocking thread pool, with the same results, timing behaviour for unknown emails (RR-0013) and error handling as today."
    check: "cargo test -p kauppa-web routes::auth::tests::hashing::runs_off_the_async_workers"
  - id: P2
    property: "Every HTML response for a signed-in user carries the cache policy the RR sets (for example `Cache-Control: no-store`), and the register and invitation responses keep theirs."
    check: "cargo test -p kauppa-web csp::tests::signed_in_pages_are_not_cached"
  - id: P3
    property: "`/assets/` responses carry cache headers: long-lived and immutable for versioned vendor files, and revalidated (or fingerprinted) for `app.css` and the manifest, so a deploy's CSS change reaches browsers."
    check: "cargo test -p kauppa-web routes::assets::tests::cache_headers"
  - id: P4
    property: "`/healthz` answers 200 only when a trivial database query succeeds and 503 otherwise, without leaking details, and the container health check and smoke tests still pass."
    check: "cargo test -p kauppa-web routes::health::tests::reports_database_state"
context:
  - crates/kauppa-web/src/routes/auth/mod.rs
  - crates/kauppa-core/src/password.rs
  - crates/kauppa-web/src/csp.rs
  - crates/kauppa-web/src/routes/assets.rs
  - crates/kauppa-web/src/routes/health.rs
  - deploy/healthcheck.sh
  - scripts/deploy/smoke_image.sh
autonomy:
  may_decide:
    - "test names"
    - "where the blocking call is wrapped (a helper in `routes/auth`, not in `password.rs`)"
    - "the health query"
  escalate:
    - { when: "any change under `routes/auth/` (security trigger path), and confirming the unknown-email timing stays equal", to: security }
    - { when: "the cache policy for signed-in pages, and whether it goes through `csp::headers` (RR-0019 constraint 1 keeps headers in one place)", to: security }
    - { when: "asset cache lifetimes and whether `app.css` gets a versioned name", to: platform-owner }
    - { when: "a health endpoint that queries the database on every probe (cost, and what an unauthenticated caller learns)", to: owner }
  forbidden:
    - "edit `crates/kauppa-core/src/password.rs`"
    - "weaken any header from RR-0019"
    - "return error details from `/healthz`"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/security.md@v1
  - .agentic/mentorship/actix-web.md@v1
resolutions: [RR-0029]
---
```

## Context

Found in the audit of 2026-11-10 (findings 7 and 20).

- `password::hash` and `password::verify` (Argon2id, tens of milliseconds)
  are called directly in async handlers; nothing uses `spawn_blocking` or
  `web::block`. A burst of sign-ins occupies the worker threads. The rate
  limiter bounds failures per client, not concurrent hashing.
- Only the register and create-invitation responses set `Cache-Control`.
  Signed-in pages have none, and `routes/assets.rs` sets none, so browsers
  fall back to heuristics: private pages may be kept, and assets may be
  refetched or go stale (AGENTS.md goal 2: works on a slow phone).
- `/healthz` returns `ok` without touching the database, so the container
  reports healthy when the database is unreachable.

## Conceptual plan

1. **Clarify (S1).** CRP to security (auth path, cache policy placement)
   with the platform and owner questions. Answered by RR-0029.
2. Tests first for each property; small changes in four modules.
3. Roll-back into `security.md` if the RR adds a cache rule.

## Validation plan

- P1: a test that many concurrent sign-ins don't delay an unrelated request.
- P2–P4: header and status assertions; the image smoke tests.
