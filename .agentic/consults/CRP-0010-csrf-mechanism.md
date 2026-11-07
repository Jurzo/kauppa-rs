# Consultation Request Pack — App-wide CSRF mechanism (MB-0018)

```yaml
---
id: CRP-0010
mission: MB-0018
owner: "@juuso-kalliomaki"
target_role: security
risk_tier: high
urgency: medium
state: open
runbook_trigger: "MB-0018 autonomy.escalate: \"choosing the mechanism (origin check, synchronizer or double-submit token, or both) — open a CRP before writing code\" (security); \"adding a configuration variable such as `KAUPPA_PUBLIC_ORIGIN`\" (platform-owner); \"any change to session middleware wiring in main.rs or to cookie attributes\" (security); \"promoting security.md's CSRF rule from should to must (Mentorship Pack change)\" (security); \"the mechanism seems to need a new dependency\" (platform-owner, answered here: no option needs one). feature.md escalation paths \"Session middleware in crates/kauppa-web/src/main.rs\" and `.agentic/{mentorship,runbooks,resolutions}/**`. RR-0002 constraint 6; RR-0008 constraint 1."
---
```

## Decision statement

Choose the mechanism that makes every state-changing request in `kauppa-web`
prove it came from the app's own pages (MB-0018 P1–P6). The answer also
settles how the app learns its public origin, and whether `security.md`'s
CSRF rule becomes a "must".

CRP-0008 already laid out the candidates and RR-0008 picked "app-wide, in
its own brief". This consult doesn't repeat that. It adds what the code
shows today and what each mechanism costs here.

## Current state

Verified on `origin/main` at `b87e553` (2026-11-07).

**Middleware stack** (`crates/kauppa-web/src/main.rs`):

- `TracingLogger` → `IdentityMiddleware::default()` → `SessionMiddleware`
  (`CookieSessionStore`, cookie `kauppa_sid`, `cookie_secure(cfg.cookie_secure)`,
  `cookie_http_only(true)`), then `Files::new("/assets", …)` and
  `routes::configure`.
- No CSRF, `Origin`, `Referer`, `Sec-Fetch-*` or `X-Forwarded-*` handling
  anywhere in `crates/` or `templates/`.
- `SameSite` isn't set explicitly. actix-session 0.10.1 defaults to
  `SameSite::Lax` (`actix-session-0.10.1/src/config.rs:385`), which is what
  `security.md` requires.

**Configuration** (`crates/kauppa-web/src/config.rs`, `.env.example`):

- `config::Environment::with_prefix("KAUPPA")`; fields `bind`,
  `database_url`, `log_level`, `session_secret`, `cookie_secure`, each with a
  default.
- There is no public-origin setting. A `public_origin` field would be read
  from `KAUPPA_PUBLIC_ORIGIN` with no new mechanism.

**Routes** (`crates/kauppa-web/src/routes/mod.rs`, attribute macros only, no
`web::resource`/`.route()`):

- 47 handlers: **34 `POST`**, 13 `GET`. No `PUT`, `PATCH` or `DELETE`.
  Plus `Files` at `/assets` (GET/HEAD only).
- Writes by scope:

  | Scope | `POST` routes |
  | --- | --- |
  | `/auth` | 3 (login, register, logout) |
  | `/lists` | 14 (incl. share, revoke, item edits by edit grantees) |
  | `/recipes` | 9 |
  | `/categories` | 4 |
  | `/favorites` | 6 |

- The 13 GET handlers render pages and fragments. Spot-checked; none writes
  (`ingredient_match` reads only). Safe methods stay unprotected, as the
  brief's non-goals say.

**Auth forms** (`routes/auth.rs`, `templates/auth/*.hbs`, `templates/layout.hbs`):

- `POST /auth/login` and `POST /auth/register` run **before any session
  exists**. They take `web::Form`, then `Identity::login`, which calls
  `session.renew()` (new cookie key, state kept with `CookieSessionStore`).
- `POST /auth/logout` needs an `Identity` and calls `id.logout()`. The default
  `LogoutBehaviour::PurgeSession` purges the whole session
  (`actix-identity-0.8.0/src/identity.rs`). Anything stored in the session,
  such as a token, is lost on logout.
- Login and register forms are plain `<form method="post" action=…>`. The
  logout form sits in `layout.hbs`'s header.
- Forged login or registration ("login CSRF": the victim is signed into the
  attacker's account and adds data to it) is in scope. So is forced logout,
  a nuisance.

**Templates and htmx:**

- 35 `<form method="post">` across 16 templates.
  - Every one except the three auth forms also carries `hx-post` with the same
    URL (progressive enhancement).
  - Fragments (`_list.hbs`, `_index.hbs`, `_section.hbs`, …) are rendered
    without the layout and swapped in by htmx.
- `<body hx-boost="true">`, so even the auth forms go through htmx when JS
  runs.
- htmx 2 (`unpkg.com/htmx.org@2.0.3`) sends requests with `XMLHttpRequest`.
  Browsers follow the Fetch spec for these, so a same-origin `POST` carries
  `Origin`. Same-origin plain form posts carry `Origin` in current browsers
  too.
- No `hx-headers` or `hx-vals` anywhere.
- **hx-boost caveat.** A boosted navigation swaps the body's *contents*; the
  `<body>` element and `<head>` are not re-rendered. So an `hx-headers` token
  on `<body>` would go stale after boosted login (if the token rotates) and
  after logout (session purged).
- `assets/` holds only `app.css` and `manifest.webmanifest`, no JS.

**Tests** (`routes/test_support.rs`, `routes/*/tests/`):

- `test_app!` wires `IdentityMiddleware` + `SessionMiddleware` +
  `routes::configure` only. No `TracingLogger`, no `Files`.
  - If the protection is a `.wrap()` added in `main.rs` alone, the route tests
    won't exercise it. It has to live in `routes::configure` (a scope-level
    wrap) or in one constructor both use.
- 36 `test_app!` uses; 69 `TestRequest::post()` calls and `login_request`
  uses across 15 test files (111 call sites in all).
- `TestRequest` sends no `Origin`. The connection info host defaults to
  `localhost:8080` (`actix-web-4.18.2/src/config.rs:292`).

**Dependencies** (`Cargo.toml`, `Cargo.lock`):

- `kauppa-web` already depends directly on `uuid` with `v4`, which is backed
  by `getrandom`, the OS CSPRNG, and gives 122 random bits.
- `rand`, `base64`, `sha2`, `hmac`, `subtle` and `url` are in `Cargo.lock`
  only as transitive dependencies. Using any of them directly is a manifest
  change, so it goes through the dependency-bump runbook.
- **No option below needs a new dependency.** The origin check is string
  comparison on a normalized origin. A token can be two `Uuid::new_v4()`
  values hex-encoded (244 bits), compared with a hand-written constant-time
  loop.

## Options and trade-offs

Options are scored against MB-0018's acceptance properties:

- P1: cross-site writes get 403;
- P2: same-origin writes succeed, behind a proxy too;
- P3: missing proof is rejected;
- P4: route-coverage guard;
- P5: share and revoke tests;
- P6: rejections are logged at `warn`.

### Option A — `Origin` check with `Referer` fallback, app-wide middleware

A single middleware (`wrap_fn` or a small `Transform`) on every non-safe
method. Every method except `GET`, `HEAD` and `OPTIONS` counts, so a future
`PUT`, `PATCH` or `DELETE` is covered automatically. It runs before routing,
so it covers unregistered paths too.

It accepts the request only when one of these matches the app's origin:

- `Origin` (scheme + host + port);
- or, when `Origin` is absent, the origin part of `Referer`.

`Origin: null`, an unparsable value, or neither header present → 403 (P3).

**The app's origin:**

- **`KAUPPA_PUBLIC_ORIGIN` set** (e.g. `https://kauppa.example.fi`):
  - normalized once at startup: lower-case scheme and host, default port
    dropped, no path;
  - rejected at startup if malformed;
  - compared exactly. `Host` and `X-Forwarded-*` are never consulted.
