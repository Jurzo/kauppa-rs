# Resolution Record — Registering the embedded `/assets` service in `main.rs`

```yaml
---
id: RR-0011
closes: CRP-0011
mission: MB-0024
decided_by: "@juuso.kalliomaki"
role: security
risk_tier: medium
decided_at: 2026-11-08T18:05:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0024-embed-templates-and-assets.md
  - .agentic/consults/CRP-0011-embedded-assets-registration.md
---
```

## Decision and outcome

Approve CRP-0011 **Option A with the guard addition**. In `main.rs`, swap
`.service(Files::new("/assets", "./assets"))` for
`.service(routes::assets::Assets)` in the same place. It stays the first
registration, before `.configure(routes::configure)`, and outside the
catch-all cross-site scope (RR-0010). `use actix_files::Files;` leaves
`main.rs`, and the `expect(dead_code)` block leaves `routes/assets.rs`, in
the same commit.

Sub-answers:

1. **Guard.** Add the behavioural rule as well as changing the string.
   - Rule 2 of `csrf::tests::coverage` expects
     `".service(routes::assets::Assets)"` in place of the `Files` string. It
     still asserts the order: first, then `.configure(routes::configure)`.
   - A new rule 6, next to rule 5: through the real wiring, every non-safe
     method (`POST`, `PUT`, `PATCH`, `DELETE`, `OPTIONS`, `TRACE`) on a known
     path (`/assets/app.css`) and an unknown one (`/assets/x`) answers `405`
     with `Allow: GET, HEAD`, never `2xx` and never the cross-site check's
     `403`. The guard owns this, so a later change to `assets.rs` can't drop
     it. `no_traversal` keeps its own copy.
2. **`actix-files` (platform-owner): (i), later.**
   - MB-0024 keeps `actix-files` in both manifests. It also keeps
     handlebars' `dir_source` feature.
   - The removal is a follow-up (a brief delta or a small brief) that names
     it, as the dependency-bump runbook's entry gate (item 1) requires. That
     follow-up:
     - removes `actix-files` from `Cargo.toml` and
       `crates/kauppa-web/Cargo.toml`;
     - drops the differential `Files` check in `serves_every_asset` and keeps
       the pinned `SERVED_AS` table;
     - drops `dir_source` and the directory comparison in
       `registry_is_embedded` that needs it.
   - MB-0024's PR changes no manifest.
3. **Behaviour changes: accepted.** Caching stays a later brief (MB-0024
   non-goal). These differences from `actix-files` are accepted:
   - no `ETag`, `Last-Modified`, `Accept-Ranges` or `Content-Disposition`, so
     no `304` and no `Range` responses;
   - paths `actix-files` normalized, such as `/assets/x/../app.css`, are now
     `404`;
   - `%2F` in the path and dotfiles, which were `400`, are now `404`;
   - `405` now carries `Allow: GET, HEAD`.
4. **Module location.** `Assets` stays in `routes::assets`. It's an HTTP
   service, and keeping it there adds no `mod` line to `main.rs`.
5. **MB-0016's P5 check path: confirmed.** MB-0024 rewrote the closed brief
   MB-0016's P5 check path in place, with its note, when the layout guard
   moved to `templates::tests`. That's acceptable under RR-0009
   constraint 2, even though the move is outside MB-0019. The note already
   in MB-0016 stays as written.

Covers:

- MB-0024's implementation PR, with P1–P4 as written;
- the one-line `main.rs` swap and the removal of its `Files` import;
- the coverage guard's rule 2 string and its new rule 6;
- the behaviour changes in sub-answer 3;
- the MB-0016 check-path rewrite already on the branch.

Does not cover:

- removing `actix-files` or handlebars' `dir_source` from any manifest (the
  follow-up, under the dependency-bump runbook);
- registering assets inside `routes::protected` or the cross-site scope
  (Option B);
- a new configuration variable or asset directory (Option C);
- cache headers, fingerprinting or compression;
- any other `main.rs` change: session middleware, cookies and middleware
  order stay as they are;
- MB-0027's vendored htmx file, which needs its own platform decision.

## Rationale and trade-offs

- **A — chosen.**
  - One line in `main.rs`. The position and shape (first, GET/HEAD only,
    outside the scope) are the ones MB-0018 reviewed.
  - No middleware, session, cookie or `test_app!` change.
  - The new rule 6 makes the guard check, rather than assume, that the
    static-file service accepts no write method. That assumption was safe
    while the service was `Files`; it isn't for a service written here.
  - Read-only over compile-time data; rollback is reverting one line and one
    guard string.
