# Consultation Request Pack — Rate limiting login and registration (MB-0026)

```yaml
---
id: CRP-0013
mission: MB-0026
owner: "@juuso-kalliomaki"
target_role: security
risk_tier: high
urgency: high
state: answered
runbook_trigger: "MB-0026 autonomy.escalate: the limits N and M, the window and the backoff (security); reading `X-Forwarded-For` behind Caddy and configuring a trusted proxy, the first exception to RR-0010's never-read-forwarded-headers stance (security); a dependency such as `actix-governor` (platform-owner; answered here: none needed); limiting in Caddy instead (security); a new configuration variable (platform-owner, Q2). feature.md escalation path `crates/kauppa-web/src/routes/auth.rs`. Routed to security, with Q2's variable and Q4 to platform."
---
```

## Decision statement

Decide how MB-0026 throttles password guessing and registration on a public
deployment. That means:

- the limits;
- how the app identifies a client behind Caddy without letting clients pick
  their own key;
- where the limiter lives: in the app, in a crate, or in Caddy;
- how its memory stays bounded.

**Routing.** The CRP goes to security. Platform-owner signs off on the
configuration variable (Q2) and on confirming that no dependency is added
(Q4).

## Current state

Verified on `63da727` (the accepted briefs, on top of `main` `e63b4f1`). No
code was changed. The only build was a release run of `password`'s test to
time Argon2id.

**Auth handlers** (`crates/kauppa-web/src/routes/auth.rs`):

- **Login** (`auth.rs:39-60`): `find_by_email`, then `password::verify`,
  then `Identity::login`.
  - An unknown email returns at `auth.rs:46-48` **without running Argon2**.
    The body and status (401, "Invalid email or password.") match a wrong
    password, but the timing differs by one verification. That's an account
    oracle by timing, so P2's "same response" bar is met in content only
    (Q7).
- **Register** (`auth.rs:79-97`): `password::hash` runs first, on every
  attempt.
  - A duplicate email gives 409 with "email already registered: {email}"
    (`kauppa-core/src/repo/sqlite/user.rs:67-69`, `error.rs:49-50`). That is
    the oracle MB-0023 deals with (CRP-0012 Q4).
- No rate limiting anywhere. `security.md` → Rules (should): "Rate-limit
  `/auth/login` and `/auth/register` (per-IP + per-account)."

**Cost per guess** (`kauppa-core/src/password.rs:14-29`):

- `Argon2::default()` is Argon2id, m = 19 MiB, t = 2, p = 1
  (`argon2-0.5.3/src/params.rs:42,52,61`).
- One hash and two verifies take 0.04–0.05 s in release on the dev machine
  (Evidence bundle), so about 15 ms of CPU and 19 MiB of memory per
  operation. A small droplet vCPU is likely 2–3× slower.
- **It blocks the worker thread.** `password::verify` and `password::hash`
  are synchronous and run directly on the actix worker. actix starts one
  worker per available CPU by default, so a 1–2 vCPU droplet can run 1–2
  Argon2 operations at once. A login flood therefore slows *every* request,
  not just logins. The limiter must run **before** Argon2, and throttled
  requests must cost nothing but a map lookup.

**How the app sees the client today:**

- `HttpRequest::peer_addr()` is the TCP peer (`actix-web-4.18.2/src/request.rs:402`).
  Behind Caddy (MB-0022) that is always Caddy's container address.
- `ConnectionInfo::realip_remote_addr()` takes `Forwarded: for=`, then the
  **first** (left-most, client-supplied) `X-Forwarded-For` value, then the
  peer (`actix-web-4.18.2/src/info.rs:118,143-144`). Its own docs say not to
  use it for security (`info.rs:164-167`). It must not be the limiter's key.
- **RR-0010.** The cross-site check never reads `X-Forwarded-*`
  (`csrf/mod.rs:18`; RR-0010 → Decision).
- **But the request log span already reads them.** tracing-actix-web
  0.7.25's root span records, at INFO:
  - `http.client_ip = realip_remote_addr()` and `http.host = connection_info.host()`,
    which reads `X-Forwarded-Host` (`info.rs:137`);
  - `http.target = path_and_query`
  (`tracing-actix-web-0.7.25/src/root_span_macro.rs:106-110`).