- **Unset — fallback options:**
  - **(i) Same host (recommended).** Compare the request's `Host` header
    with the authority in `Origin`/`Referer`. The scheme comes from
    `KAUPPA_COOKIE_SECURE`: `https` when true, `http` otherwise.
    - This is the standard same-origin check; an attacker can't set the
      victim browser's `Host`. `cargo run` works as before on any hostname
      (`localhost`, `127.0.0.1`, a LAN IP).
    - Log a startup `warn!` when `cookie_secure=true` and the origin is
      unset ("set `KAUPPA_PUBLIC_ORIGIN` behind a proxy").
  - **(ii) Refuse to start.** Strictest and makes the setting explicit, but
    breaks every existing `.env` and dev setup. `.env.example` would need a
    value, and `http://127.0.0.1:8080` there doesn't match a browser on
    `localhost:8080`.
  - **(iii) Derive from `KAUPPA_BIND`.** Wrong whenever the user browses by
    a name rather than the bind address. Not recommended.

**Coverage:**

- **Login, register, logout:** covered with no session and no template
  change. All three are plain or boosted posts from the app's own pages,
  which carry `Origin`.
- **Behind a reverse proxy:**
  - with `KAUPPA_PUBLIC_ORIGIN` set: correct whatever the proxy does to
    `Host`;
  - unset, behind a `Host`-preserving proxy (nginx `proxy_set_header Host
    $host`, Caddy's default): correct;
  - unset, behind a `Host`-rewriting proxy: every write gets 403, loudly,
    with a `warn` log naming the mismatch (P6). The fix is to set the
    variable.
  - `X-Forwarded-Host` is deliberately not trusted; it is client-settable
    unless a trusted proxy strips it.
- **Scripted clients and curl smoke tests** must send
  `-H 'Origin: <public origin>'`. That's one header, documented in the smoke
  steps. Non-browser clients aren't CSRF victims, but P3 asks for fail-closed
  anyway; Go 1.25's `CrossOriginProtection` lets header-less requests
  through, and this deliberately doesn't.

**Test-suite change:**

- `test_support` gains a default-proof helper. Either:
  - `TestRequest` builders such as `post(uri)`, used by the existing 69 call
    sites, that set `Origin: http://localhost:8080`; or
  - a test-only wrapper that injects it unless the request opts out.
- `test_app!` wires the protection the same way `main.rs` does.
- The P1/P3 tests then send a forged origin, `Origin: null`, a cross-site
  `Referer`, or nothing.
- The diff is mechanical (call-site renames), with no template or handler
  change.

**Assessment:**

- Pros:
  - no template, handler, session or cookie change;
  - no dependency;
  - covers pre-session forms naturally;
  - one place, easy to guard (P4);
  - matches the "origin check" `security.md` already names;
  - same-site attackers on a sibling subdomain are rejected, because their
    `Origin` differs.
- Cons:
  - relies on browsers sending `Origin` or `Referer`. Very old browsers that
    omit `Origin` on same-origin form posts, combined with a privacy setting
    that strips `Referer`, fail closed (403) rather than open. The app sets
    no `Referrer-Policy`, so the browser default
    (`strict-origin-when-cross-origin`) keeps same-origin `Referer`.
  - Needs the public-origin config (platform escalation).
  - "Behind a proxy" correctness depends on the operator setting the
    variable or preserving `Host`.
- Risk: medium. A wrong comparison blocks every write. It fails loudly, and
  the route suite catches it once tests send `Origin`.
- Cost / timeline: about half a day plus tests (P1–P6). Touches `main.rs`
  wiring, `config.rs`, `.env.example`, `routes/mod.rs` (or a new
  `routes/csrf.rs`) and `test_support.rs`.

### Option B — Synchronizer token in the session

How it works:

- A per-session random token is stored in the session.
- It's rendered as a hidden `_csrf` field in every form, and optionally sent
  in an `hx-headers` header.
- It's checked on every non-safe method.

Not a double-submit cookie. A sibling subdomain can toss a cookie for the
parent domain, which defeats double-submit. A token inside the encrypted
`CookieSessionStore` cookie isn't exposed to that.

**Coverage:**

- **Login and register:** need a token before any session exists.
  - The `GET /auth/login` and `/auth/register` handlers would have to create
    an anonymous session, so every anonymous visitor gets a `kauppa_sid`
    cookie. That's new behaviour, though no cookie attribute changes.
  - `Identity::login` keeps session state through `renew()`, so the
    pre-login token survives login. OWASP recommends rotating it there,
    which `auth.rs` (a security path) would have to do.
- **Logout:** purges the session, so the token goes too. The next page needs
  a fresh one.
- **Where the token travels:**
  - In `hx-headers` on `<body>` alone, it goes stale after boosted login and
    logout (see Current state). So it has to be a hidden field in all 35
    forms.
  - That means every fragment render needs the token in its context: about
    all 47 handlers, or a request-aware render wrapper.
  - Checking a form field in middleware means buffering and re-injecting the
    urlencoded body. That's doable in actix but fiddly. The alternative is an
    extractor in each handler, which is no longer "one place" and needs a
    source-scanning P4 guard.
- **Behind a reverse proxy:** independent of `Host` and headers; no config
  variable needed.
- **Scripted clients and curl:** must `GET` a page, keep the cookie, scrape
  the token, then `POST`. Smoke tests get noticeably more involved.
- **Test-suite change:**
  - every write test must first fetch a page (or a test hook must mint a
    session with a known token) and send it;
  - all 111 POST call sites change;
  - template tests must assert that every form carries the field.

**Assessment:**

- Pros:
  - the textbook defence;
  - doesn't depend on browser headers or on knowing the public origin;
  - works for non-JS clients via the hidden field.
- Cons:
  - largest diff: 35 forms, the render paths of about 47 handlers, the
    session lifecycle in `auth.rs`, and every route test;
  - creates sessions for anonymous visitors;
  - token rotation at login touches a security path;
  - body buffering in middleware;
  - a missed form fails closed and shows up only when a user submits it
    (P4 can enumerate routes, not forms).
- Risk: medium-high, given the number of touch points and the boost/stale
  token trap.
- Cost / timeline: two to three days.

### Option C — Both A and B (defence in depth)

Run the A check first, then require the B token.

- Pros: survives a browser that strips both `Origin` and `Referer`, and an
  operator who misconfigures the public origin. Each layer covers the
  other's failure mode.
- Cons: the sum of A's and B's costs. Two configurations to get right. Every
  test and smoke step must send both proofs. Little extra protection against
  the stated threats (cross-site, sibling-subdomain, no-`SameSite`
  browsers), because A already rejects all three by origin.
- Risk: as B.
- Cost / timeline: three days or more.

### Option D — Fetch Metadata (`Sec-Fetch-Site`)

D works only as an extra signal on top of A, never on its own.

- **Alone:** reject any non-safe request whose `Sec-Fetch-Site` isn't
  `same-origin`.
  - Simple, and it doesn't need the public origin at all.
  - But browsers send `Sec-Fetch-*` **only to potentially trustworthy
    origins** (HTTPS or localhost). A self-hosted install on plain HTTP over
    a LAN (`http://192.168.1.10:8080`) never receives it.
  - Older browsers don't send it either (supported since Chrome 76, Firefox
    90, Safari 16.4).
  - So D alone can't meet P2 and P3 for this app's self-hosting goal.
