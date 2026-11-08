# Consultation Request Pack — Registering the embedded `/assets` service in `main.rs` (MB-0024)

```yaml
---
id: CRP-0011
mission: MB-0024
owner: "@juuso-kalliomaki"
target_role: security
risk_tier: medium
urgency: low
state: answered
runbook_trigger: "MB-0024 autonomy.escalate: \"replacing `actix-files` or changing how `/assets` is registered in `main.rs` (MB-0018 P4 pins its position)\" (security). feature.md escalation path \"Session middleware in crates/kauppa-web/src/main.rs\" (scripts/agentic/check_triggers.py: security:session-middleware). Secondary, platform-owner: removing `actix-files` (and handlebars' `dir_source` feature) is a manifest change under dependency-bump.md, which MB-0024 doesn't name yet. Not fired: \"using a crate instead of a build.rs\" (no crate added) and \"the binary grows by more than 1 MB\" (it shrinks by 384 KB)."
---
```

## Decision statement

Approve the one-line `main.rs` change that replaces
`.service(Files::new("/assets", "./assets"))` with the embedded asset
service, decide how MB-0018's coverage guard (P4) pins it, and say whether
`actix-files` leaves the manifests in MB-0024 or later.

## Current state

Branch `agentic/mb-0024-embed-files` (from `63da727`), commits `4b39f0d`
and `54f641a`. `main.rs` is untouched.

**Done, no escalation needed:**

- `crates/kauppa-web/build.rs` (new; Cargo picks it up without a manifest
  change) writes two tables to `OUT_DIR`:
  - `TEMPLATES: &[(&str, &str)]`: name and `include_str!` per `.hbs` file,
    named the way `register_templates_directory` names them (`lists/show`),
    hidden and `#` files skipped as handlebars does;
  - `ASSETS: &[(&str, &str, &[u8])]`: path under `/assets/`, content type,
    `include_bytes!`. Content types come from a three-row extension map in
    `build.rs` (`css`, `js`, `webmanifest`). **The build fails** on a file
    in `assets/` whose extension isn't mapped. Hidden files are skipped, as
    `actix-files` refused them.
  - `cargo::rerun-if-changed` on both folders, so editing a template or
    asset rebuilds.
- `templates::build_registry()` registers from `TEMPLATES` (strict mode, same
  names). `main.rs` already calls it, so **templates are embedded on this
  branch** without touching `main.rs`. `build_registry_from(dir)` is gone;
  `routes::test_support` uses the embedded registry, so route tests render
  what production renders.
- `routes::assets::Assets` (new, `crates/kauppa-web/src/routes/assets.rs`):
  - an `HttpServiceFactory`, like `Files`, registering one prefix
    (`ResourceDef::prefix("/assets")`) with an `fn_service`;
  - `GET` and `HEAD` only; every other method gets `405` with
    `Allow: GET, HEAD` (`Files` sent `405` without `Allow`);
  - exact lookup of the path after `/assets/` in `ASSETS`. No percent-decoding,
    no `..` handling, no directory lookup: anything that isn't an embedded
    name exactly is `404`.
  - No handler attribute and no builder call, so the MB-0018 source scan
    (rules 1 and 4) has nothing to flag or to require in `routes::protected`.
  - Not registered anywhere yet. Outside tests it is dead code under
    `#![cfg_attr(not(test), expect(dead_code, reason = …))]`, which becomes
    a build error the moment `main.rs` uses it, so it can't be left behind.

**Tests at the brief's paths** (all pass; `check_brief_tests.sh` selects each):

| Brief | Test | What it proves |
| --- | --- | --- |
| P1, P2 | `templates::tests::registry_is_embedded` | No `./templates` in the test CWD; the embedded registry has exactly the names a directory registry of `templates/` has, and each source is byte-identical to the file. |
| P1, P2 | `routes::assets::tests::serves_every_asset` | No `./assets` in the test CWD; `ASSETS` lists exactly the non-hidden files in `assets/`; each is `200` on `GET` and `HEAD` with the pinned content type and identical bytes; a reference `Files::new("/assets", …)` app returns the same content type (differential check, while `actix-files` is a dependency). |
| P3 | `routes::assets::tests::no_traversal` | In an app wired as `main.rs` would be (`Assets` first, then `routes::configure`): 19 paths, from `..`, `%2e%2e`, `..%2F`, `..%5C`, double-encoded and `%00` to `//`, `./`, trailing `/`, wrong case, dotfiles and `/assets/healthz`, are `404` on `GET` and `HEAD`; `POST`, `PUT`, `PATCH`, `DELETE`, `OPTIONS` and `TRACE`, same-origin or forged, on known and unknown paths, are `405` with `Allow`, never reaching the cross-site scope; `/healthz` still works beside it. |
| P4 | `templates::tests::layout_params_do_not_shadow_body` | MB-0016's guard, unchanged, on the embedded registry, plus a strict-mode check (a missing field fails to render). |

The guard moved from `templates::` to `templates::tests::`, so MB-0016's P5
check path is rewritten in place with a note, per RR-0009 constraint 2.
Please confirm that's acceptable for a closed brief outside MB-0019.

