# Consultation Request Pack — Remove `actix-files`, handlebars' `dir_source`, and the runtime image's file copies

```yaml
---
id: CRP-0020
mission: MB-0032
owner: "@juuso-kalliomaki"
target_role: platform
risk_tier: low
urgency: low
state: open
runbook_trigger: ".agentic/runbooks/dependency-bump.md (\"Changing enabled features\": handlebars `dir_source`; removing a crate: `actix-files`; entry gate 2–3: CRP open, RR approved before the PR); CI trigger paths `Cargo.toml`, `crates/kauppa-web/Cargo.toml`, `deploy/Dockerfile`; RR-0011 sub-answer 2 and constraint 5; MB-0032 autonomy.escalate (the removal → platform-owner; the image contents and MB-0022 P1's wording → owner)"
---
```

## Decision statement

Approve removing `actix-files` and handlebars' `dir_source` feature, which
only two tests still use. This is the follow-up RR-0011 sub-answer 2
deferred.

Sub-question (owner): stop copying `templates/`, `assets/` and
`migrations/` into the runtime image, and reword MB-0022 P1's image check
to match.

## Current state

- **Manifests:**
  - `Cargo.toml`: `actix-files = "0.7.4"`, `handlebars = { version = "6", features = ["dir_source"] }`;
  - `crates/kauppa-web/Cargo.toml`: `actix-files = { workspace = true }`.
  - Locked: actix-files 0.7.4, handlebars 6.4.4 (with `walkdir` 2.5.0
    through `dir_source`).
- **Call sites** (test code only, since MB-0024, RR-0011 constraint 5):
  - `crates/kauppa-web/src/routes/assets/tests.rs`:
    - `use actix_files::Files;`;
    - `serves_every_asset` builds a reference
      `App::new().service(Files::new("/assets", source_dir()))` and, for
      each asset, checks that it answers `200` with the same pinned
      content type (the "differential check").
  - `crates/kauppa-web/src/templates/tests.rs`: `registry_is_embedded`
    builds a second registry with `register_templates_directory(source_dir(),
    DirectorySourceOptions::default())` and compares its template names
    with the embedded registry's.
  - Comments only: `build.rs` (lines 10, 32, 56), `routes/assets.rs`
    (line 4), `templates.rs` (line 13), `routes/assets/tests.rs` (lines 14,
    56, 92). These record where the content types and names came from.
    They stay true as history, and the brief lets the wording be adjusted.
- **Production:** `main.rs` registers `routes::assets::Assets` (embedded),
  and `templates::build_registry()` registers from `TEMPLATES` (embedded).
  Neither crate is used outside tests.
- **Image:** `deploy/Dockerfile`'s runtime stage runs `WORKDIR /app`, then
  `COPY templates ./templates`, `COPY assets ./assets` and
  `COPY migrations ./migrations` (144 KB, 38 files). Its comment, "Templates
  load from ./templates … until MB-0024 embeds them", is stale. The
  builder stage needs all three: `build.rs` embeds `templates/` and
  `assets/`, and `sqlx::migrate!` embeds `migrations/`.