- **As a reject-only layer in A (D+A):**
  - if `Sec-Fetch-Site` is present and is `cross-site` or `same-site`,
    reject at once;
  - otherwise apply A's rule.
  - It never *accepts* on its own, so P3 stays fail-closed.
  - It adds a browser-asserted signal that doesn't depend on `Host` or on
    the configured origin. A sibling subdomain is `same-site` → rejected even
    when the operator left `KAUPPA_PUBLIC_ORIGIN` unset behind a
    `Host`-rewriting proxy that happens to match.
  - Cost: a few lines and two test cases.

## Recommendation and question

Recommended option: **A, with D as a reject-only layer.** That is:

- an app-wide `Origin`/`Referer` check on every non-safe method;
- `Sec-Fetch-Site: cross-site|same-site` rejected first;
- `KAUPPA_PUBLIC_ORIGIN` optional, with fallback (i) (same host, scheme from
  `KAUPPA_COOKIE_SECURE`) when unset.

**Because:**

- It meets P1–P6 with one control in one place.
- It covers login, register and logout with no session and no template
  change, and needs no dependency.
- It keeps the route-test diff mechanical.
- B's extra assurance (independence from browser headers) is small against
  this app's threats. B would cost the largest diff in the app's history: a
  new anonymous-session behaviour, a token-rotation change in `auth.rs`, and
  the hx-boost stale-token trap.
