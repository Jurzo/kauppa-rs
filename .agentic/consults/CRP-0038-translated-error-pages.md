# Consultation Request Pack — Translated HTML error pages: the mechanism, the request's language and theme, the cross-site 403, signing in from a 401, htmx

```yaml
---
id: CRP-0038
mission: MB-0056
owner: "@juuso-kalliomaki"
target_role: security
risk_tier: medium
urgency: low
state: open
runbook_trigger: "MB-0056 autonomy.escalate (mechanism, request state, cross-site 403 → security; signed out, htmx, framework answers → owner); feature.md escalation triggers `crates/kauppa-web/src/routes/auth/`, the RR-0010 wiring (`routes/mod.rs`, `csrf::tests::coverage`) and `.agentic/resolutions/**` (RR-0023 and RR-0025 amended)"
---
```

## Decision statement

Decide, for MB-0056: how error answers become pages and where that code
sits (security); how the page learns the request's language, theme and
signed-in state (security); how the cross-site 403 gets the same page as
any other 403 (security); what a signed-out user can do from a 401 page
(owner, security); what htmx does with an error page, which amends
RR-0025 (owner); which framework answers are in scope (owner); and the
brief amendments that follow from the trial below.

## Terms

- **Error page**: the HTML page this mission adds, rendered with the
  layout, for an answer with a 4xx or 5xx status.
- **Plain error**: an error answer that isn't a page today: the
  `WebError` bodies (`not found`, …), the cross-site 403, and the
  framework's own answers (empty 401, 404 and 405, parse errors).
- **Handler page**: a 4xx answer a handler renders as HTML on purpose,
  such as the login form's 401 or a form's 400 with the reason. These stay
  as they are.
- **The scope**: the catch-all `web::scope("")` in `routes::configure`
  that wraps every route in `csrf::guard` (RR-0010).

## Current state, checked by a trial

Checked on `main` at `d918d47` (2026-11-14, after MB-0048's #92). The trial
ran in a throwaway worktree and was then deleted; nothing of it is
committed. It added a `from_fn` middleware around the scope (outside
`csrf::guard`), a one-file page template, two catalog keys, and a probe
test that sent one request per error source through `test_app!` and
`bare_test_app!`. Results in the Evidence bundle.

What the trial established:

1. **One middleware around the scope reaches every plain error** the brief
   lists: `WebError` answers, the `Identity` extractor's 401, the
   cross-site 403, the router's 404 for an unknown URL, and the path
   extractor's error. No `default_service` is needed: the scope's own
   not-found answer passes through the scope's middleware.
2. **The test app needs no change.** `wired_app!` calls
   `routes::configure`, so the middleware comes with it. `main.rs` doesn't
   change either. `/assets/` sits outside the scope, so its 404 and 405 are
   untouched (RR-0011, RR-0029), as the brief's non-goal asks.
3. **`Ui` works from the middleware as it is.** The middleware extracts
   `Ui` and `Identity` from the request with `FromRequest`;
   `i18n/request.rs` doesn't change. Finnish by `Accept-Language`, and
   the `kauppa_theme` cookie's dark choice, both came through.
4. **The Content-Security-Policy and `Vary` headers are present on every
   page**, because `csp::headers` stays outermost.
5. **Two corrections to the brief's table.** A malformed id in a URL
   (`/lists/not-a-uuid`) answers **404**, not 400 (actix-web's path
   extractor maps a path error to 404, `text/plain`). A wrong method on a
   route (`GET /auth/logout`) answers **405** with an empty body, which
   the brief's table doesn't list.
6. **A trap: holding a clone of the request across the inner call
   panics.** actix-web's router needs sole ownership of the request
   (`Rc::get_mut(...).unwrap()` in `HttpRequest::push_resource_id`,
   actix-web 4.18.2 `src/request.rs:197`). The first trial cloned the
   request before calling the inner service, and at least 59 tests failed
   (the listing was cut there; the one examined panicked as above). The
   middleware must read the request back from the response instead
   (`ServiceResponse::request()`), and so can't make a page when the inner
   service returns `Err` rather than a response; in the trial no path did.
7. **Which answers to rewrite needs a rule.** With "every 4xx or 5xx that
   isn't `text/html`", `/healthz`'s 503 (deliberately `text/plain`, for
   monitors) became a page. The rule "a 4xx or 5xx that **carries an
   error** (`ServiceResponse::response().error()`) **or has no
   `Content-Type`**" rewrote every plain error above and left `/healthz`
   and every handler page alone: the 165 other existing tests passed,
   including
   the login, register and invitation forms' 4xx pages (RR-0025).
8. **What failed, and why** (6 of the 171 existing tests, with the
   narrower rule):

   | Test | Why | What the mission does |
   | --- | --- | --- |
   | `csrf::tests::cross_site::rejects_cross_site_writes`, `::missing_proof_is_rejected` | pin `forbidden` | rewritten to the page, status kept (P6) |
   | `routes::admin::tests::access::admin_only_non_admin_gets_404` | pins `not found` | same |
   | `routes::theme::tests::rejects_other_values` | its "input not echoed" check reads the whole body, and the layout's theme switch says "Dark" | check the page's main content, not the layout (P6) |
   | `csrf::tests::coverage::every_write_route_is_covered` | pins `routes::configure`'s source text exactly | expected text gains the one `.wrap` (Q1) |
   | `i18n::tests::catalog_has_no_unused_keys` | the trial built key names at run time | real keys are literals in the template (no change) |

   `error::tests::responses_unchanged` passed: it calls
   `error_response()` directly, and `error.rs` doesn't change.

## Options and trade-offs

### Q1: the mechanism and where it sits (security)

#### Option A — a `from_fn` middleware around the scope, outside `csrf::guard` (recommended)

`routes::configure` becomes:

```rust
web::scope("")
    .wrap(from_fn(crate::csrf::guard))
    .wrap(from_fn(errors::pages))   // outer: sees the guard's 403 too
    .configure(protected)
```

`routes/errors.rs` holds the middleware. It calls the inner service, and
if the answer is a 4xx or 5xx that carries an error or has no
`Content-Type` (trial point 7), renders the error page for that status
and returns it with the **same status**; otherwise it passes the answer
through untouched.

- Pros: reaches every plain error inside the scope (trial points 1, 5);
  no change to `main.rs`, the test app, `error.rs` or `csrf/`; `/assets/`
  excluded by position; one place to read.
- Cons: reverses RR-0023 sub-answer 10's "no middleware is added around
  the RR-0010 scope" (that sentence was the fallback if MB-0035 landed
  before an error-page design; this is that design). Changes the text
  `csrf::tests::coverage` pins for `configure`.
- Risk: a page could hide something the plain body showed; nothing in
  the plain bodies is more than a status word or the `Invalid`/`Conflict`
  reason, which P4 checks. Order matters: the middleware must stay
  outside the guard (or the 403 isn't seen) and inside the identity and
  session middleware (or `Identity` isn't available).
- Cost: one new module, one line in `configure`.

Details the RR would fix:

1. The rewrite rule is trial point 7's, exactly.
2. The original answer's headers that aren't about the body
   (`Content-Type`, `Content-Length`) are kept: for example `Allow` on a
   405, if actix-web sets one. The page adds `Content-Type: text/html;
   charset=utf-8` and `Vary: Cookie, Accept-Language`.
3. The middleware never holds a clone of the request across the inner
   call (trial point 6). If the inner service returns `Err`, it passes the
   error on unchanged.
4. If rendering fails, the original answer goes out as it was (P5), and
   the failure is logged at ERROR with the template name, not the error's
   detail. No other log line changes (MB-0020's lines stay).
5. `csrf::tests::coverage`'s expected `configure` text gains the
   `.wrap(from_fn(errors::pages))` line, after the guard's. Its other
   rules don't change. This is the brief's forbidden "weaken" only if you
   say so; it adds one line and removes no check.

#### Option B — an app-wide middleware in `main.rs` and `wired_app!`

- Pros: also covers answers made outside the scope.
- Cons: the only things outside the scope are `/assets/` (whose 404 and
  405 must *not* change, so it needs a path exclusion) and the session,
  identity and CSP middleware. Touches `main.rs` and the test app's
  wiring, which must stay identical (RR-0026 constraint 2). More change for
  nothing gained.
- Risk: an exclusion by path is easy to get wrong.

#### Option C — actix-web's `ErrorHandlers` at the same place as A

- Same placement and effect as A, through `ErrorHandlers::default_handler`.
- Cons: its handler gets the response after the fact, as in A, but the
  rule in point 7 must be written into each handler anyway, and the async
  form (`ErrorHandlerResponse::Future`) is harder to read than a
  `from_fn`.

#### Option D — per handler (rejected)

A handler can't reach the framework's 401, 404 and 405, and every handler
would need the request. It doesn't meet P1.

### Q2: language, theme and signed-in state (security)

#### Option A — extract `Ui` and `Identity` in the middleware (recommended)

The middleware calls `Ui::from_request` and `Identity::from_request` on
the response's request (trial point 3). "Signed in" means the identity
gives a user id that parses as a UUID, as `request::current_user` does.

- Pros: no change to `i18n/request.rs` (a trigger path); one way to pick
  the language, as for every page.
- Cons: a signed-in user's error page reads `users.language` once more,
  since the handler's `Ui` (if it had one) isn't kept. Signed-out errors
  don't touch the database. That is a partial hit on the brief's slow-mode
  trigger ("a second database read on every error"): it's one indexed
  read per signed-in error, and errors are rare.
- Risk: if the read fails, the page can't be rendered and the original
  answer goes out (P5).

#### Option B — the `Ui` extractor stores itself in the request's extensions

- Pros: no second read when the handler already extracted `Ui`.
- Cons: edits `i18n/request.rs` (trigger path) and makes every request
  carry the value; saves one read on a rare path.

### Q3: the cross-site 403 (security)

#### Option A — leave `csrf/` as it is (recommended)

The guard's plain 403 has no `Content-Type`, so the middleware (outside
the guard) rewrites it by the same rule and the same code as a
`WebError::Forbidden`. Both pages depend only on the status and the
request (language, theme, signed in), so for the same request they're
the same bytes. P4's test compares them byte for byte.

- Pros: no change in `csrf/`; the guard's comment ("the same plain answer
  as any other 403") stays true of its fallback, and the page is the same
  for both.
- Risk: someone later makes the page depend on why the request failed.
  P4's comparison test catches it.

#### Option B — the guard returns `WebError::Core(Forbidden)`

- Cons: `error_response()` would add a second WARN line to every
  cross-site rejection (it logs "request error"), which the brief's
  non-goals rule out; and it edits `csrf/` for no gain.

### Q4: signed out (owner, security)

Today, a signed-in page opened after the sign-in ended (RR-0026's 30 idle
days) shows an empty 401.

#### Option A — the sign-in form in the 401 page, returning to the page asked for (recommended)

The 401 page shows the login form (the same form as `/auth/login`, made a
partial so it's written once). For a GET, the form carries a hidden
`return` with the asked-for path; `login_submit` sends a successful
sign-in there through `language::same_site_path` (already used for the
language and theme switches: a path starting with one `/`, no `\`, no
control characters), and to `/lists` otherwise. For a POST that got a 401
(a form sent after the sign-in ended), `return` is left out, because a GET
of a POST path doesn't show what the user was doing.

- Pros: status stays 401 (brief non-goal); one step to get back.
- Cons: changes `login_submit` and the login template (auth trigger
  paths). The 401 page then holds one of RR-0025's three
  `hx-target-4xx` forms; that's the same form, so constraint 2 holds.
- Risk: an open redirect, if `same_site_path` misses a case. It is the
  validator the language and theme switches already depend on; the
  mission adds tests for `//host`, `/\host`, `https://host` and an
  encoded `%2F%2Fhost` through the login.

#### Option B — redirect to the sign-in page

- Cons: changes the status from 401 to 303, which the brief's non-goals
  rule out, and an htmx request follows the redirect silently and swaps
  the login page into its target.

#### Option C — the form in the 401 page, always to `/lists` afterwards

- Pros: no change to where login sends the user; no redirect surface.
- Cons: the user has to find their page again.

### Q5: htmx (owner)

Today a boosted link or form that gets a 4xx or 5xx does nothing visible:
htmx doesn't swap errors by default, and RR-0025 limits
`hx-target-4xx` to three forms and keeps `responseHandling` out of the
config.

#### Option A — the error page tells htmx to replace `<body>` (recommended)

For a request with `HX-Request: true`, boosted or not, the error page also
sends `HX-Retarget: body` and `HX-Reswap: innerHTML`. The vendored
response-targets extension (2.0.4, on `<body>` since RR-0025) swaps any
non-200 answer that carries `HX-Retarget`
(`responseTargetPrefersRetargetHeader`, default true). The trial checked
the header is sent on a boosted request.

- Pros: no change to templates, the extension, the htmx config or
  `hx-target-*`. Only answers the middleware made get the header;
  handler pages and the three forms behave as today. A fragment request
  (`hx-post` into part of the page) gets the whole page in `<body>`,
  never inside its own target (P3), so a user whose list was unshared or
  whose sign-in ended sees why their click did nothing.
- Cons: amends RR-0025 constraint 3 and sub-answer 3 ("no `HX-Retarget`
  on non-200 responses without an RR"): this RR would be that RR, for
  error pages only. A fragment request's page replaces the page the user
  was on; what they typed in an open form is lost, as it would be on any
  full reload.
- Risk: not measured, only read from the code: whether htmx pushes the
  failed URL into history on a boosted GET, and whether the `<title>`
  updates. P7 checks both in a browser.

#### Option B — as A, for boosted requests only

Fragment requests stay as today: nothing happens.

- Pros: a smaller change to RR-0025.
- Cons: the most common case after a share is revoked or a sign-in ends
  is a fragment request (ticking an item, editing a quantity), and it
  stays silent.

#### Option C — `responseHandling` in `htmx-config` (rejected before)

RR-0025 rejected it as option B. It would also swap the handler pages'
4xx fragments into their own targets, which changes forms that work today.

### Q6: framework answers in scope (owner)

All of them are reachable through the scope (trial point 1): the
`Identity` 401, the unknown-URL 404, the path-error 404, the 405, and
the form and query parse 400s. Recommended: all in scope, with the
brief's table corrected (trial point 5). Out of scope as the brief says:
`/assets/`. Also out: `/healthz`'s 503, which stays `text/plain` for
monitors (trial point 7).

### Q7: RR-0023 sub-answer 10 (security, for the record)

It said error pages are translated "after MB-0029 defines a request-aware
error page", and, if MB-0035 landed first, that the bodies stay English
and "no middleware is added around the RR-0010 scope". MB-0035 did land
first; that fallback is what `templating.md` now records (RR-0036
sub-answer 7). With Q1 A, RR-0038 supersedes the "no middleware" clause
and replaces the `templating.md` exception (P6). RR-0023 gets an amendment
note; nothing else in it changes.

### Q8: brief amendments for the RR's roll-back (owner, for information)

- P1: the table in Context gains the 405 and shows the path error as 404.
- P2: names the chosen mechanism (Q4) and the `return` rule.
- P3: names `HX-Retarget: body` and `HX-Reswap: innerHTML` (Q5).
- P6: `error::tests::responses_unchanged` is **kept**, not rewritten:
  `error.rs`'s bodies are now the fallback P5 requires. The pins to
  rewrite are the four tests in trial point 8, and
  `csrf::tests::coverage`'s expected `configure` text (Q1 detail 5).
- `context`: add `routes/language.rs` (`same_site_path`) and
  `routes/theme/tests.rs`.
- `escalate`: the items this RR answers become resolved; the rest stay.
- `resolutions: [RR-0038]`.

## Recommendation and question

Recommended: Q1 A (a `from_fn` middleware around the scope, with
details 1–5); Q2 A (extract `Ui` and `Identity`, one more read for a
signed-in error); Q3 A (no change in `csrf/`); Q4 A (the form in the 401
page, returning to the asked-for GET path through `same_site_path`); Q5 A
(`HX-Retarget: body` for every htmx error page); Q6 all in scope; Q7 the
amendment note; Q8 the brief amendments.

Crisp questions:

1. **Security**: turn plain errors into pages with one `from_fn`
   middleware wrapped around the RR-0010 scope outside `csrf::guard`, by
   trial point 7's rule, with details 1–5 of Q1 A, including the one-line
   change to `csrf::tests::coverage`'s expected text?
2. **Security**: read the language and theme by extracting `Ui` in the
   middleware, accepting one more `users.language` read for a signed-in
   user's error page, and leave `i18n/request.rs` unchanged?
3. **Security**: leave `csrf/` unchanged and rely on the middleware to give
   the cross-site 403 the same page as any 403, checked byte for byte?
4. **Owner / security**: put the sign-in form in the 401 page, keep the
   status 401, and after signing in return to the asked-for path (GET
   only) through `same_site_path`, otherwise `/lists`? Or option C (always
   `/lists`)?
5. **Owner**: send `HX-Retarget: body` and `HX-Reswap: innerHTML` with
   every error page answered to an htmx request, boosted or not, amending
   RR-0025 constraint 3 for error pages only? Or option B (boosted only)?
6. **Owner**: all framework answers inside the scope in scope, `/assets/`
   and `/healthz`'s 503 out?
7. **Owner**: accept the brief amendments in Q8 and the RR-0023 amendment
   note in Q7?

## Blast radius and rollback

What can break: every error answer inside the scope changes body and
`Content-Type` (statuses don't). A client that reads the plain words (a
script, a monitor) sees HTML; `/healthz` stays as it is. Under htmx, error
pages now replace the page. Signing in can return to a path.

Who is affected: every user who meets an error; most visibly anyone whose
sign-in ended, who sees a sign-in form instead of a blank page.

Rollback plan: revert the implementation PR. Removing the one `.wrap` line
alone brings back today's plain bodies, because `error.rs` and `csrf/` are
unchanged under the recommendation; the login `return` and the template
partial are independent of it.

## Minimal context

- Brief: `.agentic/missions/MB-0056-translated-error-pages.md` (P1–P7,
  `escalate`, `forbidden`: no detail on pages, no telling the 403s
  apart, no JavaScript beyond htmx, no literal text, no weakened
  `csrf::tests::coverage`, no dependencies).
- `crates/kauppa-web/src/routes/mod.rs:23-29` (`configure`, the scope).
- `crates/kauppa-web/src/csrf/mod.rs:181-215` (`guard`, the plain 403).
- `crates/kauppa-web/src/error.rs:27-60` (`WebError`'s status and bodies).
- `crates/kauppa-web/src/i18n/request.rs:174-213` (`Ui::from_request`).
- `crates/kauppa-web/src/routes/request.rs` (`is_hx`, `current_user`).
- `crates/kauppa-web/src/routes/language.rs:30-36` (`same_site_path`).
- `crates/kauppa-web/src/routes/auth/mod.rs:100-150` (login form and
  submit; success redirects to `/lists` at line 146).
- `crates/kauppa-web/src/routes/test_support.rs:245-271` (`wired_app!`
  calls `routes::configure`).
- `crates/kauppa-web/src/csrf/tests/coverage.rs:100-110` (the pinned
  `configure` text).
- `assets/vendor/response-targets-2.0.4.min.js` (`HX-Retarget` makes a
  non-200 swap).
- RR-0010 (cross-site check), RR-0023 sub-answer 10, RR-0025 constraints
  1–3, RR-0026 (sign-in deadlines), RR-0036 sub-answers 7 and 8.

## Evidence bundle (Hardened)

Trial on `main` `d918d47`, debug build, `cargo test -p kauppa-web`
(2026-11-14). Throwaway worktree in the session scratchpad, removed after.

Probe, one request per source (signed in = a session from
`login_request`; the page's `<title>` shown):

| Request | Status | Original answer | Page | Notes |
| --- | --- | --- | --- | --- |
| `GET /nope` | 404 | no `Content-Type`, no error | yes | router's not-found, through the scope's middleware |
| `GET /lists`, signed out | 401 | no `Content-Type`, error (`Identity`) | yes | English and, with `Accept-Language: fi`, Finnish |
| `GET /lists/not-a-uuid`, signed in | **404** | `text/plain; charset=utf-8`, error (path) | yes | not 400 as the brief's table says |
| `GET /lists/<random>`, signed in | 404 | no `Content-Type`, error (`WebError`) | yes | |
| same, `Accept-Language: fi`, `kauppa_theme=dark` | 404 | as above | yes | `lang="fi"`, dark choice checked |
| same, `HX-Request` and `HX-Boosted` | 404 | as above | yes | `HX-Retarget: body` sent |
| `GET /auth/logout` | **405** | no `Content-Type`, no error | yes | not in the brief's table |
| `POST /lists`, `Origin: http://evil.example` (`bare_test_app!`) | 403 | no `Content-Type`, no error | yes | the guard's 403 |
| `POST /theme`, `theme=Dark` | 400 | no `Content-Type`, error (`Invalid::Theme`) | yes | |
| `GET /healthz` | 200 | `text/plain` | no | unchanged |

Every page carried `Content-Security-Policy` and `Vary`.

Full suite (171 existing tests) with the rule "error status and not
`text/html`": 164 passed, 7 failed (the six in trial point 8 plus
`routes::health::tests::reports_database_state`, whose 503 became a page).
With the rule in point 7 (plus the probe test): 166 passed, 6 failed
(point 8's table).

First trial, with `req.request().clone()` held across `next.call`: at
least 59 tests failed (output cut there); the one examined,
`routes::health::tests::reports_database_state`, panicked at actix-web
4.18.2 `src/request.rs:197:43` (`Option::unwrap()` on `None` in
`push_resource_id`).