- **B — not chosen.**
  - Every asset request would pass `csrf::guard` and the catch-all scope.
    That's close to the brief's `forbidden` line ("behind the catch-all
    scope's write paths").
  - `{path:.*}` goes through actix's path extraction and decoding, so P3's
    traversal cases would need proving again against a different matcher.
  - A forged `POST /assets/app.css` would become a logged `403` (MB-0018
    P6) instead of a quiet `405`, which adds noise from scanner traffic.
- **C — not chosen.** It misses the brief's goal ("one binary"), because
  the folder still ships beside the binary and can drift from it. It also
  needs a new configuration variable, which is a platform escalation.
- **`actix-files` later (sub-answer 2).** This PR keeps one security
  decision. The removal is a pure deletion that `cargo deny check` can judge
  on its own. Measured in CRP-0011, it drops six crates from `kauppa-web`'s
  tree and doesn't change the release binary.
- **Behaviour changes (sub-answer 3).** The `404`s are what P3 asks for.
  Losing revalidation costs a refetch of 6.9 KB of assets.

## Approvals and authority

Approved by security-owner @juuso.kalliomaki (main question and
sub-answers 1, 3, 4 and 5); sub-answer 2 (`actix-files` deferral) by
platform-owner @juuso.kalliomaki.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0011-embedded-assets-registration.md`
- Mission Brief: `.agentic/missions/MB-0024-embed-templates-and-assets.md`
- Continuity Pack: `.agentic/continuity/MB-0024.md`
- Earlier decisions:
  - RR-0010: one catch-all scope, with static files registered before it;
  - RR-0009 constraint 2: closed briefs' check paths are rewritten in place;
  - MB-0018 P4: the coverage guard.
- Evidence links:
  - CRP-0011's Current state: branch `agentic/mb-0024-embed-files`, commits
    `4b39f0d` and `54f641a`; `main.rs` untouched; all four MB-0024 checks
    pass.
  - CRP-0011's Evidence bundle:
    - fast gate green;
    - `check_brief_tests.sh` selects all four MB-0024 checks and MB-0016's
      rewritten P5;
    - release sizes: base 9,505,464 B; Option A 9,121,240 B (−384,224 B);
    - a scratch copy with Option A applied: smoke results, and all 62
      `kauppa-web` tests pass with clippy clean.
  - **MB-0027's vendored file**, checked by reading `crates/kauppa-web/build.rs`
    and `routes/assets.rs` at `611043f`. MB-0027 suggests the file live at
    `assets/vendor/htmx-2.0.3.min.js`, and its P3 needs a JavaScript content
    type there.
    - `build.rs`'s `CONTENT_TYPES` maps `js` to `text/javascript`. That is
      what `actix-files` 0.7.4 sent: `mime_guess` 2.0.5 maps `js` to
      `text/javascript`, and `actix-files`' UTF-8 step only adds a charset to
      `application/javascript`.
    - `build.rs`'s `files()` walks subfolders and names each file by its
      `/`-separated path under `assets/`. Only segments starting with `.` are
      skipped, so `vendor/htmx-2.0.3.min.js` is embedded.
    - `Assets` looks up the whole path after `/assets/` exactly, so
      `/assets/vendor/htmx-2.0.3.min.js` resolves.
    - MB-0027 still has to add a `SERVED_AS` row in `routes/assets/tests.rs`
      (`text/javascript`). `serves_every_asset` fails on an asset without
      one, as intended.
    - Not exercised end to end, since there is no nested or `.js` asset on
      the branch yet; MB-0027's P3 covers that.

## Constraints and follow-on rules (Hardened)

1. **Static files stay first and outside the scope.** `main.rs` registers
   exactly `.service(routes::assets::Assets)` and then
   `.configure(routes::configure)`. Moving assets into the cross-site scope
   or `routes::protected`, or putting anything between the two, needs an RR.
2. **`/assets` is read-only.** `Assets` answers `GET` and `HEAD` only.
   Every other method gets `405` with `Allow: GET, HEAD`. The coverage
   guard's rule 6 enforces this through the real wiring. Weakening or
   removing rule 6 needs an RR.
3. **Exact names only.** `Assets` serves an embedded name exactly, with no
   decoding, normalizing, directory lookup or filesystem fallback. Anything
   else is `404`. A directory or filesystem fallback needs an RR.
4. **Content types are pinned.** Every asset's content type comes from
   `build.rs`'s `CONTENT_TYPES`, which fails the build on an unmapped
   extension, and from a row in `SERVED_AS`. Adding a file type needs a
   row in each, in the same PR.
5. **`actix-files` leaves through its own change.** Until the follow-up in
   sub-answer 2 merges, `actix-files` may only be used by the differential
   check in `serves_every_asset`. No production code may use it again.

## Roll-back into the system (Hardened)

To be applied in MB-0024's implementation PR:

- `MB-0024`:
  - `resolutions: []` → `resolutions: [RR-0011]`;
  - after the front matter, add the note: "`/assets` registration and the
    coverage guard's rule 6 per RR-0011. `actix-files` and handlebars'
    `dir_source` stay; their removal is a follow-up under the dependency-bump
    runbook (RR-0011 sub-answer 2)."
- `CRP-0011`: `state: open` → `answered`.
- `crates/kauppa-web/src/csrf/tests/coverage.rs`: in the module doc, rule 2
  reads "`main.rs` registers nothing besides `routes::assets::Assets` and
  `routes::configure`", and a new item reads "6. behind the real wiring,
  every non-safe method on `/assets/…` answers 405 with `Allow: GET, HEAD`,
  so the static files accept no write". The code changes to match
  (sub-answer 1).
- `.agentic/continuity/MB-0024.md`: record the follow-up for `actix-files`
  and `dir_source` under Open questions and next steps until it is briefed.

No Mentorship Pack or Runbook changes. `AGENTS.md`'s "`assets/` is served
directly" still holds; only how it is served changes.

## Supersession and history

Status: active once approved. Supersedes nothing. Keeps RR-0010's layout
(static files before the one catch-all scope) and replaces the `Files`
service with `routes::assets::Assets`. Applies RR-0009 constraint 2 to
MB-0016's P5 outside MB-0019.