- C can still be added later behind the same middleware if a deployment ever
  needs it, without undoing A.

**Crisp question to answer:** Does MB-0018 implement **(A) an Origin/Referer
check**, **(A+D) the same with `Sec-Fetch-Site` as a reject-only layer**,
**(B) a session synchronizer token**, or **(C) both A and B**?

**Sub-questions** (MB-0018 Conceptual plan, step 1):

1. **Public origin.** Do you approve a new optional `KAUPPA_PUBLIC_ORIGIN`
   (platform-owner escalation)? It would be read in `config.rs`, validated
   and normalized at startup, and documented in `.env.example`. When it's
   unset, which fallback applies:
   - (i) same host as the request, scheme from `KAUPPA_COOKIE_SECURE`
     (recommended);
   - (ii) refuse to start;
   - (iii) derive it from `KAUPPA_BIND`?

   And is a single origin enough, or should it accept a comma-separated list
   (e.g. a hostname and a LAN IP)? Recommendation: single, with a list as a
   later amendment.
2. **`security.md` should → must.** Does the CSRF rule move from "Rules
   (should)" to "Rules (must)"? The RR would roll this back into the
   Mentorship Pack. Suggested wording: "Every non-safe request passes the
   app-wide cross-site check in one place (MB-0018); no route is exempt
   without an approved RR; safe methods stay side-effect free." It would also
   drop "double-submit cookie" from the wording, because cookie tossing from a
   sibling subdomain defeats it. Recommendation: yes. RR-0002 constraint 6 and
   RR-0008 constraint 1 already treat it as merge-blocking.
