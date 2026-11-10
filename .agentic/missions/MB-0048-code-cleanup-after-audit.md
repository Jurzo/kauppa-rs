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
    property: "`sqlx` is a dev-dependency of `kauppa-web`, or removed from it, and no non-test code in the crate names it."
    check: "cargo test -p kauppa-web routes::auth::tests::register_requires_invite::register_requires_invite_no_bypass"
  - id: P2
    property: "`POST /lists/{id}/favorites` is removed if nothing uses it (templates, tests of real flows), or kept with a comment and test naming its user; the CSRF coverage test still accounts for every route."
    check: "cargo test -p kauppa-web csrf::tests::coverage::every_write_route_is_covered"
  - id: P3
    property: "No source file under `crates/` exceeds 800 lines: `repo/sqlite/list.rs` is split by area while every list-table query stays behind the central access predicate and the bypass guard passes unchanged in intent."
    check: "cargo test -p kauppa-core repo::share::list_queries_route_through_central_predicate"
  - id: P4
    property: "Leftovers of MB-0035 are gone or used: `InvitationState::as_str`, `PasswordLengthError`'s unused display text, and `Entity` variants and `errors.*` keys that no code path can produce."
    check: "cargo test -p kauppa-web i18n::tests::catalogs_complete"
  - id: P5
    property: "Plain-text error bodies (`not found`, `forbidden`, the cross-site 403 and rejected-input bodies from `error.rs`) follow the owner's decision: translated through a request-aware error response, or kept English with the reason stated in `templating.md`."
    check: "cargo test -p kauppa-web error::tests::responses_unchanged"
  - id: P6
    property: "The full suite and lints pass with no lint allowed or lowered."
    check: "scripts/agentic/fast-gate.sh"
context:
  - crates/kauppa-web/Cargo.toml
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
    - { when: "the `Cargo.toml` change (trigger path; dependency-bump runbook)", to: platform-owner }
    - { when: "removing a route, in case a bookmark or script uses it", to: owner }
    - { when: "any change to how list queries are guarded (`repo::share`'s bypass guard, RR-0002, RR-0009 constraint 3)", to: security }
    - { when: "translating error bodies (RR-0023 sub-answer 10 left them English) and any `error.rs` change", to: owner }
  forbidden:
    - "weaken the bypass guard or the CSRF coverage test"
    - "change a status code"
    - "lower lint strictness"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/persistence.md@v1
  - .agentic/mentorship/actix-web.md@v1
resolutions: []
---
```

## Context

Found in the audit of 2026-11-10 (findings 15–18). Items marked (#63)
exist once MB-0035's second PR merges.

- `kauppa-web` lists `sqlx` under `[dependencies]`, but only tests use it
  (`error.rs` tests, `admin/bootstrap.rs` tests), and `persistence.md` says
  the web layer never imports it.
- `routes/lists/items.rs` still registers `POST /{id}/favorites`; no
  template posts to it since MB-0036 merged the two add forms.
- `repo/sqlite/list.rs` is 801 lines (RR-0009's guideline is about 800), and
  MB-0036's pack already notes a split. Several test files are at 700–780.
- (#63) `InvitationState::as_str` has no caller in `kauppa-web`;
  `PasswordLengthError`'s `Display` is no longer shown; `Entity::Favorite`,
  `Ingredient` and `Recipe` have `errors.*` keys for reasons no code
  raises.
- (#63) `error.rs` bodies and the cross-site 403 are plain English text for
  every user.

## Conceptual plan

1. **Clarify (S1).** One CRP: the manifest change (platform), the route and
   the error bodies (owner), the split's guard (security).
2. One item per commit, tests green after each.

## Validation plan

- The named tests, the fast gate, `cargo deny check`.
- `wc -l` over `crates/` in the Merge-Readiness Pack.