- The formatters print span fields with every event (`telemetry.rs`). So
  every WARN or ERROR line already carries an unverified, client-chosen
  "client IP", and the full address at INFO. The brief forbids "full client
  addresses at INFO or above" for the new log lines. The existing span field
  sits outside that rule today, but conflicts with its intent (Q6).

**Configuration** (`config.rs:5-57`):

- `KAUPPA_*` via the `config` crate.
- `Settings` has no `deny_unknown_fields`, so a new optional field is
  backward compatible.
- `KAUPPA_PUBLIC_ORIGIN` is the precedent for an optional, validated-at-load
  setting (`config.rs:13-29`).

**Wiring and tests:**

- `AppState` holds `pool`, `templates` and `origin` (`state.rs`). Limiter
  state would be one more `Arc` there, so `routes/test_support.rs::test_state`
  (`test_support.rs:25-42`) builds one too.
- `TestRequest` has no peer address unless `.peer_addr(…)` is set
  (`request.rs:395-396`; `test/test_request.rs:182`). The limiter needs a
  defined key for "no peer".
- **Limits and the existing suite.** No existing test sends more than a
  handful of failed logins. Many tests call `login_request` successfully, and
  successes don't count (P4). So with limits scoped to one `AppState` (one
  per test), the existing suite needs no change.
  - The MB-0018 cross-site tests post to `/auth/login` and `/auth/register`.
    They are rejected by the guard before the handler, so they never touch
    the limiter.

**Deployment** (MB-0022):

- Caddy is the only public listener. The app has no published port, and the
  Compose network is internal.
- Caddy's `reverse_proxy` (v2.5 and later) sets `X-Forwarded-For` to the
  address of the client it sees. It **replaces** whatever the client sent,
  unless that client is in Caddy's own `trusted_proxies`. So, with no
  `trusted_proxies` in the Caddyfile, the header the app receives holds
  exactly one address, the real client's.
  - MB-0022's smoke test should confirm it: a spoofed header sent through
    Caddy doesn't change the limiter key.
- **Shared addresses.** A household usually shares one public IPv4 address
  (NAT), and mobile carriers put many subscribers behind CGNAT addresses. A
  per-client limit is therefore really per household or per carrier
  gateway, and must allow for several people typing passwords.

## Questions

### Q1 — Limits, window and backoff (security)

**Design inputs:**

- **Legitimate use:** a handful of accounts per household, logging in rarely
  (sessions persist), occasionally mistyping a password on a phone.
- **Registration:** a few times per invitation, plus typos.
- **Attacks:**
  - credential stuffing from many addresses (per-account limit);
  - focused guessing from one address (per-client limit);
  - Argon2 floods (both limits, and throttled requests skip Argon2).
- **Token guessing (MB-0023).** With 256-bit tokens, guessing invitation
  tokens is infeasible at any rate. The register limit's real job is to bound
  Argon2 cost, the duplicate-email oracle (CRP-0012 Q4), and account
  spraying with a leaked link.

**Proposal** (constants in `kauppa-web`, named in the RR):

| Limiter | Key | Counts | Limit | Window |
| --- | --- | --- | --- | --- |
| `LOGIN_PER_CLIENT` (P1) | client (Q2) | failed `POST /auth/login` | **N = 10** | **15 min** |
| `LOGIN_PER_ACCOUNT` (P2) | normalized email, existing or not | failed `POST /auth/login` | **M = 5** | **15 min** |
| `REGISTER_PER_CLIENT` (P3) | client | failed `POST /auth/register` (bad or missing invite, duplicate email, invalid input) | **R = 10** | **1 h** |

**Semantics:**

- **Fixed window per key.** The window starts at the key's first counted
  failure and resets when it ends. A key at its limit gets **429** with
  `Retry-After: <seconds until the window ends>`, rounded up. That holds even
  with the correct password (P1).
- **Throttled requests are cheap.** They don't count and don't extend the
  window, and they skip `find_by_email` and Argon2 entirely.
- **Successes don't count** (P4). They also don't reset the key, because
  resetting on success lets an attacker who owns one account interleave
  successful logins to clear the per-client counter.
  - **Alternative:** reset the per-account counter when that account logs
    in. That's friendlier, and safe because only the right password can do
    it. Recommendation: no reset in v1, for simplicity. The window is short.