3. **Wiring.** Do you approve one middleware in the `App` wiring? It would be
   applied in `routes::configure` (or a shared constructor) so that
   `test_app!` exercises it, plus a matching line in `main.rs` if needed.
   Session middleware and cookie attributes stay unchanged
   (`SameSite=Lax` stays).

## Blast radius and rollback

**What can break:**

- **A or A+D:** every write in the app, if the comparison or the configured
  origin is wrong. The failure is loud: 403 everywhere, with a `warn` log per
  rejection.
- **B and C:** additionally any form that misses its token, and the
  login/logout flow through session renewal and purge.

**Who is affected:** every user who writes anything, including signing in.
Read-only pages are unaffected in every option.

**Rollback plan:**

- **A or A+D:** remove one middleware registration (one commit). The config
  field can stay unused.
- **B or C:** also revert the template, render-context and `auth.rs`
  changes.
- No schema change and no cookie-attribute change in any option.
- Operators rolling back don't need to unset `KAUPPA_PUBLIC_ORIGIN`.
  `Settings` has no `deny_unknown_fields`, so a leftover variable is ignored
  even if the field is removed.

## Minimal context

- `.agentic/missions/MB-0018-cross-site-request-protection.md`: P1–P6,
  `autonomy.escalate`, Conceptual plan step 1, slow-mode triggers ("login and
  register can't carry the proof without JavaScript" doesn't fire under A).
- `.agentic/consults/CRP-0008-share-csrf-origin-check.md`: the earlier
  options (its B and C are this consult's A and B).
- `.agentic/resolutions/RR-0008-share-csrf-deferral.md`: app-wide only, no
  partial controls; P8 → MB-0018 P5.
- `.agentic/resolutions/RR-0002-list-sharing-authz.md` constraint 6.
- `.agentic/mentorship/security.md`: threat #4, Rules (should), Escalation.
- Code: `crates/kauppa-web/src/main.rs`, `config.rs`, `routes/mod.rs`,
  `routes/auth.rs`, `routes/test_support.rs`, `templates/layout.hbs`,
  `templates/auth/{login,register}.hbs`.

## Evidence bundle (Hardened)

Everything here was read on branch `agentic/mb-0018-csrf-crp` from
`origin/main` `b87e553`. No code was changed and nothing was built.

- **Write routes:**
  `grep -rnE '#\[(get|post|put|patch|delete|route)\(' crates/kauppa-web/src | grep -v /tests/`
  gives 34 `post` and 13 `get`.
  `grep -rn 'web::resource\|\.route(' crates/kauppa-web/src` gives none.
- **No existing protection:**
  `grep -rn 'X-Forwarded\|Forwarded\|connection_info\|realip\|Origin\|Referer\|Sec-Fetch' crates/ templates/`
  gives no matches.
- **Forms:** `grep -rc '<form' templates` gives 35 forms in 16 files, all
  `method="post"`. `grep -rn 'hx-headers\|hx-vals' templates` gives none.
  The layout has `<body hx-boost="true">`.
- **Test call sites:** `grep -rn 'TestRequest::post()\|login_request(' crates/kauppa-web/src`
  gives 111 call sites (excluding `test_support`), in 15 files;
  `test_app!` has 36 uses.
- **Library behaviour** (from the cargo registry sources):
  - actix-session 0.10.1 defaults to `SameSite::Lax` (`src/config.rs:385`);
  - actix-identity 0.8.0: `login` calls `session.renew()`, and `logout`
    defaults to `PurgeSession`;
  - actix-web 4.18.2's default test host is `localhost:8080`
    (`src/config.rs:292`).
- **Dependencies:**
  - `Cargo.lock` has `rand` 0.8.8/0.10.3, `getrandom`, `base64`, `sha2`,
    `hmac`, `subtle` and `url`, all transitive only;
  - `crates/kauppa-web/Cargo.toml` already has `uuid` (workspace `v4`).