- **Smoke check:** `scripts/deploy/smoke_image.sh` (MB-0022 P1):
  - `contents_ok` requires `/app/templates/layout.hbs`,
    `/app/assets/app.css` and `/app/migrations/0001_init.sql`;
  - `app_files_read_only` checks `/app/templates` and `/app/assets`;
  - the header comment and the check title name them.
  - MB-0022 (closed, close-out PR #44 merged) P1's property reads
    "contains the binary, `templates/`, `assets/` and `migrations/` but no
    source tree, …".
- `deploy/README.md` (line 269) already says the copies are no longer
  needed after MB-0024, and that changing the Dockerfile is an owner
  decision.

## Target state

1. Both `actix-files` lines are gone, and handlebars has no features
   (`handlebars = "6"`; its default feature set is empty). The lockfile
   change is pure removal: `cargo update -w`, no version moves.
2. `serves_every_asset` (P2) keeps everything except the reference `Files`
   app and its block:
   - no `./assets` in the CWD;
   - the embedded list equals the files on disk;
   - `200` on `GET`/`HEAD` with the pinned `SERVED_AS` content type;
   - bytes equal to disk;
   - a file without a `SERVED_AS` row panics.

   The content types were pinned to what `actix-files` sent (RR-0011
   constraint 4), so the pin stays the contract and the differential check
   goes.
3. `registry_is_embedded` (P3) compares the embedded names with a
   `names_on_disk()` helper in the test (`std::fs::read_dir` walk, like
   `files_on_disk()` in the assets tests). The helper applies the
   directory source's rules: `.hbs` only, `/`-separated stem, and hidden
   `.x` and temporary `#x` files skipped. Strict mode and the byte-for-byte
   comparison stay.
4. Image (owner sub-question): the runtime stage drops the three `COPY`
   lines and the stale comment. `WORKDIR /app` either stays as an empty
   working directory or goes (question 3).
   - `contents_ok` asserts that `/app/templates`, `/app/assets` and
     `/app/migrations` are **absent**, alongside the source tree.
   - `app_files_read_only` keeps only the binary.
   - MB-0022 P1's property says "contains the binary and no `templates/`,
     `assets/` or `migrations/` directory, source tree, toolchain or
     `.env`" (question 4 for how).

## Options and trade-offs

### Option A — remove both now, image change included (recommended)

- Pros:
  - finishes MB-0024: one binary, no files beside it that could drift from
    what it embeds (AGENTS.md goal 3);
  - 9 fewer lockfile packages, with no new ones;
  - the smoke check asserts that the image no longer depends on the
    folders.
- Cons:
  - loses the differential check against `actix-files`, which is only
    meaningful while `actix-files` exists;
  - MB-0022's closed P1 wording changes.
- Risk: low. Test-only code and a runtime image that holds nothing the
  binary reads. The release binary is effectively unchanged (−3,104 B).
- Cost: under an hour, plus a CI `deploy` run for `smoke_image.sh`.

### Option B — remove the crates, keep the image as is

- Pros: no MB-0022 wording change.
- Cons:
  - 144 KB of dead files in every image;
  - the smoke check keeps requiring files nothing reads, which misleads
    the next reader;
  - MB-0032 P4 is unmet.

### Option C — do nothing

- Pros: no work.
- Cons:
  - two dependencies kept for tests (`actix-files` ships advisories:
    CRP-0007 cleared five in 2026-11);
  - `dir_source` keeps `walkdir` in the tree;
  - RR-0011's follow-up stays open.

## Recommendation and question

Recommended option: **A**.
Because: both crates serve only checks that compare the embedded copy with
what it replaced. The pinned tables are now the contract (RR-0011
constraint 4), and the image copies are dead weight the smoke check
wrongly requires.

Crisp question to answer: **Approve removing `actix-files` (both
manifests) and handlebars' `dir_source`, with the two tests rewritten as in
Target state 2–3, and the runtime image no longer carrying `templates/`,
`assets/` and `migrations/`?**

Numbered sub-questions:

1. **Removal (platform):** approve the two manifest edits and the
   lockfile removal of the 9 packages below, as two commits (manifest
   edit, then `cargo update -w`) per the runbook gate?
2. **Tests (platform):** accept dropping the differential `Files` check
   (P2) and replacing the `dir_source` comparison with a `read_dir` walk
   (P3), as RR-0011 sub-answer 2 described?
3. **Image (owner):** drop the three runtime `COPY` lines. Keep
   `WORKDIR /app` as the empty working directory (smallest change), or
   drop it so the process runs in `/`?
4. **MB-0022 P1 wording (owner):** rewrite MB-0022's closed P1 property in
   place with a note ("Image contents updated by MB-0032 (RR-00xx)"), as
   RR-0009 constraint 2 does for check paths? Or leave MB-0022 as history
   and let MB-0032 P4 be the current contract, with only `smoke_image.sh`'s
   comment changing?
5. **`deploy/README.md` (owner):** replace the "After MB-0024" note at
   line 269 with a short "the image holds only the binary" line in the
   same PR?

## Blast radius and rollback

What can break:

- nothing at runtime from the crate removal (test-only use; the release
  binary differs by 3 KB);
- the image: a code path that still read `./templates`, `./assets` or
  `./migrations` at runtime would fail. None exists (MB-0024; the trial's
  tests run from the crate directory, which has none of the three), and
  `smoke_image.sh` serves the login page, `/assets/app.css` and a user's
  lists from the trimmed image.

Who is affected: developers (lockfile), the deploy image.
Rollback plan: revert the PR. The lockfile restores the 9 packages at
their current versions, and the Dockerfile gets its `COPY` lines back.

## Supply-chain check

Removal only: no crate is added or upgraded, and no version of a
remaining crate moves (trial `cargo update -w`: 9 × "Removing", nothing
else).

| Package leaving `Cargo.lock` | Version | Pulled in by |
| --- | --- | --- |
| actix-files | 0.7.4 | kauppa-web (direct) |
| http-range | 0.1.6 | actix-files |
| mime_guess | 2.0.5 | actix-files |
| unicase | 2.10.0 | mime_guess |
| v_htmlescape | 0.17.0 | actix-files |
| v_escape-base | 0.1.0 | v_htmlescape |
| walkdir | 2.5.0 | handlebars `dir_source` |
| same-file | 1.0.6 | walkdir |
| winapi-util | 0.1.11 | actix-files, same-file, walkdir (Windows-only targets) |

- CRP-0011 counted six crates for `actix-files` alone, on the Linux target. `winapi-util` is also shared by `actix-files`, and `dir_source` adds
  `walkdir` and `same-file`.
- `cargo deny check` after removal: advisories, bans, licenses and sources
  ok. The same 8 `unnecessary-skip` warnings appear as on `main`.
- `deny.toml`'s `skip` comment ("handlebars/walkdir pull windows-sys
  0.48") is already stale on `main`. Tidying it is out of scope: lint
  config is a trigger path, and MB-0032 forbids other changes.

## Minimal context

- `.agentic/missions/MB-0032-remove-file-serving-leftovers.md`, `.agentic/continuity/MB-0032.md`
- RR-0011 (sub-answer 2, constraints 4–5), CRP-0011 (six-crate measure),
  RR-0009 constraint 2 (closed briefs rewritten in place)
- `Cargo.toml`, `crates/kauppa-web/Cargo.toml`, `crates/kauppa-web/build.rs`,
  `crates/kauppa-web/src/routes/assets/tests.rs`,
  `crates/kauppa-web/src/templates/tests.rs`, `deploy/Dockerfile`,
  `scripts/deploy/smoke_image.sh`, `.agentic/missions/MB-0022-first-deployment.md` (P1)

## Evidence bundle (Hardened)

Trial (2026-11-09) in a scratch copy of `main` at 50322c0, with Target
state 1–3 applied, then deleted. Not committed.

| Check | Result |
| --- | --- |
| `cargo update -w` | removes the 9 packages above; nothing added or updated |
| `cargo tree -i actix-files` | "did not match any packages" |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | clean |
| `cargo test -p kauppa-web --all-targets` | 96 passed, including `serves_every_asset` and `registry_is_embedded` (same count as `main`) |
| `cargo deny check` | advisories, bans, licenses, sources ok |
| Release binary (`cargo build --release -p kauppa-web`, thin LTO, stripped, x86_64 Linux) | `main` 9,327,064 B → trial 9,323,960 B (−3,104 B) |

- The image was not built in the trial: the change is three `COPY` lines,
  and `smoke_image.sh` in CI's `deploy` workflow is P4's check.
- Runtime image payload today: `templates/` 62,265 B (27 files), `assets/`
  66,009 B (3 files, including vendored htmx), `migrations/` 16,620 B
  (8 files).
- Nothing else in `deploy/` refers to `/app` or the three folders
  (`docker-entrypoint.sh`, `healthcheck.sh`, `compose.yaml`). The root
  `.dockerignore` must still admit them for the builder stage, so it
  stays.