- **Order in the login handler:**
  1. check both limiters, client and then account;
  2. look up and verify;
  3. on failure, record in both.
- **429 body.** The login page is rendered with "Too many attempts. Try again
  in N minutes.", the same for the client and account limiters. The message
  doesn't say which limit fired, so it doesn't reveal whether an account is
  being targeted.
  - For P2's oracle-safety, an unknown email is counted and throttled exactly
    like a known one. The limiter never consults the database.
- **No escalating backoff in v1.** Each window is independent. The brief asks
  not to lock a legitimate user out for long, and 15 minutes is the cap.
  - **Optional amendment:** after three consecutive throttled windows for the
    same key, double the window (cap 2 h). It slows sustained guessing
    against one account from 480/day to under 100/day, but it lengthens the
    lockout an attacker can impose on a victim.

**Effect:**

- **Focused guessing.** One address manages 10 failures per 15 minutes, or
  960 per day. One account, attacked from any number of addresses, gets 5 per
  15 minutes, or 480 per day.
  - Against a 12-character minimum (enforced only in the HTML today; see
    Q7), that's adequate for a household app.
- **Lockout DoS.** An attacker who knows an account's email can keep it
  throttled with 5 failures every 15 minutes. That is inherent to any
  per-account limit without a device-trust mechanism, which is out of scope.
  It is bounded: no admin action is needed, and it ends when the attack
  stops (non-goal: lockout that needs an admin).
  - Two softeners: count per account only failures from clients that aren't
    already throttled; or make M slightly higher (10). Recommendation: M = 5,
    and accept the risk.
- **Shared NAT and CGNAT.** 10 failures per 15 minutes per address leaves a
  family plenty of typos. A CGNAT gateway shared with an attacker could
  throttle a user for one window. Accepted.

**Recommendation:** N = 10 per 15 min, M = 5 per 15 min, R = 10 per hour,
with fixed windows, no escalating backoff, no reset on success, and 429 with
`Retry-After`.

### Q2 — Client identity behind Caddy (security; platform for the variable)

#### Option A — TCP peer address only

- Pros: unspoofable; no configuration; consistent with RR-0010.
- Cons: **behind Caddy every request has the same peer.** The per-client
  limit becomes a global one: 10 failed logins per 15 minutes across all
  users, and any stranger can lock everyone out of logging in. P1 and P3
  turn into a denial-of-service switch. That's unusable for MB-0022's only
  deployment shape.
- Risk: high (availability).

#### Option B — `X-Forwarded-For` from configured trusted proxies only (recommended)

**Setting.** A new optional `KAUPPA_TRUSTED_PROXIES`: comma-separated IP
addresses or CIDRs, e.g. `172.30.0.0/24`.

- It's parsed and validated at load, like `KAUPPA_PUBLIC_ORIGIN`. A
  malformed entry refuses to start.
- **Unset or empty means trust none**, so the key is the peer address, as in
  local development.
- The parser uses `std::net` with no crate: an address, plus a prefix length,
  masked against the peer as a `u32` or `u128`.

**Resolution** (P5):

1. Peer address not in the trusted set → the peer address is the client.
   `X-Forwarded-For` is ignored.
2. Peer trusted → take the last `X-Forwarded-For` header line and walk its
   comma-separated addresses **from the right**, skipping addresses in the
   trusted set. The first untrusted address is the client. With Caddy as the
   only hop, that's the one address Caddy wrote.
