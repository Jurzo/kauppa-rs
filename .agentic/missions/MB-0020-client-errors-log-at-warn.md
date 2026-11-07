# Mission Brief — Log expected 4xx responses at WARN, not ERROR

```yaml
---
id: MB-0020
owner: "@juuso-kalliomaki"
goal: Make ERROR in the server log mean "something is broken" again. Client errors the app answers on purpose (400, 401, 403, 404, 409) are logged at WARN; only server errors (5xx) stay at ERROR. The response the user sees doesn't change.
non_goals:
  - Changing status codes, response bodies or which errors are returned.
  - "`tracing_actix_web`'s own per-request log line (its levels come from its root span builder)."
  - Adding new log fields, metrics or alerting.
  - Re-classifying which `CoreError` maps to which status.
acceptance:
  - id: P1
    property: "Every client error a handler returns — `Invalid` (400), `Unauthorized` (401), `Forbidden` (403), `NotFound` (404), `Conflict` (409) — is logged once at WARN by `kauppa_web::error`, with the same fields as today, and nothing at ERROR."
    check: cargo test -p kauppa-web error::tests::client_errors_log_at_warn
  - id: P2
    property: "Server errors — template render failures, IO errors, `Internal`, and any `CoreError` that maps to 500 — are still logged at ERROR."
    check: cargo test -p kauppa-web error::tests::server_errors_log_at_error
  - id: P3
    property: Status codes and the sanitized response bodies are unchanged for every variant (internals never reach the client).
    check: cargo test -p kauppa-web error::tests::responses_unchanged
  - id: P4
    property: "End to end, a request that ends in an expected 403 (a view grantee posting to a shared list) and one that ends in 404 (another user's list) produce no ERROR line from `kauppa_web::error`."
    check: cargo test -p kauppa-web routes::lists::tests::sharing_roles::expected_errors_log_at_warn
context:
  - crates/kauppa-web/src/error.rs
  - crates/kauppa-web/src/routes/lists/tests/mod.rs
  - crates/kauppa-web/src/routes/test_support.rs
  - .agentic/mentorship/security.md
  - .agentic/mentorship/actix-web.md
autonomy:
  may_decide:
    - "how the level is chosen (e.g. a match on `status_code()` in `error_response`)"
    - "moving MB-0008's `LogCapture` test writer from `routes::lists::tests` to `routes::test_support` so both test modules share it"
    - test names inside the paths above and the exact assertions
  escalate:
    - { when: "a 4xx would be better at INFO or DEBUG (e.g. 401 on every expired session), or a 5xx at WARN", to: owner }
    - { when: "the log line needs a field it doesn't have today", to: security }
    - { when: "the change needs `tracing_actix_web` configuration or a dependency change", to: platform-owner }
  forbidden:
    - change status codes or response bodies
    - log more of an error than today (no request bodies, emails or cookies; `security.md` → Logging)
    - add dependencies or lower lint strictness
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/actix-web.md@v1
  - .agentic/mentorship/security.md@v1
resolutions: []
---
```

## Context

Found during MB-0019's smoke tests (2026-11-07). `WebError::error_response`
in `crates/kauppa-web/src/error.rs` logs every error with
`tracing::error!(error = ?self, "request error")`, whatever its status.
After MB-0008 (sharing), a view grantee's attempted write (403) is a normal,
expected outcome. So is a stale link to a deleted recipe (404). Both still
show up as ERROR:

```
ERROR kauppa_web::error: request error error=Core(Forbidden)
 WARN tracing_actix_web::middleware: Error encountered while processing the incoming HTTP request: Core(Forbidden)
```

That buries real failures, such as a template render error or the database
being unreachable, among routine ones. It also makes ERROR useless as an
alert signal for self-hosters. The owner asked for expected 4xx at WARN
(2026-11-07).

The log line's content stays as it is. `security.md` → Logging forbids
passwords, hashes, cookies and full emails at INFO or higher, and WARN is
higher than INFO, so the same rule keeps holding.

## Conceptual plan

- **Choose the level from the status.** In `error_response`, a status in
  `400..=499` logs with `tracing::warn!` and everything else with
  `tracing::error!`, same message and fields. One match, so a new variant
  falls into the right level through `status_code()`.
- **Unit tests in `error.rs` (P1–P3):** build each variant, call
  `error_response()` under a captured subscriber, and assert the level, the
  status and the body.
- **End to end (P4):** one test in the `sharing_roles` area, reusing MB-0008's
  log-capture writer (moved to `test_support` if both modules need it).

Checkpoints: P1–P3 green before P4.

Slow-mode trigger: a captured log shows an error variant whose status isn't
what its name suggests. Stop and report; don't re-map it here.

## Validation plan

- P1–P3: unit tests in `crates/kauppa-web/src/error.rs` (`error::tests`).
  `tracing` output is captured with a scoped subscriber writing to a buffer,
  and each test asserts the level token (`WARN` / `ERROR`) and the
  `kauppa_web::error` target.
- P4: a route test in `routes/lists/tests/sharing_roles.rs`. A view grantee
  posts an item (403) and a stranger opens the list (404). The captured log
  has `WARN kauppa_web::error` lines for both and no `ERROR` line.
- Smoke: run the binary, repeat both requests with curl, and grep the log;
  include the output in the Merge-Readiness Pack.
