# Resolution Record — Rate-limit login and registration: in-app fixed windows, trusted-proxy client key

```yaml
---
id: RR-0013
closes: CRP-0013
mission: MB-0026
decided_by: "@juuso.kalliomaki"
role: security
risk_tier: high
decided_at: 2026-11-08T18:16:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/mentorship/security.md
  - .agentic/missions/MB-0026-auth-rate-limiting.md
  - .agentic/consults/CRP-0013-auth-rate-limiting.md
---
```

## Decision and outcome

Approve CRP-0013's recommendation on every question: a **hand-rolled,
in-app limiter** with three fixed-window maps, consulted inside the login and
register handlers **before any database or Argon2 work**, keyed per client
by the TCP peer or, only from configured trusted proxies, by the right-most
untrusted `X-Forwarded-For` entry.

Sub-answers:

1. **Limits (Q1, security).** Constants in `kauppa-web`:

   | Constant | Key | Counts | Failures | Window |
   | --- | --- | --- | --- | --- |
   | `LOGIN_PER_CLIENT` (P1) | client (sub-answer 2) | failed `POST /auth/login` | **10** | **15 min** (900 s) |
   | `LOGIN_PER_ACCOUNT` (P2) | normalized email, existing or not | failed `POST /auth/login` | **5** | **15 min** (900 s) |
   | `REGISTER_PER_CLIENT` (P3) | client | failed `POST /auth/register` (missing, unknown, expired, revoked or used invitation; duplicate email; invalid input) | **10** | **1 h** (3600 s) |
   | `MAX_KEYS` (P4) | — | entries per map | **10_000** | — |

   - **Fixed window per key**, starting at the key's first counted failure.
     A key at its limit gets **429** with `Retry-After` = seconds until its
     window ends, rounded up, even with the correct password.
   - **Throttled requests are free.** They don't count, don't extend the
     window, and skip `find_by_email`, the invitation check and Argon2.
   - **Successes neither count nor reset** any counter (P4).
   - **No escalating backoff.** Each window is independent; 15 minutes is
     the longest login lockout.
   - **Login order:** check the client limiter, then the account limiter;
     look up and verify; on failure, record in both.
   - **429 page.** Login re-renders with "Too many attempts. Try again in N
     minutes." (N = `Retry-After` in minutes, rounded up), the same text for
     both limiters. Register re-renders its page with the same message and
     keeps the hidden `invite` field.
   - An unknown email is counted and throttled exactly like a known one; the
     limiter never consults the database (P2).
2. **Client identity (Q2 B, security; platform for the variable).**
   - **New optional setting `KAUPPA_TRUSTED_PROXIES`:** comma-separated IP
     addresses or CIDRs (e.g. `172.30.0.0/24`), parsed with `std::net` at
     load in `config.rs`, documented in `.env.example`. A malformed entry
     refuses to start. Unset or empty means trust none.
   - **Resolution (P5):**
     1. peer not trusted → the peer address; `X-Forwarded-For` is ignored;
     2. peer trusted → join all `X-Forwarded-For` header lines in order, walk
        the comma-separated entries from the right, skip trusted addresses;
        the first untrusted address is the client;
     3. peer trusted but the header is missing, empty or unparsable, or
        every entry is trusted → the peer address (the shared bucket), with
        one WARN per process: "forwarded-for missing from a trusted proxy";
     4. no peer address at all (e.g. `TestRequest` without `.peer_addr`) →
        one fixed "unknown peer" bucket, which fails safe the same way.
   - **Never read:** `Forwarded`, `X-Real-IP`, `realip_remote_addr()`,
     `X-Forwarded-Host`, `X-Forwarded-Proto`.
   - **Key normalization:** IPv4 as is; IPv4-mapped IPv6 → IPv4; other IPv6
     keyed by its /64.
   - **Narrow exception to RR-0010's stance.** Only the limiter's client key
     reads `X-Forwarded-For`, and only from a `KAUPPA_TRUSTED_PROXIES` peer.
     The cross-site check (RR-0010) is unchanged and still reads no
     `X-Forwarded-*` header.
   - Not approved: A (peer only; one global bucket behind Caddy), B′
     (`KAUPPA_PROXY_HOPS`), C (Caddy `caddy-ratelimit`).
3. **Shape (Q3, security; structure is `may_decide`).** One `AuthLimiter` in
   `kauppa-web`, three `std::sync::Mutex<HashMap<Key, Entry>>` maps, an
   injected clock, held in `AppState` as an `Arc` and built in `main.rs` and
   `test_support::test_state`. The handlers call it; it is not middleware.
   The lock is never held across `.await`; a poisoned lock is recovered with
   `PoisonError::into_inner`, never `unwrap`.
4. **Dependency (Q4, platform).** None. No `actix-governor`, `governor`,
   `dashmap` or `ipnet`; no manifest change.