3. Peer trusted, but the header is missing, empty or unparsable, or every
   entry is trusted → fall back to the peer address. That's the shared
   bucket, which fails safe: it throttles Caddy as a whole, never letting a
   client pick a key. Log at WARN once per process ("forwarded-for missing
   from a trusted proxy"), which points at a misconfigured proxy.
4. More than one `X-Forwarded-For` header line → join them in order, then
   apply step 2. That matches how proxies append.

**What is never read:**

- `Forwarded`, `X-Real-IP` and `realip_remote_addr()`. Caddy writes
  `X-Forwarded-For`, and one header with one rule is easier to audit.
- `X-Forwarded-Host` and `X-Forwarded-Proto`. Those stay out of the app's
  decisions, as in RR-0010.

**Key normalization:**

- an IPv4 address as is;
- an IPv4-mapped IPv6 address converted to IPv4;
- **IPv6 keyed by its /64.** A single IPv6 subscriber typically holds a whole
  /64 and could otherwise rotate through 2^64 keys.

**Deploy side** (MB-0022):

- `compose.yaml` pins the internal network's subnet (`ipam.config.subnet`),
  so `KAUPPA_TRUSTED_PROXIES` can name it.
- The Caddyfile sets no `trusted_proxies`. Caddy then replaces any
  client-sent `X-Forwarded-For` (see Current state).
- These belong in MB-0022's operator guide and its smoke test.

**Assessment:**

- Pros:
  - real per-client limits behind Caddy;
  - unspoofable from outside: the app port isn't published, and if it ever
    were, direct clients are untrusted peers whose header is ignored;
  - explicit: no trust without configuration.
- Cons:
  - a new configuration variable;
  - CIDR parsing to write and test;
  - a wrong subnet silently falls back to the peer (shared bucket, which
    fails safe) or, if too broad, trusts other containers on the network.
    The Compose network holds only Caddy and the app.
- Risk: low-medium.

**The RR-0010 exception, and why it is narrow:**

- RR-0010 refuses forwarded headers because a mistake there would let a
  forged *origin* through, a cross-site write.
- Here the worst case of a wrongly trusted header is that a client picks its
  own per-client bucket. The per-account limit (M) still holds whatever the
  key, because it doesn't use the address.
- The RR should state the exception's scope:
  - only the limiter's client key reads `X-Forwarded-For`;
  - only from `KAUPPA_TRUSTED_PROXIES` peers;
  - the cross-site check is unchanged.
- **Proposed roll-back into `security.md`** (for the RR to apply; not
  edited here). Under Rules (must): "Client addresses for security decisions
  come from the TCP peer, or from `X-Forwarded-For` only when the peer is in
  `KAUPPA_TRUSTED_PROXIES` (right-most untrusted entry; RR-*); never from
  `realip_remote_addr()`." And the "Rate-limit…" line moves from should to
  must.

#### Option B′ — Trust a fixed number of hops (`KAUPPA_PROXY_HOPS=1`)

Take the N-th address from the right of `X-Forwarded-For`, whatever the peer.

- Pros: no address configuration.
- Cons: if the app's port is ever reachable without Caddy (a Compose mistake,
  local testing on a LAN), every client picks its own key. It trusts the
  header, not the peer.
- Not recommended.

#### Option C — Limit in Caddy instead (custom build with `caddy-ratelimit`)

- **What it takes.** A custom Caddy image built with `xcaddy` and the
  third-party `github.com/mholt/caddy-ratelimit` module, plus `rate_limit`
  zones in the Caddyfile keyed on `{remote_host}`.
- Pros: the client address is native, with no forwarded headers; floods stop
  before the app.
- Cons:
  - **It can't express the brief.** It counts requests, not *failed* logins,
    so successes count (P4 fails). It can't key on the form's email, so
    there's no per-account limit (P2 fails). It sends no rendered page.
  - **It can't be tested.** None of P1–P6's `cargo test` checks can run
    against it.
  - **More to maintain.** A custom Caddy build to keep updated, and a
    non-core plugin to vet: supply chain, versus Caddy's official image.
  - **No protection without Caddy.** Local and LAN installs get nothing.
- **As an extra outer layer** (a coarse requests-per-minute cap on `/auth/*`),
  it could complement B later. That would be an MB-0022 amendment, not this
  brief.
- Not recommended as the mechanism.

**Recommendation: B**, with `KAUPPA_TRUSTED_PROXIES` (CIDR list, empty by
default) and the scoped RR-0010 exception above. No Caddy plugin.

### Q3 — In-app limiter shape (security; data structure is `may_decide`)

**Shape** (no new dependency):

- one `AuthLimiter` in `kauppa-web` with three `FixedWindow` maps,
  `Mutex<HashMap<Key, Entry>>` from `std`;
- an injected clock (a trait, or a `now: Instant` parameter) so tests
  control time (validation plan);
- held in `AppState` as an `Arc` and built in `main.rs` and `test_state`;
- the handlers call `limiter.check(...)` and `limiter.record_failure(...)`.

**Not middleware.** P2 needs the form's email, and P4 needs the outcome.
Middleware would have to buffer and parse the body, and to guess the
outcome from the status. So the checks live in the two handlers, and the
brief's slow-mode trigger (routes outside `/auth`) doesn't fire.

