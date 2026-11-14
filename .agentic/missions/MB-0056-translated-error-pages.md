# Mission Brief — Translated HTML error pages, with a way on

```yaml
---
id: MB-0056
owner: "@juuso-kalliomaki"
goal: "When the app can't show what was asked for (not found, not allowed, signed out, a rejected request, a server fault), the user gets a page in their language and theme, inside the usual layout, that says what happened and offers a way on (a signed-out user can sign in), with or without htmx, while every status code and every security decision stays as it is."
non_goals:
  - "Changing any status code, or which request gets which status: authorization, the cross-site check (RR-0010) and routing decide exactly as today."
  - "Changing the forms that already catch `Invalid` and `Conflict` and show the translated reason in the page; they keep doing so."
  - "Error answers for `/assets/` (static files: 404 and 405 stay as RR-0011 and RR-0029 set them)."
  - "The log lines for errors (MB-0020: client errors at WARN, server errors at ERROR) and what they record."
  - "Pages for errors the app doesn't produce today, or new error states."
acceptance:
  - id: P1
    property: "Every 4xx and 5xx answer to a page request, from the app's own errors (`WebError`), the cross-site check, the signed-out `Identity` extractor, an unknown URL, a malformed path (404) or form (400) and a wrong method (405), is an HTML page rendered with the layout (navigation, language selector, theme switch) in the request's language and theme. It names what happened in plain words and offers a way on (at least a link to the front page, or to the lists when signed in). The status code is the one the request gets today. One `from_fn` middleware around the RR-0010 scope, outside `csrf::guard`, makes the pages, rewriting only a 4xx or 5xx that carries an error or has no `Content-Type` (RR-0038 sub-answer 1); handler pages and `/healthz` pass through."
    check: cargo test -p kauppa-web routes::errors::tests::error_pages
  - id: P2
    property: "A signed-out request for a page that needs a sign-in lets the user sign in from where they land: the 401 page (status kept) shows the sign-in form, shared with `/auth/login` as a partial. For a GET it carries a hidden `return` with the asked-for path and query, and a successful sign-in goes to `language::same_site_path(return)`, otherwise to `/lists`; `//host`, `/\\host`, `https://host`, `%2F%2Fhost` and a control character each land on `/` (RR-0038 sub-answer 4). A user whose sign-in ended (RR-0026's 30-day idle limit) no longer sees an empty page."
    check: cargo test -p kauppa-web routes::errors::tests::signed_out
  - id: P3
    property: "Under htmx, a boosted link or form answered with an error page shows that page (it replaces `<body>`) instead of nothing happening, and MB-0029's error-page forms keep working. Every error page answered to an htmx request, boosted or a fragment request, sends `HX-Retarget: body` and `HX-Reswap: innerHTML`, so the page replaces `<body>` and never lands inside a fragment's target; only the middleware's pages carry these headers (RR-0038 sub-answer 5)."
    check: cargo test -p kauppa-web routes::errors::tests::under_htmx
  - id: P4
    property: "Error pages disclose nothing new: no error detail, type name, SQL, id or input value; the cross-site 403 and an authorization 403 are the same page, so a client still can't tell which check failed; a 404 for a list the user may not see is the same page as for one that doesn't exist (RR-0002 constraint 4)."
    check: cargo test -p kauppa-web routes::errors::tests::no_detail
  - id: P5
    property: "If rendering an error page itself fails, the answer is today's plain body with the original status, never a panic and never a second error page; all page text comes from the catalogs in both languages; the Content-Security-Policy and other headers are unchanged; templates stay free of literal text, inline scripts and styles."
    check: cargo test -p kauppa-web routes::errors::tests::render_failure_falls_back
  - id: P6
    property: "Nothing else changes: `error::tests::responses_unchanged` stays as it is (`error.rs`'s bodies are now P5's fallback); the tests that pin today's plain bodies or the wiring (`csrf::tests::cross_site::{rejects_cross_site_writes, missing_proof_is_rejected}`, `routes::admin::tests::access::admin_only_non_admin_gets_404`, `routes::theme::tests::rejects_other_values`, whose no-echo check reads the page's main content rather than the layout, and `csrf::tests::coverage`'s expected `configure` text, one `.wrap` added) are rewritten with their status assertions kept, `templating.md`'s English-bodies exception (RR-0036 sub-answer 7) is replaced by the new rule, and the whole suite passes with no assertion about status or authorization weakened."
    check: cargo test --workspace --all-targets
  - id: P7
    property: "In a browser, in English and Finnish, light and dark, with and without JavaScript: a dead link, an unknown URL, a list whose share was revoked, an ended sign-in and a forged cross-site post each show the right page in the layout, and its way on works; a boosted link to a dead page shows the page; the console shows no Content-Security-Policy violation."
    check: "manual: browser check on the release binary, recorded in MB-0056's Continuity Pack"