**What's blocked:** `main.rs` still serves `./assets` through `Files`, so a
binary started outside the repo root renders pages (templates embedded) but
serves `404` for `/assets/app.css` and the manifest. P1 is met for templates
only until this consult is answered.

## Options and trade-offs

### Option A — Swap the line in place; the guard pins the new line and checks its methods

`main.rs`:

```diff
-use actix_files::Files;
 …
             // Static files (GET and HEAD only) go first: `routes::configure`
             // puts every route behind the cross-site check, in one catch-all
             // scope (RR-0010).
-            .service(Files::new("/assets", "./assets"))
+            .service(routes::assets::Assets)
             .configure(routes::configure)
```

and the `expect(dead_code)` block comes out of `routes/assets.rs`.

**Why it stays first, outside the scope:** exactly today's shape. `/assets`
is matched before the catch-all `web::scope("")`, so asset requests never
pass `csrf::guard` (they don't need to: safe methods only) and an unknown
`/assets/…` path is `404` from `Assets`, not something the scope could
answer. Order also matters functionally: registered after the scope, the
scope's empty prefix would match `/assets/…` first.

**MB-0018 P4 guard (`csrf::tests::coverage`):**

- Rule 2's expected list changes one string:
  `".service(Files::new(\"/assets\", \"./assets\"))"` →
  `".service(routes::assets::Assets)"`. Order (first, then
  `.configure(routes::configure)`) is still asserted.
- Today rule 2 *trusts* that the static-file service is GET/HEAD-only
  because it is `Files`. With a home-made service that trust should be
  checked. Proposed addition to the guard, beside rule 5: through the real
  wiring, every non-safe method on `/assets/app.css` and `/assets/x` answers
  `405`, never `2xx`. `no_traversal` already does this; the guard would own
  it so a later change to `assets.rs` can't drop it.
- Rules 1, 3, 4 and 5 are unaffected (verified in a scratch copy with the
  change applied: all 62 `kauppa-web` tests pass, clippy clean).

- Pros: one-line, same position and semantics as today; no middleware,
  session or cookie change; `test_app!` doesn't need to change.
- Cons: route tests built with `test_app!`/`bare_test_app!` don't mount
  `/assets` (true today too); behaviour beyond the content type differs from
  `actix-files` (see "Behaviour changes" below).
- Risk: low. The service is read-only over compile-time data.
- Cost: minutes, plus the guard addition.

### Option B — Register assets inside `routes::protected`; `main.rs` registers only `routes::configure`

Replace the factory with a handler attribute, e.g.
`#[route("/{path:.*}", method = "GET", method = "HEAD")]` under
`web::scope("/assets")` in `routes::protected`, and delete the line from
`main.rs`.

- Pros: one registration path. The coverage guard sees the handler natively
  and rule 2 shrinks to `[".configure(routes::configure)"]`. `test_app!`
  serves assets too.
- Cons:
  - every asset request passes `csrf::guard` and the catch-all scope. The
    brief forbids serving assets "behind the catch-all scope's write paths";
    this is close to that line even though the routes are GET/HEAD;
  - `{path:.*}` goes through actix's path extraction and decoding rules, so
    P3's traversal cases need re-proving against a different matcher;
  - a forged `POST /assets/app.css` becomes `403` from the guard instead of
    `405`: harmless, but noisier `warn` logs (MB-0018 P6) for scanner
    traffic.
- Risk: low-medium (new interaction with the guard and matcher).
- Cost: an hour, plus re-running P3 and the guard.

### Option C — Keep `actix-files`, pointed somewhere the binary can find

Resolve the folder relative to `current_exe()` or a new `KAUPPA_ASSETS_DIR`.

- Pros: keeps `ETag`, `Last-Modified`, `Range` and `304`s.
- Cons: doesn't meet the brief's goal ("one binary"; assets still ship
  beside it and can drift from it); a new config variable is a platform
  escalation; still a `main.rs` change.
- Risk: low. Cost: small. Not recommended; listed as the "do less" option.

## Recommendation and question

Recommended option: **A**, with the guard addition.

Because: it changes one line in `main.rs`, keeps the position and the
GET/HEAD-only, outside-the-scope shape MB-0018 reviewed, and turns the
guard's assumption about the static-file service into a check. B mixes
static files into the cross-site scope for little gain; C misses the goal.

**Crisp question to answer:** May MB-0024 replace
`.service(Files::new("/assets", "./assets"))` with
`.service(routes::assets::Assets)` in `main.rs` (A), register assets in
`routes::protected` and drop the `main.rs` line (B), or keep `actix-files`
(C)?

**Sub-questions:**

1. **Guard.** Under A, does the coverage guard get the extra behavioural
   rule (non-safe methods on `/assets/…` are `405`), or is the string pin in
   rule 2 enough? Recommendation: add the rule.
2. **`actix-files` (platform-owner).** After A it is unused outside the
   differential check in `serves_every_asset`. Removing it is a manifest
   change (`Cargo.toml` and `crates/kauppa-web/Cargo.toml`), so the
   dependency-bump runbook applies, and its entry gate wants the brief to
   name it. Options:
   - (i) **later**: MB-0024 keeps it; a follow-up (brief delta or a small
     brief) removes it, drops the differential check (the pinned
     `SERVED_AS` table stays) and handlebars' `dir_source` feature, which
     only `registry_is_embedded`'s comparison still uses;
   - (ii) **now**: amend MB-0024 to name the removal, and approve it in the
     same RR.

   Recommendation: (i). It keeps this PR to one security decision, and the
   removal is a pure deletion that `cargo deny check` can judge on its own.
   Measured: removing it from both manifests drops six crates from
   `kauppa-web`'s tree (`actix-files`, `http-range`, `mime_guess`,
   `unicase`, `v_escape-base`, `v_htmlescape`) and doesn't change the
   release binary (`actix-files` is already unlinked once `main.rs` stops
   using it).