**Locking.** The critical section is a map lookup and update, and is never
held across `.await`. A poisoned mutex is recovered with
`PoisonError::into_inner` (no `unwrap`; AGENTS.md → Errors). The counters
stay valid because each update is a single assignment.

**Recommendation:** as above.

### Q4 — `actix-governor` vs hand-rolled (platform)

**`actix-governor`** is not in the tree (`cargo tree`; not in the local
registry). Adding it brings `governor` and its dependencies, so it goes
through the dependency-bump runbook.

- **It counts the wrong thing.** It's middleware that counts *every request*
  to a scope, keyed by an extractor, so it can't count only failures (P1,
  P4).
- **It can't key per account.** That would need a custom extractor that
  reads the body (P2).
- **Its IP extractor trusts forwarded headers unconditionally.**
  `SmartIpKeyExtractor` reads `Forwarded`, `X-Forwarded-For` and
  `X-Real-IP`, the same trap as `realip_remote_addr()`. We'd write our own
  extractor anyway (P5).
- **Memory isn't bounded by itself.** `governor`'s keyed state needs
  periodic `retain_recent()` calls.
- **It's still GCRA.** That's a better smoothing algorithm than a fixed
  window, but for "N failures per window, then 429 with `Retry-After`", a
  fixed window is easier to reason about and to test with a fake clock.

**Hand-rolled:** about 150 lines plus tests, no supply-chain surface, and it
expresses P1–P5 directly.

**Recommendation: hand-rolled.** No dependency, so this question is
informational for platform.

### Q5 — Memory bounds (security)

- **Expiry.** Each entry stores `{ window_start: Instant, failures: u32 }`.
  An entry whose window has passed is treated as absent and is purged
  lazily.
- **Hard cap per map:** `MAX_KEYS = 10_000`.
  - **Size:** the key (an IPv4 address or IPv6 /64, or a 64-bit keyed hash
    of the email) plus the entry is under 64 bytes. With `hashbrown`
    overhead, that's at most about 1–2 MB per map and 3–6 MB in total.
  - **On insert into a full map:**
    1. Purge every expired entry (O(n), only when full).
    2. If it's still full, evict the entry with the oldest `window_start`.
- **Why evict rather than refuse.** Refusing new keys (fail closed) would let
  a flood of distinct addresses throttle every newcomer.
  - **Client map.** Eviction lets a flood of 10,000 or more distinct
    addresses within 15 minutes push out a tracked attacker's entry. But that
    attacker already has 10,000 addresses, and the per-account limit still
    holds.
  - **Account map.** The attacker chooses the emails. But each client can add
    only N = 10 new account entries per window before being throttled, so
    flushing one account's entry needs about 1,000 addresses.
  - P4's test fills a map past the cap and checks that it stays at the cap.
- **No plaintext emails in memory.** The account key is a 64-bit hash of the
  trimmed, ASCII-lowercased email, from a `RandomState` keyed once per
  process (`std`). The map holds no emails, so a heap dump or a debug print
  can't leak them.
  - Collisions (about 1 in 2^32 at 10,000 keys) merge two accounts' counters.
    That's harmless.
  - The normalization is deliberately looser than the login lookup, which
    is case-sensitive (`users.email` is `BINARY`). So `Pat@` and `pat@`
    share a counter.