5. **Memory (Q5, security).**
   - Each entry is `{ window_start: Instant, failures: u32 }`; an entry past
     its window counts as absent and is purged lazily.
   - **At most `MAX_KEYS` = 10,000 entries per map.** On insert into a full
     map: purge every expired entry, then, if still full, evict the entry
     with the oldest `window_start`.
   - **Account keys are hashes:** a 64-bit hash of the trimmed,
     ASCII-lowercased email from a `std` `RandomState` built once per
     process. No email is held in memory.
   - **Approved extra log line:** one WARN per map per window when eviction
     starts, "auth limiter at capacity; evicting oldest", with the limiter
     name and no key.
6. **Logging (Q6 (b), security).**
   - Throttle line (P6), WARN:
     `tracing::warn!(limiter = "login" | "register", key_kind = "client" | "account", retry_after_s, "auth attempt throttled")`.
     No email, password, address or address prefix.
   - **Root span.** The custom `RootSpanBuilder` (introduced by MB-0023 for
     path-only `http.target`, RR-0012) sets `http.client_ip` to the
     limiter's resolved client key, truncated to IPv4 /24 or IPv6 /48,
     instead of `realip_remote_addr()`. If MB-0026 lands first, it introduces
     the builder with this field only.
7. **MB-0023 interplay and the login timing fix (Q7, security).**
   - `register_submit` checks `REGISTER_PER_CLIENT` before the invitation
     check and before `password::hash`, and records every failure listed in
     sub-answer 1. MB-0023 lands first; whichever lands second wires the
     other's check.
   - **Dummy verify.** For an unknown email, login runs
     `password::verify(submitted, DUMMY_HASH)` against a fixed PHC string
     made once per process with `password::hash`, then returns the same 401.
     `password.rs` is unchanged. Unknown-email attempts now cost one Argon2,
     bounded by the limiter.

Covers:

- MB-0026's implementation PR, with P1–P6 as written and the three log
  changes below approved beyond P6's "no other line changes":
  - the capacity WARN (sub-answer 5);
  - the once-per-process trusted-proxy WARN (sub-answer 2);
  - `http.client_ip` in the root span (sub-answer 6);
- the `KAUPPA_TRUSTED_PROXIES` setting and its `.env.example` entry;
- the dummy-hash verify at login;
- the `security.md` change below.

Does not cover:

