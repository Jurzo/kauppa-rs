# Mission Brief — Code cleanup: unused code, an unused route, a file over the size guideline

```yaml
---
id: MB-0048
owner: "@juuso-kalliomaki"
goal: "The code holds nothing the app no longer uses, the largest source file is back under the size guideline, and plain error responses are either translated or documented as English by decision."
non_goals:
  - "Behaviour changes users can see, except the error-body decision in P5."
  - "Moving to compile-time-checked queries (MB-0049)."
  - "Refactors beyond the items listed."
acceptance:
  - id: P1
    property: "`sqlx` is a dev-dependency of `kauppa-web` (RR-0036 sub-answer 1), same version and features, `Cargo.lock` unchanged; a test fails if it is back under `[dependencies]`, and the binary builds without it."
    check: "cargo test -p kauppa-web state::tests::sqlx_is_a_dev_dependency"
  - id: P2
    property: "`POST /lists/{id}/favorites` is removed (RR-0036 sub-answer 2): its handler, form type, registration and the test cases that post to it; the old URL answers 404, and the CSRF coverage test still accounts for every route."
    check: "cargo test -p kauppa-web csrf::tests::coverage::every_write_route_is_covered"
  - id: P3
    property: "No source file under `crates/` exceeds 800 lines. `repo/sqlite/list.rs` is split into `list_access.rs` (the predicate and granted-row helpers) and `list.rs` (the `impl`, unchanged) with no SQL, body or signature change, and the bypass guard changes exactly as RR-0036 sub-answer 4 lists; `templates/tests.rs` moves its navigation-panel code to `templates/tests/navigation.rs` with wrappers, no check path changing."
    check: "cargo test -p kauppa-core repo::share::list_queries_route_through_central_predicate"
  - id: P4
    property: "The leftovers RR-0036 sub-answer 6 lists are gone: `InvitationState::as_str`; `PasswordLengthError`'s `Display` and `Error` impls and their one test; `Entity::Favorite` and `Invalid::ChooseFavorite` with their five keys per catalog. Every catalog key is used by a template, a named key in Rust or `every_text`."
    check: "cargo test -p kauppa-web i18n::tests::catalog_has_no_unused_keys"
  - id: P5
    property: "Plain-text error bodies (`not found`, `forbidden`, the cross-site 403 and rejected-input bodies from `error.rs`) stay English in every language, unchanged byte for byte, with the reason stated in `templating.md` (RR-0036 sub-answer 7)."
    check: "cargo test -p kauppa-web error::tests::responses_unchanged"
  - id: P6
    property: "The full suite and lints pass with no lint allowed or lowered."
    check: "scripts/agentic/fast-gate.sh"
context:
  - crates/kauppa-web/Cargo.toml
  - crates/kauppa-web/src/routes/lists/tests/
  - crates/kauppa-web/src/templates/tests.rs
  - crates/kauppa-web/src/csrf/mod.rs
  - locales/en.json
  - locales/fi.json
  - .agentic/mentorship/templating.md
  - .agentic/consults/CRP-0036-code-cleanup-after-audit.md
  - .agentic/resolutions/RR-0036-code-cleanup-after-audit.md
  - crates/kauppa-web/src/routes/lists/items.rs
  - crates/kauppa-core/src/repo/sqlite/list.rs
  - crates/kauppa-core/src/repo/share.rs
  - crates/kauppa-core/src/error.rs
  - crates/kauppa-core/src/domain/invitation.rs
  - crates/kauppa-core/src/domain/password_rule.rs
  - crates/kauppa-web/src/i18n/messages.rs
  - crates/kauppa-web/src/error.rs
  - .agentic/resolutions/RR-0009-source-layout-and-test-paths.md
autonomy:
  may_decide:
    - "how `list.rs` is split, inside RR-0009's rules"
    - "the order of the items, one commit each"
    - "a size check script, if cheap"
  escalate:
    - { when: "a guard change beyond RR-0036 sub-answer 4, or `share.rs` passing 800 lines", to: security }
    - { when: "any `Cargo.toml` change beyond moving the `sqlx` line", to: platform-owner }
    - { when: "a catalog key `catalog_has_no_unused_keys` flags for a reason outside this brief", to: owner }
    - { when: "any `error.rs` or `csrf/` change", to: owner }
  forbidden:
    - "weaken the bypass guard or the CSRF coverage test"
    - "change a status code"
    - "lower lint strictness"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/persistence.md@v1
  - .agentic/mentorship/actix-web.md@v1
resolutions: [RR-0036]
---
```

## Context

Found in the audit of 2026-11-10 (findings 15–18), re-checked on `main`
`b800583` on 2026-11-14 (CRP-0036) and decided in RR-0036.

- `kauppa-web` lists `sqlx` under `[dependencies]`, but only tests use it,
  in four files; `persistence.md` says the web layer never imports it.
- `routes/lists/items.rs` still registers `POST /{id}/favorites`; no
  template posts to it since MB-0036 merged the two add forms.
- `repo/sqlite/list.rs` is 855 lines and `templates/tests.rs` 821, both
  over RR-0009's guideline of about 800.
- `InvitationState::as_str` has no caller anywhere; `PasswordLengthError`'s
  `Display` is shown nowhere (one unit test reads it); `Entity::Favorite`
  is never raised. 15 of the 28 entity keys are unreachable; only
  `Favorite`'s are removed, since the types still allow the others.
- `error.rs` bodies and the cross-site 403 are plain English for every
  user, and stay so by decision (RR-0036 sub-answer 7). A signed-out
  request also gets an empty 401 from `actix-identity`; a translated error
  page is a separate brief (RR-0036 sub-answer 8).

## Conceptual plan

1. **Clarify (S1).** One CRP: the manifest change (platform), the route and
   the error bodies (owner), the split's guard (security). Answered by
   RR-0036 (CRP-0036, all as recommended).
2. Start from `main` after MB-0055 (PR #88) merges. One item per commit,
   tests green after each; the `templates/tests.rs` move last.

## Validation plan

- The named tests, the fast gate, `cargo deny check`, and
  `csrf::tests::cross_site` for the cross-site body (P5).
- The MB-0008 bypass experiments re-run against the split (P3): a method
  without a gate, and list SQL in another file, each fail the guard.
- `wc -l` over `crates/` in the Merge-Readiness Pack.
