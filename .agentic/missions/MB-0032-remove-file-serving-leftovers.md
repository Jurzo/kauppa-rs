# Mission Brief — Remove the file-serving leftovers of MB-0024

```yaml
---
id: MB-0032
owner: "@juuso-kalliomaki"
goal: "Finish MB-0024's move to embedded files: drop `actix-files` and handlebars' `dir_source`, which only tests still use, and stop copying `templates/`, `assets/` and `migrations/` into the runtime image, which the binary no longer reads."
non_goals:
  - "Caching headers (`ETag`, `Cache-Control`) for `/assets` (a separate brief, MB-0024 non-goal)."
  - "Any change to what `/assets` serves or to template rendering."
  - "Other dependency bumps."
acceptance:
  - id: P1
    property: "`actix-files` is in neither `Cargo.toml` nor `crates/kauppa-web/Cargo.toml`, handlebars has no `dir_source` feature, and `Cargo.lock` loses only the packages that only they pulled in; `cargo deny check` passes."
    check: "scripts/agentic/fast-gate.sh and cargo deny check, with `cargo tree -i actix-files` reporting no match (output in the Merge-Readiness Pack)"
  - id: P2
    property: "`serves_every_asset` keeps the pinned `SERVED_AS` table (every file in `assets/` served byte-for-byte with its pinned content type) without the differential `Files` check."
    check: cargo test -p kauppa-web routes::assets::tests::serves_every_asset
  - id: P3
    property: "`registry_is_embedded` still proves every template under `templates/` is registered from the embedded copy, without `dir_source`."
    check: cargo test -p kauppa-web templates::tests::registry_is_embedded
  - id: P4
    property: "The runtime image holds the binary and no `templates/`, `assets/` or `migrations/` directory, and still serves the login page, `/assets/app.css` and a registered user's lists (MB-0022 P1's check updated to say so)."
    check: scripts/deploy/smoke_image.sh
context:
  - Cargo.toml
  - crates/kauppa-web/Cargo.toml
  - crates/kauppa-web/build.rs
  - crates/kauppa-web/src/routes/assets/tests.rs
  - crates/kauppa-web/src/templates/tests.rs
  - deploy/Dockerfile
  - scripts/deploy/smoke_image.sh
  - .agentic/resolutions/RR-0011-embedded-assets-registration.md
autonomy:
  may_decide:
    - "how `registry_is_embedded` lists the template files without `dir_source` (e.g. `std::fs::read_dir` in the test)"
    - "comment wording that mentions `actix-files` in `build.rs` and `routes/assets.rs`"
  escalate:
    - { when: "the dependency removal itself (dependency-bump runbook: CRP and RR before the PR)", to: platform-owner }
    - { when: "dropping `templates/`, `assets/` or `migrations/` from the runtime image, which changes MB-0022 P1's wording (MB-0022 escalation: owner)", to: owner }
    - { when: "any change to `main.rs` or to the `/assets` service's behaviour", to: security }
  forbidden:
    - "add or bump any other dependency"
    - "change the content types in `build.rs` or `SERVED_AS`"
    - lower lint strictness
runbook: .agentic/runbooks/dependency-bump.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/actix-web.md@v1
  - .agentic/mentorship/templating.md@v1
resolutions: []
---
```

## Context

RR-0011 sub-answer 2 (platform-owner) deferred the clean-up when MB-0024
(#38) embedded templates and assets: `actix-files` stays only for the
differential check in `serves_every_asset` (constraint 5), and handlebars'
`dir_source` only for the directory comparison in `registry_is_embedded`.
CRP-0011 measured that removing `actix-files` drops six crates from
`kauppa-web`'s tree and doesn't change the release binary. The removal must
be named in a brief (dependency-bump runbook, entry gate item 1).

`deploy/Dockerfile`'s runtime stage still copies `templates/`, `assets/`
and `migrations/` into `/app`. The binary embeds all three (MB-0024's
`build.rs`; `sqlx::migrate!` in `repo/sqlite/mod.rs`), so only the build
stage needs them. `smoke_image.sh` currently requires them in the image
(MB-0022 P1's wording), so P4 changes that check too.

## Conceptual plan

1. **Clarify (S1).** CRP under the dependency-bump runbook (current state,
   target, "do nothing", the six crates), with the image question to the
   owner.
2. Remove the dependencies and rewrite the two tests; then the Dockerfile
   and `smoke_image.sh`.

## Validation plan

- P1: fast gate, `cargo deny check`, `cargo tree -i actix-files`.
- P2, P3: the named tests.
- P4: `smoke_image.sh` in CI's `deploy` workflow on the PR.