- rate limiting any route besides `POST /auth/login` and
  `POST /auth/register` (the admin portal's POSTs need an admin session);
- the doubling-window backoff amendment;
- resetting the per-account counter on a successful login;
- the per-account softeners (counting only unthrottled clients, M = 10);
- an address prefix in the throttle line;
- limiting in Caddy, now or as an outer layer (an MB-0022 amendment if
  wanted);
- configuration knobs for the limits;
- server-side password length validation (a follow-up for MB-0023 or a later
  brief);
- any change to password hashing, sessions or cookies.

## Rationale and trade-offs

- **In-app fixed windows (Q1, Q3, Q4) — chosen.**
  - They count *failures* and key per account, which P1, P2 and P4 need;
    neither middleware nor Caddy can.
  - Throttled requests cost a map lookup, before Argon2 blocks a worker.
  - A fixed window is easy to reason about and to test with a fake clock;
    about 150 lines, no supply-chain surface.
  - Accepted: an attacker who knows an email can keep that account throttled
    for 15-minute windows; a CGNAT gateway shared with an attacker can
    throttle a user for one window.
- **`actix-governor` — not chosen.** It counts every request, can't key per
  account, its IP extractor trusts forwarded headers unconditionally, and its
  keyed state needs manual pruning.
- **Trusted-proxy `X-Forwarded-For` (Q2 B) — chosen.** Real per-client limits
  behind Caddy, unspoofable from outside, and no trust without configuration.
  A wrong subnet falls back to the shared bucket, which fails safe. The worst
  case of a wrongly trusted header is a client picking its own per-client
  bucket; the per-account limit still holds. That is why the exception to
  RR-0010 can be this narrow.
- **A (peer only) — not chosen.** Behind Caddy it is one global bucket: any
  stranger could lock everyone out.
- **B′ (hop count) — not chosen.** It trusts the header rather than the peer,
  so a reachable app port lets every client choose its key.
- **C (Caddy) — not chosen.** It can't express P2 or P4, can't be tested
  with `cargo test`, needs a custom Caddy build with a third-party plugin, and
  protects nothing without Caddy.
- **Evict rather than refuse (Q5) — chosen.** Refusing new keys at capacity
  would let a flood of addresses throttle every newcomer. Flushing a tracked
  entry needs thousands of addresses, and the per-account limit still holds.
- **Root span client key (Q6 b) — chosen.** Every log line today carries a
  client-chosen, full address from `realip_remote_addr()`. Not chosen: (a)
  leaving it, which keeps that in the throttle line's context.
- **Dummy verify (Q7) — chosen.** Removes the login timing oracle with no
  `password.rs` change, on the same "response mustn't differ" property.

## Approvals and authority

Approved by security-owner @juuso.kalliomaki; `KAUPPA_TRUSTED_PROXIES`
(sub-answer 2) and no new dependency (sub-answer 4) by platform-owner
@juuso.kalliomaki. Risk tier high: record any co-approver here.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0013-auth-rate-limiting.md`
- Mission Brief: `.agentic/missions/MB-0026-auth-rate-limiting.md`
- Related: RR-0010 (never reads `X-Forwarded-*`; narrowed here for the
  limiter key only); CRP-0012 / RR-0012 (MB-0023: register handler shape,
  path-only root span, duplicate-email oracle); MB-0022 (Caddy, Compose
  network).
- Evidence links: CRP-0013 → Current state and Evidence bundle:
  - the early return before Argon2 for unknown emails (`auth.rs:46-48`);
  - Argon2id m = 19 MiB, t = 2, p = 1, about 15 ms per operation in release,
    synchronous on the actix worker;
  - `realip_remote_addr()` takes the left-most `X-Forwarded-For`
    (`actix-web-4.18.2/src/info.rs:143-144`) and warns against security use;
  - tracing-actix-web 0.7.25 logs `http.client_ip` from it at INFO;
  - `peer_addr()` is `None` in `TestRequest` unless set;
  - no limiter crate in `cargo tree`.

## Constraints and follow-on rules (Hardened)

1. **Limit before cost.** The limiter check runs before any database lookup,
   invitation check or Argon2 call in both handlers. A throttled request does
   no other work.
2. **Client addresses for security decisions** come from the TCP peer, or
   from `X-Forwarded-For` only when the peer is in `KAUPPA_TRUSTED_PROXIES`
   (right-most untrusted entry). Never from `realip_remote_addr()`,
   `Forwarded` or `X-Real-IP`. Any other use of a forwarded header needs an
   RR.
3. **RR-0010 is unchanged.** The cross-site check never reads a forwarded
   header; this exception doesn't extend to it.
4. **No oracle.** The limiter treats unknown and known emails alike and never
   reads the database; the 429 page doesn't say which limit fired; login
   runs one Argon2 verify whether or not the account exists.
5. **Bounded memory.** At most `MAX_KEYS` entries per map; no email or
   password in limiter memory or limiter logs.
6. **Constants, not configuration.** Changing a limit, window or `MAX_KEYS`
   is a reviewed code change on a security trigger path, citing this RR.
7. **Tests.** Successful logins in the existing suite don't count, and each
   test has its own `AppState`, so no existing test needs changing. Limiter
   tests use the injected clock, never `sleep`.
8. **Coordination.**
   - MB-0023 lands first. If MB-0026 splits `routes/auth.rs` into
     `routes/auth/` first, it carries RR-0012's trigger-path roll-back
     (`feature.md`, `check_triggers.py`, `CODEOWNERS`, `security.md`
     Escalation) so `routes/auth/` stays a trigger path.
   - MB-0022 follow-ups, before public DNS: `compose.yaml` pins the internal
     network's subnet and sets `KAUPPA_TRUSTED_PROXIES` to it; the Caddyfile
     sets no `trusted_proxies`; the smoke test shows a spoofed
     `X-Forwarded-For` sent through Caddy doesn't change the key; the
     operator guide names the shared-bucket fallback. Record these in
     MB-0022's Continuity Pack.

## Roll-back into the system (Hardened)

To be applied in MB-0026's implementation PR:

- `.agentic/mentorship/security.md`:
  - remove "Rate-limit `/auth/login` and `/auth/register` (per-IP +
    per-account)." from Rules (should);
  - add to Rules (must): "**Rate limiting.** Failed `POST /auth/login`
    attempts are limited per client and per account, and failed
    `POST /auth/register` attempts per client, before any database or
    Argon2 work (MB-0026, RR-0013). The limits are constants named in
    RR-0013."
  - add to Rules (must): "**Client addresses.** Client addresses for
    security decisions come from the TCP peer, or from `X-Forwarded-For`
    only when the peer is in `KAUPPA_TRUSTED_PROXIES` (right-most untrusted
    entry; RR-0013); never from `realip_remote_addr()`."
- `MB-0026`:
  - `resolutions` gains `RR-0013`;
  - P1's "constants named in the RR" points at RR-0013;
  - P6's "no other log level or line changes" gains "except the lines
    RR-0013 approves".
- `CRP-0013`: `state: open` → `answered`.

## Supersession and history

Status: active once signed. Supersedes nothing. Narrows RR-0010's
never-read-forwarded-headers stance for the limiter's client key only;
RR-0010's decision is unchanged.
