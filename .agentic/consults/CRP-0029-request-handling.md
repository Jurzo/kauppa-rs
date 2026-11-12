# Consultation Request Pack — Request handling: hashing off the async threads, cache headers, the health check

```yaml
---
id: CRP-0029
mission: MB-0046
owner: "@juuso-kalliomaki"
target_role: security
risk_tier: low
urgency: medium
state: answered
runbook_trigger: "feature.md security trigger: `routes/auth/`; RR-0019 constraint 1 (headers come from one place); MB-0046 autonomy.escalate"
---
```

## Decision statement

Decide where password hashing runs, which cache headers pages and assets
carry and where they are set, and whether the health check queries the
database.

## Current state

- `password::hash` and `password::verify` (Argon2id defaults: 19 MiB, 2
  passes) are called directly in the async login and register handlers,
  including the dummy verify for unknown emails. Nothing uses
  `web::block` or `spawn_blocking`.
- `Cache-Control` is set only by the register and create-invitation
  responses (`no-store`). No other page sets one, and
  `routes/assets.rs` sets none and no `ETag`.
- `csp::headers` (`DefaultHeaders`, outermost) is the one place security
  headers come from (RR-0019 constraint 1); it adds a header only when the
  response lacks it.
- `/healthz` returns `ok` without touching the database. The container
  health check and Caddy's `depends_on: service_healthy` use it.

## Options and trade-offs

### Q1 — Hashing (security)

- **Recommended:** a helper in `routes/auth` runs each `password::hash` /
  `password::verify` call (the dummy verify too) inside
  `actix_web::web::block`, actix's blocking pool. `password.rs` is
  untouched. A pool error maps to `WebError::Internal` (500).
  - Unknown and known emails keep the same single Argon2 call, so RR-0013's
    equal cost holds.
  - `actix-web.md`'s red flag is about wrapping *async* code in
    `web::block`; this is CPU-bound sync code, its intended use.
- Not recommended now: a semaphore capping concurrent hashes. The rate
  limiter already bounds repeated failures per client and account; add a
  cap if load shows a need.

### Q2 — Pages (security)

- **Recommended:** `Cache-Control: no-store` on every response that
  doesn't set its own, added to `csp::headers`. Pages are per-user and
  per-language, and none is expensive to rebuild.
  - One place, as RR-0019 constraint 1 wants; the RR amends that
    constraint to include this header.
  - `/assets` sets its own header (Q3), so `DefaultHeaders` leaves it.
  - htmx's Back restore uses `sessionStorage`, not the HTTP cache, so
    boosted navigation is unaffected. The browser's own back/forward cache
    won't keep pages after a full load; they are refetched.
- Alternative: `private, no-cache` (allows revalidation; there are no
  validators, so it behaves the same with more words).

### Q3 — Assets (platform)

- **Recommended:**
  - `assets/vendor/*` (version in the file name):
    `Cache-Control: public, max-age=31536000, immutable`.
  - Everything else (`app.css`, the manifest, images):
    `Cache-Control: no-cache` with an `ETag`, and `304 Not Modified` when
    `If-None-Match` matches. A deploy's CSS reaches browsers on the next
    load; an unchanged file costs a small 304.
  - The `ETag` is computed in `build.rs` from the file's bytes with the
    standard library's hasher: no new dependency.
- Alternative: fingerprinted file names for `app.css`
  (`app.<hash>.css`, immutable). Fastest, but the layout and tests must
  learn the generated name; more machinery than this app needs.
- Alternative: a short `max-age` without validators. Simple, but new HTML
  can load with stale CSS for that period.

### Q4 — Health check (owner)

- **Recommended:** `/healthz` runs `SELECT 1` through a new
  `SqlitePool::ping()` and answers `200 ok` or `503 unavailable`, with no
  detail. One trivial read per probe.
  - It is reachable through Caddy without signing in; a caller learns only
    up or down, as today.
  - The container is restarted or held back by Compose when the database
    can't be opened, instead of serving 500s.

## Recommendation and question

Recommended: all four as above.

Crisp questions:

1. Hashing through `web::block` in a `routes/auth` helper, no semaphore?
2. `Cache-Control: no-store` as a default header in `csp::headers`,
   amending RR-0019 constraint 1?
3. Assets: immutable for `vendor/`, `no-cache` plus `ETag`/304 for the
   rest, hash computed in `build.rs`?
4. `/healthz` queries the database and answers 503 when it fails?

## Blast radius and rollback

What can break:
- A wrong immutable header on a file whose name doesn't change would pin
  it in browsers for a year; only `vendor/` gets it, and RR-0015 already
  forbids editing those files in place.
- `no-store` makes Back after a full page load refetch.
- A health check that fails for a transient reason restarts the container.

Who is affected: every user (faster repeat loads; no cached private
pages).

Rollback plan: revert the PR. Browsers keep `vendor/` files they cached as
immutable, which is harmless: those files never change.

## Minimal context

- MB-0046 P1–P4. `security.md` → Security headers rule gains the cache
  header (roll-back).
- Files: `routes/auth/mod.rs`, `csp.rs`, `routes/assets.rs`, `build.rs`,
  `routes/health.rs`, `repo/sqlite/mod.rs`.

## Evidence bundle (Hardened)

- `grep -rn "spawn_blocking\|web::block" crates/` → none (2026-11-10).
- `grep -rn CACHE_CONTROL crates/kauppa-web/src` → only
  `routes/auth/mod.rs` (`private`) and `routes/admin/mod.rs`.
- `crates/kauppa-web/src/routes/health.rs` (6 lines, no state).
- actix-web 4.18 `DefaultHeaders` inserts only headers the response lacks
  (CRP-0019 Evidence).