- **Logging when full** (P6 says no other log line changes). Logging at WARN,
  once per window, when eviction starts ("auth limiter at capacity; evicting
  oldest") helps operators. But it's a new line beyond P6's wording.
  Recommendation: include it, and the RR should approve it explicitly.

**Recommendation:** cap 10,000 per map; lazy expiry; purge-then-evict-oldest;
hashed account keys; one capacity WARN per window.

### Q6 — Logging (security)

**Throttle line (P6).** At WARN:
`tracing::warn!(limiter = "login", key_kind = "client" | "account", retry_after_s, "auth attempt throttled")`.

- It never includes the email, the password or the address.
- The brief forbids full client addresses at INFO and above. A truncated
  prefix (IPv4 /24, IPv6 /48) could be added at WARN for diagnosis.
  Recommendation: omit it in v1.

**The existing request span.** As found in Current state, every log line
already carries `http.client_ip` from `realip_remote_addr()`. That's a full,
client-chosen address at INFO, and the throttle WARN line would inherit it.
Two choices:

- **(a) Leave the span alone.** Then the throttle line carries whatever
  address the request claimed. That reads like a violation of the brief's
  forbidden list in spirit, though the line itself doesn't log it.
- **(b) A custom `RootSpanBuilder` (recommended).** It sets `http.client_ip`
  to the limiter's resolved client key, truncated (/24 or /48), or omits it.
  It's the same `main.rs` change CRP-0012 Q5 proposes for path-only
  `http.target`, so both missions would share one builder.
  - Whichever MB lands second adopts it. Recommendation: MB-0023 introduces
    it, and MB-0026 adds the client-ip field.
  - It changes no session or cookie wiring.

**Recommendation: (b)**, coordinated with CRP-0012 Q5.

### Q7 — Interaction with MB-0023, and adjacent gaps (security)

**Register (P3).**

- The register handler checks `REGISTER_PER_CLIENT` before the invitation
  check and before `password::hash`, and records a failure for:
  - a missing, unknown, expired, revoked or used token;
  - a duplicate email;
  - invalid input.
- Whichever brief lands second wires the other's check into
  `register_submit`. Both edit that function and `routes::auth::tests`.
  **Ordering:** MB-0023 first (its RR changes the handler's shape), then
  MB-0026 on top. If MB-0026 lands first, MB-0023 inherits the check.
- **Invitation-token guessing** is infeasible at 256 bits regardless of R.
  The limit's value is Argon2 cost and the duplicate-email oracle
  (CRP-0012 Q4).
- **Admin portal POSTs** (`/admin/invitations`) aren't limited; they need an
  admin session. Not in scope.

**The login timing oracle** (`auth.rs:46-48`). An unknown email skips
Argon2, so P2's "the limiter isn't an account oracle" holds only for the
limiter's own response. The handler stays a timing oracle.

- **Fix:** verify the submitted password against a fixed dummy PHC hash.
  It's computed once at startup with `password::hash`, so `password.rs`
  isn't changed. That equalizes timing.
- **Cost:** unknown-email attempts now cost one Argon2, which the limiter
  bounds.
- Recommendation: do it in MB-0026. It's in `auth.rs`, it's the same
  "response mustn't differ" property, and it's small. The RR should list it,
  since it's a behaviour change on a trigger path beyond P1–P6's wording.

**Server-side password length.** `minlength="12"` exists only in
`register.hbs`; the handler accepts any length. That's out of scope for
MB-0026. Noted as a follow-up for MB-0023 or a later brief.

**Order with MB-0022.** MB-0022's order (MB-0023, MB-0025 and MB-0026 before
public DNS) stands. Q2 B needs MB-0022's `compose.yaml` to pin the subnet and
set `KAUPPA_TRUSTED_PROXIES`. Without that, the app falls back to the shared
peer bucket, which is safe but would throttle every user at once under
attack. That's a pre-DNS check for the operator guide.

## Recommendation and question

| # | Question | Recommendation |
| --- | --- | --- |
| Q1 | Limits | Login: N = 10 failures per client / 15 min; M = 5 failures per account / 15 min. Register: R = 10 failures per client / 1 h. Fixed windows, no escalating backoff, successes neither count nor reset, 429 + `Retry-After`, same page for both limiters |
| Q2 | Client identity | `KAUPPA_TRUSTED_PROXIES` (CIDR list, empty = trust none); right-most untrusted `X-Forwarded-For` entry only from trusted peers; IPv6 keyed by /64; fallback to peer; scoped RR-0010 exception, rolled into `security.md`; no Caddy plugin |
| Q3 | Shape | In-handler checks against one `AuthLimiter` in `AppState`; `std` `Mutex<HashMap>` fixed windows; injected clock |
| Q4 | Dependency | Hand-rolled; no `actix-governor` (wrong counting model, unsafe IP extractor) |
| Q5 | Memory | 10,000 keys per map; lazy expiry; purge-then-evict-oldest; account keys as keyed 64-bit hashes; one capacity WARN per window |
| Q6 | Logging | WARN `auth attempt throttled` with limiter and key kind only; custom `RootSpanBuilder` (shared with CRP-0012 Q5) so `http.client_ip` is the resolved, truncated key |
| Q7 | MB-0023 and gaps | Register limiter before the invite check and hashing; MB-0023 lands first; dummy-hash verify for unknown emails at login |

Crisp questions to answer:

1. (security) Q1: approve **N = 10 / 15 min, M = 5 / 15 min, R = 10 / 1 h**,
   fixed windows, no backoff, no reset on success? Or other numbers, or the
   doubling-window amendment?
2. (security, platform) Q2: approve **`KAUPPA_TRUSTED_PROXIES`** and the
   right-most-untrusted `X-Forwarded-For` rule as a scoped exception to
   RR-0010's stance? Or A, B′ or C?
3. (platform) Q4: confirm **no new dependency** (hand-rolled limiter).
4. (security) Q5: purge-then-evict-oldest at 10,000 keys, and the extra
   capacity WARN line?
5. (security) Q6: the custom root span replacing `http.client_ip` (with
   CRP-0012 Q5)?
6. (security) Q7: include the **dummy-hash verify** for unknown emails at
   login in this mission?

## Blast radius and rollback

**What can break:**

- **Login for everyone,** if the client key is wrong (everyone shares the
  Caddy bucket) and an attacker fills it. Q2's fallback makes a
  misconfiguration fail toward this rather than toward spoofable keys. The
  operator guide's check catches it.
- **Login for one account** while it is being attacked (Q1 lockout DoS,
  accepted).
- **Memory,** if the cap logic is wrong (P4's test).
- **Request log fields,** under Q6 (b).

**Who is affected:** anyone signing in or registering. No other route is
touched.

**Rollback plan:**

- Revert the commit. No schema change, no cookie or session change.
- A leftover `KAUPPA_TRUSTED_PROXIES` is ignored (no `deny_unknown_fields`).
- In an emergency, set the constants high and redeploy; no configuration
  knob for the limits is proposed.

## Minimal context

- `.agentic/missions/MB-0026-auth-rate-limiting.md`: P1–P6, `autonomy`,
  slow-mode trigger.
- `.agentic/resolutions/RR-0010-csrf-mechanism.md`: never reads
  `X-Forwarded-*`; one check in one place.
- `.agentic/mentorship/security.md`: threat 1, Rules (should) rate-limit,
  Logging, Escalation (`auth.rs`).
- `.agentic/missions/MB-0022-first-deployment.md`: Caddy as the only public
  listener, Compose internal network, its escalation on Caddy-side limiting.
- CRP-0012 (MB-0023; branch `agentic/mb-0023-invites`): Q4 duplicate-email
  oracle, Q5 custom root span, Q7 register response shape.

## Evidence bundle (Hardened)

All read on `agentic/mb-0026-rate-limit` at `63da727`.

- **No limiter or client-address code:** `grep -rn 'connection_info\|peer_addr\|realip' crates/`
  gives no matches.
- **Auth flow:** `routes/auth.rs:39-60` (login, early return at 46-48) and
  `79-97` (register, hash first at 85).
- **Argon2id cost:**
  - parameters: `argon2-0.5.3/src/params.rs:42` (m = 19 × 1024 KiB), `:52`
    (t = 2), `:61` (p = 1);
  - `cargo test --release -p kauppa-core --lib hash_then_verify` (one hash,
    two verifies) finished in 0.04–0.05 s over four runs on the dev machine
    (WSL2, 32 threads), about 15 ms per operation.
- **Library sources** (cargo registry):
  - actix-web 4.18.2: `src/info.rs:118` (`Forwarded: for=`), `:143-144`
    (first `X-Forwarded-For`), `:137` (`X-Forwarded-Host`), `:164-167`
    (the security warning); `src/request.rs:395-402` (`peer_addr`, `None` in
    tests); `src/test/test_request.rs:182` (`TestRequest::peer_addr`).
  - tracing-actix-web 0.7.25: `src/root_span_macro.rs:106-110`
    (`http.host`, `http.client_ip = realip_remote_addr()`, `http.target`).
- **Dependencies:** `cargo tree` shows no `governor`, `actix-governor`,
  `dashmap` or `ipnet`, and the cargo registry has no copy.
- **Configuration precedent:** `config.rs:13-29` (`KAUPPA_PUBLIC_ORIGIN`,
  optional and validated at load).