3. **Behaviour changes.** Do you accept these differences from
   `actix-files`, with caching left to a later brief (MB-0024 non-goal)?
   - no `ETag`, `Last-Modified`, `Accept-Ranges` or `Content-Disposition`,
     so no `304` or `Range` responses: browsers refetch 6.9 KB of assets
     when they revalidate;
   - `/assets/x/../app.css` was normalized and served; now `404`;
   - `%2F` in the path and dotfiles were `400`; now `404`;
   - `405` now carries `Allow: GET, HEAD`.

   Recommendation: accept. The brief's P3 asks for exactly the `404`s.
4. **Module location.** `Assets` lives in `routes::assets` so the prototype
   compiles without a `mod` line in `main.rs`. Keep it there, or move it to
   the crate root (`crate::assets`, one more `main.rs` line) next to
   `templates`? The brief check `assets::tests::…` selects either.
   Recommendation: keep it in `routes`; it's an HTTP service.

## Blast radius and rollback

What can break: `/assets/app.css` and `/assets/manifest.webmanifest`, if the
service mismatches (unstyled pages, no install manifest). Nothing else: no
session, cookie, middleware or route change, and writes can't reach it.

Who is affected: every visitor's styling; no data.

Rollback plan: revert the `main.rs` line (and the guard string) to `Files`;
`actix-files` is still a dependency under sub-question 2 (i). The embedded
templates can stay; they don't depend on this answer.

## Minimal context

- `.agentic/missions/MB-0024-embed-templates-and-assets.md`: P1–P4,
  `autonomy.escalate`, `forbidden` ("serve assets outside GET and HEAD, or
  behind the catch-all scope's write paths").
- `crates/kauppa-web/src/main.rs:83` (the `Files` line) and its comment;
  `crates/kauppa-web/src/csrf/tests/coverage.rs` rule 2.
- `.agentic/resolutions/RR-0010-csrf-mechanism.md` (one catch-all scope,
  static files before it).
- `.agentic/runbooks/dependency-bump.md` (entry gate item 1).

## Evidence bundle (Hardened)

- **Gates on the branch:** `scripts/agentic/fast-gate.sh` OK (fmt, clippy
  pedantic + nursery `-D warnings`, `cargo test --workspace --all-targets`,
  `cargo test -p kauppa-core`); `check_brief_tests.sh` selects all four
  MB-0024 checks and MB-0016's rewritten one.
- **Release binary sizes** (`cargo build --release -p kauppa-web`, thin LTO,
  stripped, x86_64 Linux):

  | Build | Bytes | vs base |
  | --- | --- | --- |
  | `63da727` (base) | 9,505,464 | — |
  | this branch (templates embedded, `Files` still in `main.rs`) | 9,503,128 | −2,336 |
  | Option A applied (scratch copy) | 9,121,240 | −384,224 (−4.0 %) |
  | Option A + `actix-files` removed from manifests (scratch) | 9,121,240 | −384,224 |

  Embedded payload: templates 30,871 B (26 files), assets 6,926 B (2 files).
  The brief's 1 MB growth trigger doesn't fire.
- **Smoke, Option A (scratch copy):** the release binary alone in an empty
  directory with a `.env` and `data/`: `GET /` `200 text/html`,
  `GET /auth/login` `200`, `GET /assets/app.css` `200 text/css;
  charset=utf-8` 6,709 B, `GET /assets/manifest.webmanifest` `200
  application/manifest+json` 217 B, `HEAD /assets/app.css` `200` with
  `content-length: 6709` and no body, `GET /assets/../Cargo.toml`
  (`--path-as-is`) `404`, `GET /assets/nope` `404`, `POST /assets/app.css`
  `405`.
- **Content types:** pinned to `actix-files` 0.7.4 (`mime_guess` 2.0.5 plus
  its `prefer_utf8` default, which adds `; charset=utf-8` to `text/css`),
  and checked against a live `Files` service in `serves_every_asset`.
- **Encoded paths:** actix-web's router decodes unreserved characters before
  matching, so `/assets/app%2Ecss` is the same URL as `/assets/app.css` and
  is served (asserted in `no_traversal`); reserved encodings such as `%2F`
  stay encoded and miss.
