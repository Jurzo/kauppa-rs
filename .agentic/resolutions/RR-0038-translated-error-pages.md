# Resolution Record — Translated error pages: one middleware around the RR-0010 scope, sign-in from the 401 page, error pages swap `<body>` under htmx

```yaml
---
id: RR-0038
closes: CRP-0038
mission: MB-0056
decided_by: "@Jurzo"
role: security
risk_tier: medium
decided_at: 2026-11-14T13:33:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0056-translated-error-pages.md
  - .agentic/consults/CRP-0038-translated-error-pages.md
  - .agentic/resolutions/RR-0023-translations.md
  - .agentic/resolutions/RR-0025-response-targets-on-body.md
  - .agentic/mentorship/templating.md
  - .agentic/mentorship/actix-web.md
  - .agentic/mentorship/security.md
---
```

## Decision and outcome

Approve CRP-0038's questions 1–7 as recommended.

Sub-answers:

1. **The mechanism (Q1 A, security).**
   - `crates/kauppa-web/src/routes/errors.rs` holds a `from_fn` middleware.
     `routes::configure` wraps it around the RR-0010 scope, outside
     `csrf::guard`:
     `web::scope("").wrap(from_fn(crate::csrf::guard)).wrap(from_fn(errors::pages)).configure(protected)`.
     `main.rs` and the test app's wiring don't change.
   - **The rewrite rule.** An answer is replaced by the error page only
     when its status is 4xx or 5xx **and** it carries an error
     (`ServiceResponse::response().error()` is `Some`) **or** has no
     `Content-Type`. Every other answer passes through untouched, among
     them handler pages (the login, register and invitation forms' 4xx,
     forms' 400 and 409 with the reason) and `/healthz`'s `text/plain`
     503.
   - **Same status.** The page keeps the original status. It keeps the
     original headers except `Content-Type` and `Content-Length`, and adds
     `Content-Type: text/html; charset=utf-8` and
     `Vary: Cookie, Accept-Language`.
   - **No request clone across the inner call** (CRP-0038 trial point 6:
     actix-web's router panics). The middleware reads the request from
     the response. An `Err` from the inner service passes on unchanged.
   - **Fallback (P5).** If the page can't be rendered (template error,
     the language read fails), the original answer goes out unchanged, and
     the middleware logs one ERROR line naming the template, without the
     error's detail. `error.rs` and its log lines (MB-0020) don't change;
     `error.rs`'s bodies are now that fallback.
   - **`csrf::tests::coverage`.** Its expected `configure` text gains the
     `.wrap(from_fn(errors::pages))` call after the guard's. No other rule
     of the test changes. This is not a weakening under MB-0056's
     `forbidden`.
2. **Language, theme and signed-in state (Q2 A, security).**
   The middleware extracts `Ui` and `Identity` with `FromRequest` from the
   response's request. "Signed in" means the identity's id parses as a
   UUID (as `request::current_user`). `i18n/request.rs` doesn't change. A
   signed-in user's error page reads `users.language` once more; signed-out
   error pages read nothing.
3. **The cross-site 403 (Q3 A, security).** `csrf/` doesn't
   change. Its plain 403 has no `Content-Type`, so the middleware rewrites
   it with the same code as `WebError`'s 403. The page depends only on the
   status and the request's language, theme and signed-in state, never on
   which check refused it.
4. **Signed out (Q4 A, owner and security).**
   - The 401 page shows the sign-in form, the same form as `/auth/login`,
     moved into a partial both use. The status stays 401.
   - For a GET, the form carries a hidden `return` holding the asked-for
     path and query. For any other method there is no `return`.
   - `login_submit` sends a successful sign-in to
     `language::same_site_path(return)` when `return` is present, and to
     `/lists` otherwise. A failed sign-in renders the login page as today
     (RR-0025's `hx-target-4xx` form), keeping `return`.
   - Tests cover `//host`, `/\host`, `https://host`, an encoded
     `%2F%2Fhost` and a control character through the login: each lands on
     `/`.
5. **htmx (Q5 A, owner).** For a request with
   `HX-Request: true`, boosted or not, the error page also sends
   `HX-Retarget: body` and `HX-Reswap: innerHTML`; the response-targets
   extension on `<body>` then swaps it. No change to templates, the
   extension, `htmx-config` or `hx-target-*`. Only pages this middleware
   made carry the headers. This amends RR-0025 constraint 3 and sub-answer
   3 for error pages only.
6. **Framework answers (Q6, owner).** In scope: everything
   inside the scope — the `Identity` 401, the unknown-URL 404, the
   path-error 404, the 405, form and query parse 400s. Out of scope:
   `/assets/` (outside the scope; RR-0011, RR-0029) and `/healthz`'s 503
   (typed, for monitors).
7. **Brief amendments and RR-0023 (Q7, Q8, owner).** The
   brief amendments in CRP-0038 Q8, and an amendment note on RR-0023
   sub-answer 10 (below).

Covers:

- MB-0056's implementation as above, in one PR, tests first;
- the login template split into a partial and the `return` field in
  `templates/auth/`, and the `return` handling in `routes/auth/mod.rs`;
- new templates and catalog keys for the error pages, in both catalogs
  (the owner reviews the Finnish);
- the rewritten pins named in CRP-0038 trial point 8, with every status
  assertion kept.

Does not cover:

- any status code change, any change to `csrf/`, `error.rs`, `main.rs`,
  `i18n/request.rs`, the Content-Security-Policy, the htmx config or the
  vendored files;
- a `return` for non-GET requests, or any redirect target that isn't a
  same-site path;
- error pages for `/assets/` or `/healthz`;
- caching `Ui` in the request (CRP-0038 Q2 B).

## Rationale and trade-offs

- **One middleware around the scope (A) over app-wide (B), `ErrorHandlers`
  (C) or per handler (D).** The trial showed A reaches every plain error
  with no change to `main.rs`, the test app, `error.rs` or `csrf/`, and
  keeps `/assets/` out by position. B needs a path exclusion for the same
  result; D can't reach the framework's answers.
- **The narrow rewrite rule.** "Any non-HTML error" turned `/healthz`'s
  503 into a page; "carries an error or is untyped" catches every plain
  error and nothing a handler rendered on purpose.
- **Extract `Ui`, accept one read.** Errors are rare; changing a trigger
  path to save one indexed read isn't worth it.
- **No change in `csrf/`.** The same code renders both 403s, so they can't
  differ; making the guard return `WebError` would add a log line.
- **Sign in from the 401 page, keep 401.** A redirect would change the
  status (a non-goal) and be followed silently by htmx. Returning to the
  asked-for GET path reuses the validator the language and theme
  switches already rely on.
- **`HX-Retarget` over `responseHandling` or `hx-target-error` on
  `<body>`.** Both alternatives would also swap handler pages, changing
  forms that work today. The header touches only the middleware's pages,
  and covers fragment requests, which is where a revoked share or an
  ended sign-in is most often met.

## Approvals and authority

@Jurzo as security for sub-answers 1–3 and 4; owner for 4–7.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0038-translated-error-pages.md`
- Mission Brief: `.agentic/missions/MB-0056-translated-error-pages.md`
- Related: RR-0010 (cross-site check, the scope), RR-0011 and RR-0029
  (`/assets/` answers), RR-0023 (sub-answer 10, amended), RR-0025
  (constraint 3 and sub-answer 3, amended), RR-0026 (sign-in deadlines),
  RR-0035 (theme cookie), RR-0036 (sub-answers 7 and 8).
- Evidence: CRP-0038 → "Current state, checked by a trial" and "Evidence
  bundle" (trial on `main` `d918d47`: one probe request per error source;
  full-suite results under both rewrite rules; the request-clone panic).

## Constraints and follow-on rules (Hardened)

1. **Error pages never depend on why a request was refused.** A page is a
   function of the status and the request's language, theme and
   signed-in state only. In particular, the cross-site 403 and an
   authorization 403 are the same bytes for the same request, and a 404
   for a list the user may not see is the same as for one that doesn't
   exist (RR-0002 constraint 4). Tests compare them byte for byte.
2. **No detail on an error page**: no error text, type name, SQL, id or
   input value.
3. **The middleware stays outside `csrf::guard` and inside the identity
   and session middleware.** Moving it changes what it can see; that
   needs an RR.
4. **Handler pages are never rewritten.** A handler that renders a 4xx on
   purpose sets `Content-Type: text/html` and returns `Ok`.
5. **`HX-Retarget` on a non-200 answer only from the error-page
   middleware** (replaces RR-0025 constraint 3's blanket "without an RR"
   for this case). Any other use still needs an RR.
6. **`return` only through `same_site_path`.** Any new redirect target
   taken from a request goes through it, or through a stricter check.
7. **The fallback stays plain.** `error.rs`'s bodies and the guard's 403
   remain the answer when a page can't be rendered.

## Roll-back into the system (Hardened)

To be applied after this record is signed, in the consult's PR:

- `MB-0056`: CRP-0038 Q8's amendments: the Context table (path error 404,
  the 405); P2 names the form in the 401 page and the `return` rule; P3
  names `HX-Retarget: body` and `HX-Reswap: innerHTML`; P6 keeps
  `error::tests::responses_unchanged` as is and names the pins to rewrite
  (CRP-0038 trial point 8) and `csrf::tests::coverage`'s expected text;
  `context` adds `routes/language.rs` and `routes/theme/tests.rs`;
  `escalate` settled; `resolutions: [RR-0038]`.
- `CRP-0038`: `state: open` → `answered`.
- `RR-0023`: under "Supersession and history", add "Amended by RR-0038
  (2026-11-14): error pages are translated by a middleware around the
  RR-0010 scope; sub-answer 10's 'no middleware is added around the
  RR-0010 scope' no longer applies." Nothing else in RR-0023 changes.
- `RR-0025`: under "Supersession and history", add "Amended by RR-0038
  (2026-11-14): the error-page middleware sends `HX-Retarget: body` on
  its pages for htmx requests (constraint 3 and sub-answer 3 allow it for
  that case only)."
- `.agentic/mentorship/templating.md`: the English-error-bodies exception
  replaced by: "Error answers inside the RR-0010 scope are pages in the
  request's language and theme (RR-0038); `error.rs`'s English bodies are
  only the fallback when a page can't be rendered." (MB-0056 P6)
- `.agentic/mentorship/actix-web.md`: line 19's "Error paths return the
  sanitized message from `WebError::error_response`" gains "; the
  error-page middleware turns it into a page (RR-0038). A handler that
  renders its own 4xx sets `Content-Type: text/html`."
- `.agentic/mentorship/security.md`: one line in the auth rules: "A
  redirect target taken from a request (`return`) goes through
  `language::same_site_path` (RR-0038 constraint 6)."

## Supersession and history

Status: active once signed. Amends RR-0023 (sub-answer 10) and RR-0025
(constraint 3, sub-answer 3); supersedes nothing.