context:
  - crates/kauppa-web/src/error.rs
  - crates/kauppa-web/src/csrf/mod.rs
  - crates/kauppa-web/src/main.rs
  - crates/kauppa-web/src/routes/mod.rs
  - crates/kauppa-web/src/routes/request.rs
  - crates/kauppa-web/src/routes/test_support.rs
  - crates/kauppa-web/src/i18n/request.rs
  - crates/kauppa-web/src/theme.rs
  - crates/kauppa-web/src/routes/auth/mod.rs
  - crates/kauppa-web/src/routes/language.rs
  - crates/kauppa-web/src/routes/theme/tests.rs
  - crates/kauppa-web/src/routes/admin/tests/access.rs
  - crates/kauppa-web/src/csrf/tests/cross_site.rs
  - templates/layout.hbs
  - templates/auth/login.hbs
  - locales/en.json
  - locales/fi.json
  - .agentic/resolutions/RR-0010-csrf-mechanism.md
  - .agentic/resolutions/RR-0023-translations.md
  - .agentic/resolutions/RR-0025-response-targets-on-body.md
  - .agentic/resolutions/RR-0036-code-cleanup-after-audit.md
  - .agentic/resolutions/RR-0038-translated-error-pages.md
  - .agentic/consults/CRP-0038-translated-error-pages.md
  - .agentic/mentorship/templating.md
  - .agentic/mentorship/security.md
  - .agentic/mentorship/actix-web.md
autonomy:
  may_decide:
    - "template and catalog key names, and the English wording (the owner reviews the Finnish)"
    - "which links each page offers, within P1"
    - "test names inside `routes::errors::tests` (record a rename here)"
    - "whether the pages share one template with a per-status message or use one per status"
  escalate:
    - { when: "any change to the mechanism beyond RR-0038 sub-answer 1: the middleware's place (constraint 3), the rewrite rule, or a change to `main.rs`, the test app, `error.rs` or `csrf/`", to: security }
    - { when: "a change to `i18n/request.rs`, or caching `Ui` in the request (CRP-0038 Q2 B)", to: security }
    - { when: "a page that would differ between the cross-site 403 and an authorization 403, or between a hidden and a missing list (RR-0038 constraint 1)", to: security }
    - { when: "a `return` beyond RR-0038 sub-answer 4 (non-GET requests, or a target not checked by `same_site_path`)", to: security }
    - { when: "`HX-Retarget` or `HX-Reswap` beyond the middleware's pages, or any change to templates' `hx-target-*`, the extension or `htmx-config` (RR-0038 sub-answer 5, RR-0025)", to: owner }
    - { when: "an error answer inside the scope that the rewrite rule misses, or one outside it (`/assets/`, `/healthz`) that would need a page", to: owner }
    - { when: "any status code change, or any change to the Content-Security-Policy or the htmx config", to: security }
  forbidden:
    - "show error detail, a type name, SQL, an id or an input value on an error page"
    - "let a page reveal which of the cross-site check or authorization refused a request"
    - "JavaScript beyond htmx, `hx-on`, inline scripts, inline styles or `style=` attributes"
    - "literal text in templates (every label through `{{t}}`)"
    - "weaken `csrf::tests::coverage`, the bypass guard, or any status assertion"
    - "add dependencies"
    - "lower lint strictness"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
  - .agentic/mentorship/security.md@v1
  - .agentic/mentorship/actix-web.md@v1
resolutions: [RR-0038]
---
```

## Context

Requested by the owner in RR-0036 sub-answer 8 (2026-11-14): "a separate
brief for a translated HTML error page (layout, navigation, sign-in on
401), after MB-0055". MB-0055 merged the same day (PR #88), so the layout
now carries the theme switch as well as the language selector.

**What a user gets today** (CRP-0036 → P5, read on `main` `b800583`):

| Status | Body | When a user sees it |
| --- | --- | --- |
| 401 | *empty* (`actix-identity`'s `Identity` extractor) | **The most common case:** opening any signed-in page after the sign-in ended (RR-0026: 30 idle days, 90 days at most), or from a bookmark while signed out |
| 404 | `not found` (`error.rs`) | A link to a list or recipe that was deleted, or to a list whose share was revoked (RR-0002 constraint 4); a non-admin opening `/admin` |
| 404 | *empty* (`actix-web`) | An unknown URL |
| 403 | `forbidden` (`error.rs`, and `csrf/mod.rs:211`) | Rare in normal use: a page left open while a share was lowered to view; any form post if `KAUPPA_PUBLIC_ORIGIN` is set wrong, or a proxy strips `Origin` and `Referer` |
| 400, 409 | the reason's English text | Only where a handler doesn't catch `Invalid`/`Conflict`; the forms do |
| 404 | `text/plain` path error (`actix-web`) | A malformed id in a URL (CRP-0038: 404, not 400) |
| 400 | `Parse error: …` (`actix-web`) | A malformed form or query |
| 405 | *empty* (`actix-web`) | A wrong method on a route, such as `GET /auth/logout` (CRP-0038) |
| 401 | `unauthorized` (`error.rs`) | Almost never: an identity that isn't a UUID |
| 500 | `internal server error` | A server fault |

On a plain navigation the browser shows the bare text, or a blank page. On
a boosted link (htmx) **nothing happens**: htmx doesn't swap a 4xx or 5xx
by default, and MB-0029 (RR-0018, RR-0025) enabled swapping only for the
login, register and create-invitation forms, deliberately keeping
`hx-target-*` off everything else and `responseHandling` out of the htmx
config.

**History of the decision.** RR-0023 sub-answer 10 kept the plain bodies
English "after MB-0029 defines a request-aware error page", and ruled out
"middleware around the RR-0010 scope". RR-0036 sub-answer 7 recorded the
English bodies in `templating.md` as an exception until this brief, and
sub-answer 8 asked for it.

**Why it isn't a small change** (CRP-0036 Q5 B):

- `ResponseError::error_response(&self)` has no request, so it can't pick
  a language, a theme or whether the user is signed in. The page must be
  built where the request is known: a middleware that rewrites error
  responses (wired in `main.rs`, a trigger path, and mirrored in the test
  app), or a different error path in every handler.
- The language and theme come from the `Ui` extractor in
  `i18n/request.rs` (a trigger path), which reads the account's language
  from the database for a signed-in user.
- The cross-site 403 is made inside `csrf/` (a trigger path), before any
  handler runs; its comment requires that the client can't tell it from
  any other 403.
- The empty 401 and 404 and the parse errors come from the framework, not
  from `WebError`.
- Three test files pin today's bodies byte for byte.

## Conceptual plan

1. **Clarify (S1).** Done: CRP-0038 with a trial, decided in RR-0038 (2026-11-14), which settles the questions below. One CRP: the mechanism and where it sits (security);
   language, theme and signed-in state outside `Ui` (security); the
   cross-site 403 (security); signed out: form in the page or redirect, and
   whether to return to the asked-for page (owner, security); htmx
   swapping for boosted requests and fragments, amending RR-0025 (owner);
   framework answers in scope (owner). Include a trial of the recommended
   mechanism on one status, with its effect on the test app.
2. Tests first: the pages for each status, signed in and out, both
   languages, with and without `HX-Request` and `HX-Boosted`; the
   no-detail and fallback properties; the rewritten pins (P6).
3. The page template(s) and catalog keys, the mechanism, the htmx change
   the RR allows.
4. `templating.md`'s exception replaced (roll-back).
5. Browser check (P7) by the owner.

Slow-mode triggers: the mechanism needs the cross-site check or the
identity middleware reordered; a page can't be rendered without a second
database read on every error; making boosted errors swap would also make
fragment requests swap whole pages into their targets.

## Validation plan

- P1–P5: route tests through the real wiring (`test_app!`, and
  `bare_test_app!` for the cross-site 403), one per status source, in both
  languages and themes, signed in and out, plain and htmx.
- P4: the pages for a revoked share and a missing list compared byte for
  byte; the cross-site and authorization 403 pages compared byte for byte.
- P5: a forced render failure (a broken template in the test registry).
- P6: the whole suite; the rewritten pins keep their status assertions.
- P7: the owner's browser check.
