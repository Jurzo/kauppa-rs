# Consultation Request Pack — Code cleanup after the audit: the `sqlx` manifest move, the unused list route, splitting `repo/sqlite/list.rs` under the bypass guard, plain error bodies

```yaml
---
id: CRP-0036
mission: MB-0048
owner: "@juuso-kalliomaki"
target_role: security
risk_tier: medium
urgency: low
state: answered
runbook_trigger: "MB-0048 autonomy.escalate (`Cargo.toml` → platform; removing a route → owner; the list-query guard → security; error bodies and `error.rs` → owner); feature.md escalation triggers `crates/*/Cargo.toml`, `crates/kauppa-web/src/routes/lists/`, `crates/kauppa-core/src/repo/sqlite/list.rs` and `.agentic/resolutions/**`; dependency-bump.md entry gate"
---
```

## Decision statement

Decide, for MB-0048: whether `sqlx` moves to `kauppa-web`'s
dev-dependencies (platform); whether `POST /lists/{id}/favorites` is removed
(owner); how `repo/sqlite/list.rs` is split and what changes in the bypass
guard and in RR-0009 constraint 3 (security); whether plain error bodies
are translated or stay English by a written decision (owner); and the brief
amendments that follow from re-checking the audit against today's `main`.

## Terms

- **Bypass guard**: the test
  `repo::share::list_queries_route_through_central_predicate`
  (`crates/kauppa-core/src/repo/share.rs:687-782`). It reads repository
  source and fails if a list query can run without the access predicate.
- **Access predicate**: `ACCESS_SELECT` and `authorize`
  (`repo/sqlite/list.rs:33-127`). `authorize` returns a `ListGrant`, and
  every query on a list's rows takes one.
- **Plain body**: an error response whose body is a short text with no HTML
  and no `Content-Type` of its own, such as `not found`.

## Current state: the audit re-checked

Checked on `main` at b800583 (2026-11-14). The audit ran on 2026-11-10.
MB-0035, MB-0036 and MB-0051 to MB-0054 have merged since.

| # | Audit finding | Today | Evidence |
| --- | --- | --- | --- |
| 1 | `kauppa-web` lists `sqlx` under `[dependencies]` | Still true | `crates/kauppa-web/Cargo.toml:28` |
| 2 | Only tests use it (`error.rs`, `admin/bootstrap.rs`) | Still true, but in four files, not two | See P1 below |
| 3 | `POST /{id}/favorites` is registered and no template posts to it | Still true | `routes/lists/items.rs:137`, `routes/mod.rs:58` |
| 4 | `repo/sqlite/list.rs` is 801 lines | Changed: 855 lines | MB-0052 added to it |
| 5 | Several test files are at 700 to 780 | Changed: `templates/tests.rs` is now 821, over the guideline | See P3 below |
| 6 | `InvitationState::as_str` has no caller in `kauppa-web` | Still true, and no caller anywhere | `domain/invitation.rs:41` |
| 7 | `PasswordLengthError`'s `Display` is no longer shown | Still true. One unit test still reads it | `domain/password_rule.rs:33-42`, `:124-130` |
| 8 | `Entity::Favorite`, `Ingredient` and `Recipe` have keys no code raises | Changed: only `Favorite` is never raised. 15 of the 28 entity keys are unreachable, across six entities | See P4 below |
| 9 | `error.rs` bodies and the cross-site 403 are plain English | Still true | See P5 below |

### P1: `sqlx` in `kauppa-web`

Every use is test code:

- `src/error.rs:96`, inside `#[cfg(test)] mod tests` (from line 62).
- `src/admin/bootstrap.rs:140-153`, inside `#[cfg(test)] mod tests` (from
  line 134).
- `src/routes/auth/tests/rate_limit.rs` (lines 14, 79, 80 and six
  `sqlx::query` calls).
- `src/routes/admin/tests/access.rs:10, 129, 132`.

`sqlx` is 0.8.6 in `Cargo.lock`, declared once in the workspace manifest
(`Cargo.toml:51`) and used by `kauppa-core` (`crates/kauppa-core/Cargo.toml:15`).

Tried on a scratch copy of b800583, never in the repository: with the
`sqlx` line moved from `[dependencies]` to `[dev-dependencies]`,
`cargo check -p kauppa-web --locked`, `cargo check --workspace --all-targets
--locked` and clippy with `-D warnings` all pass, `Cargo.lock` is
byte-identical, and `cargo deny check` reports all four sections ok.

### P2: `POST /lists/{id}/favorites`

- Registered at `routes/lists/items.rs:137` (`add_favorite`) and
  `routes/mod.rs:58`. Its comment (`items.rs:134-136`) says it stays "for
  pages loaded before" MB-0036. MB-0036's pack records that as the agent's
  choice and names removal as a follow-up.
- No template has the form. Two tests assert its absence:
  `routes/lists/tests/add_field.rs:80` and `sharing_roles.rs:153`.
- No mention in `README.md`, `deploy/`, `design-system/`, `scripts/` or
  `.github/`.
- Tests that post to it directly: `routes/lists/tests/items.rs` lines 63
  and 105 (`owner_isolation`), 176, 210, 217 and 255 (`htmx_fragments`);
  `tests/mod.rs:130` (the `add_favorite` row of `list_posts`, used by the
  role tests); `tests/editor_match.rs:70` (RR-0024 sub-answer 3: the editor
  gets 403). The CSRF coverage test finds routes by scanning the source, so
  it follows a removal with no edit.
- What else only this route uses: `FavoriteForm` (`items.rs:29`),
  `parse_product` (`items.rs:66-73`), and through it
  `Invalid::ChooseFavorite` with its key `errors.choose_favorite`
  (`locales/en.json:51`, `fi.json:51`).
- `ShoppingListRepo::add_product` is still used by the add field
  (`items.rs:110`). It stays.

### P3: file sizes

`wc -l` over `crates/**/*.rs`, files over 700 lines:

| Lines | File |
| --- | --- |
| 855 | `crates/kauppa-core/src/repo/sqlite/list.rs` |
| 821 | `crates/kauppa-web/src/templates/tests.rs` |
| 782 | `crates/kauppa-core/src/repo/share.rs` (holds the bypass guard) |
| 779 | `crates/kauppa-web/src/routes/lists/tests/sharing_roles.rs` |
| 763 | `crates/kauppa-web/src/routes/assets/tests.rs` |
| 723 | `crates/kauppa-web/src/i18n/tests.rs` |
| 713 | `crates/kauppa-web/src/routes/auth/tests/rate_limit.rs` |

Two files exceed 800. The brief names only the first.

How `list.rs` is laid out:

- Lines 24-127: `ACCESS_SELECT`, `Accessible`, `accessible_lists`,
  `ListGrant`, `authorize`.
- Lines 129-216: `ITEM_SELECT`, `STORE_VIEW_SELECT`, `SHARE_SELECT`,
  `fetch_items`, `fetch_item`, `touch_list`.
- Lines 218-248: `begin_read` and `begin_write` (an inherent
  `impl SqlitePool`).
- Lines 250-855: `impl ShoppingListRepo for SqlitePool`, 22 methods, 605
  lines. Rust requires a trait implementation to be one block in one file.

What the guard reads today:

- `LIST_RS = include_str!("sqlite/list.rs")` and
  `MOD_RS = include_str!("mod.rs")` (`share.rs:624-625`).
- Rule 1: every method in the `impl ShoppingListRepo` block except `create`
  calls `self.begin_read(`, `self.begin_write(` or `accessible_lists(`
  before any `sqlx::query`.
- Rule 2: the number of methods checked equals the number the trait
  declares.
- Rule 3: in `list.rs`, a top-level item that names a list table or uses a
  row `SELECT` must be one of seven allowed headers, and an `async fn`
  among them must take `grant: &ListGrant`.
- Rule 4: every other `.rs` file directly in `repo/sqlite/`
  (`other_sqlite_files`, which skips only `list.rs`) names no list table
  and none of the four `SELECT` constants.

One gap today: `ListGrant`'s fields are private to the module, and the
whole of `list.rs` is that module. Any code in the file could write
`ListGrant { … }` without calling `authorize`. Nothing does, and rule 1
would still demand a gate call, but neither the compiler nor the guard
forbids it.

### P4: leftovers of MB-0035

Checked by `grep` and by deleting each item locally, running
`cargo check`, then reverting (nothing committed):

- **`InvitationState::as_str`**: no caller. With it deleted, the workspace
  and all test targets compile.
- **`PasswordLengthError`'s `Display`** (and the `std::error::Error` impl
  that needs it): the only use is the unit test
  `domain::password_rule::tests::one_message_names_both_bounds`
  (`password_rule.rs:124-130`). No brief names that test as a check.
  `kauppa-web` only calls `.is_err()` (`routes/auth/mod.rs:278`) and shows
  `Message::PasswordLength` from the catalog.
- **`Entity` variants.** What non-test code raises:

  | Reason | Raised for | Never raised for |
  | --- | --- | --- |
  | `NameEmpty` | category, item, list, product | favorite, ingredient, recipe |
  | `TooMany` | category, ingredient, product | favorite, item, list, recipe |
  | `Unknown` | category, list, product, recipe | favorite, ingredient, item |
  | `Conflict::Exists` | category, product | favorite, ingredient, item, list, recipe |

  `Entity::Ingredient` is raised at `repo/sqlite/list.rs:517` and
  `recipe.rs:332`, and `Entity::Recipe` at `routes/lists/items.rs:61`, so
  the brief's wording is stale for those two. `Entity::Favorite` is raised
  nowhere. With it deleted, everything compiles, and one test fails on its
  hard-coded count (`i18n::messages::tests::every_text_covers_every_reason`,
  `3 * 7`).
- **Catalog keys.** Each catalog has 28 entity keys (four reasons times
  seven entities). 13 are reachable and 15 are not. The types allow all 28,
  because a reason takes any `Entity`, and `messages::every_text` asks for
  a key for each.
- **The named check can't see this.** `i18n::tests::catalogs_complete`
  passed during the experiment with the four `favorite` keys still in both
  catalogs. It checks that used keys exist, not that existing keys are
  used.

### P5: plain error bodies

From `crates/kauppa-web/src/error.rs:49-58`, and the cross-site check at
`csrf/mod.rs:211`:

| Status | Body | When a user sees it |
| --- | --- | --- |
| 404 | `not found` | Normal use: following a link to a list or recipe that was deleted, or to a list whose share was revoked (RR-0002 constraint 4); a non-admin opening `/admin` (`routes/admin/mod.rs:68`). On a plain navigation the browser shows the bare text. On an htmx request nothing is swapped, so the user sees no change |
| 403 | `forbidden` (`error.rs:55`) | Rare: a `view` grantee has no write controls, so this needs a forged request or a page left open while the role was lowered from `edit` |
| 403 | `forbidden` (`csrf/mod.rs:211`) | Forged cross-site requests. Also every form post by every user if `KAUPPA_PUBLIC_ORIGIN` is set wrong, and a browser or proxy that strips both `Origin` and `Referer` |
| 401 | `unauthorized` (`error.rs:54`) | Almost never: only when the session holds an identity that isn't a UUID (`routes/request.rs:13-18`) |
| 400 | the reason's English text, such as `unknown item` | Only where a handler doesn't catch `Invalid`. The list, recipe, product, category, share and invitation forms catch it and render the translated message in the page. What is left needs a forged request, for example `POST /language` with an unsupported value (`routes/language.rs:47`) |
| 409 | the conflict's English text | As 400: the forms that can conflict catch it |
| 500 | `internal server error` | A server fault |

Three more plain or empty answers come from the framework, not from
`error.rs`. The brief doesn't name them. Read from the crates' source, not
run:

- **A signed-out request to a signed-in page is a 401 with an empty body.**
  `actix-identity` 0.8.0 builds it in `Identity`'s extractor
  (`identity.rs:262-270`). This is the most common of all these cases: it
  is what a user gets when a sign-in ends after 30 idle days (RR-0026).
- An unknown URL is `actix-web`'s 404 with an empty body.
- A path id that isn't a UUID, or a malformed form, gets `actix-web`'s
  English text (`Path deserialize error: …`, `Parse error: …`).

`templating.md:30-31` says core errors are "never English text shown as
is". `error.rs:50` cites RR-0023 sub-answer 10 for its exception, but no
Mentorship Pack records the exception.

### The brief's named checks

All five `cargo test` checks select exactly one test today (`-- --list`).
Two don't prove their property:

- **P1** names `register_requires_invite_no_bypass`. That test scans for
  `UserRepo::create` (RR-0012). It says nothing about `sqlx`.
- **P4** names `catalogs_complete`, which passes with unused keys present
  (above).
- **P5** names `error::tests::responses_unchanged`. It fits only if the
  bodies stay English: it asserts them byte for byte.
- P2 and P3 fit.

## Options and trade-offs

### Q1: `sqlx` moves to dev-dependencies (platform)

**A (recommended): move the line.** `sqlx = { workspace = true }` leaves
`[dependencies]` and joins `[dev-dependencies]` in
`crates/kauppa-web/Cargo.toml`. No other manifest changes.

- Pros: the compiler then enforces `persistence.md:60` ("the web layer
  never imports `sqlx`"): non-test code that names it stops compiling.
- Cons: none found. Same version, same features, same lock file.
- Risk: low. Cost: one line moved.

**B: remove it from `kauppa-web`.** The four test files would need a
raw-SQL helper exported from `kauppa-core`. That widens core's public
surface for tests only. Not recommended.

**C: do nothing.** The rule stays a convention.

What `dependency-bump.md` requires for a move with no version change:

- Its "When to use" list (new crate, major bump, replacement, features)
  doesn't name a move. Its first line makes any `crates/*/Cargo.toml`
  change a trigger path, and `check_triggers.py` fires
  `platform:deps(workspace)` on it. So the runbook applies.
- Entry gate: the brief names the change (P1); a consult is open with
  current state, target state and alternatives (this section); an RR is
  approved **before the PR is opened**; the Continuity Pack is current.
- Supply-chain check: no package enters or leaves the build. `sqlx` 0.8.6
  stays as locked; `cargo deny check` passes with the move. Its one ignored
  advisory, RUSTSEC-2026-0459 (RR-0016, review by 2026-12-31), is
  unchanged and belongs to MB-0041.
- Gates: `cargo deny check` passes; no new `#[allow]`. The rule about a
  separate `cargo update -p` commit doesn't apply: there is no version
  change and no `cargo update`.

### Q2: removing `POST /lists/{id}/favorites` (owner)

**A (recommended): remove it.** Delete `add_favorite`, `FavoriteForm` and
`parse_product`, and the registration at `routes/mod.rs:58`.

- A request to the old URL then gets `actix-web`'s 404 with an empty body.
- The only possible caller is a list page loaded before MB-0036 was
  deployed and never navigated since, or a script the owner wrote. No
  page, document or test of a real flow uses it.
- Tests: the `/favorites` cases leave `owner_isolation` and
  `htmx_fragments` (MB-0004 P5 and P6), the `add_favorite` row leaves
  `list_posts`, and the by-id 403 block leaves
  `editor_match_store_view_and_id_add`. Those tests keep their names and
  their other assertions, so no brief check path changes. The two
  assertions that the page has no such form stay.
- RR-0024 sub-answer 3 says the route "stays owner-only". With no route,
  nobody can add by product id over HTTP. `ShoppingListRepo::add_product`
  stays owner-only, proven by `repo::list::favorite_match` (line 173) and
  `repo::share`. The RR should note that it amends RR-0024 sub-answer 3 in
  that one respect.
- `Invalid::ChooseFavorite` and `errors.choose_favorite` lose their only
  producer and go with it, under P4.
- Pros: one write route fewer on the shared-list surface; less dead code.
- Cons: edits tests that closed briefs name (assertions removed, none
  weakened for a route that still exists).
- Risk: low. Cost: small.

**B: keep it, with a comment and a test naming its user.** There is no
user to name. Not recommended.

### Q3: splitting `repo/sqlite/list.rs`, and the guard (security)

The 605-line `impl` must stay one block, so the split is between the
`impl` and what stands before it.

**A (recommended): a second file, `repo/sqlite/list_access.rs`, for the
predicate and the granted-row helpers.**

- Moves to `list_access.rs` (about 220 lines): `ACCESS_SELECT`,
  `Accessible`, `accessible_lists`, `ListGrant`, `authorize`, `begin_read`,
  `begin_write`, `ITEM_SELECT`, `fetch_items`, `fetch_item`, `touch_list`.
- Stays in `list.rs` (about 665 lines): `STORE_VIEW_SELECT`,
  `SHARE_SELECT` and the `impl ShoppingListRepo` block, unchanged.
- No SQL text, method body or signature changes. Moved items that
  `list.rs` calls become `pub(super)`. `ListGrant`'s fields stay private,
  and `list.rs` reads them through accessor methods.
- **This closes the gap named above.** `list.rs` is then a different
  module, so writing `ListGrant { … }` there is a compile error. Only
  `authorize` can make a grant, by the compiler and not by convention.

What changes in the guard, exactly:

1. One constant is added:
   `LIST_ACCESS_RS = include_str!("sqlite/list_access.rs")`.
2. Rules 1 and 2 are untouched. They still read the `impl` block in
   `LIST_RS` and the trait in `MOD_RS`, with the same three gate strings
   and the same count check.
3. Rule 3 runs over both files instead of one, with the same seven allowed
   headers. Before comparing, a leading `pub(super) ` is dropped from a
   header, because `fetch_items` and `touch_list` now carry it. The
   `&ListGrant` requirement on an allowed `async fn` is unchanged.
4. Rule 4: `other_sqlite_files` skips two names, `list.rs` and
   `list_access.rs`, instead of one. Every other file in `repo/sqlite/`,
   present or future, is still scanned, and the four forbidden names are
   the same.
5. Nothing is removed and no rule is loosened. `share.rs` grows by about
   ten lines, to about 792. If it would pass 800, the implementation stops
   and asks, because moving the guard would change its test path.

What the RR would need to say and roll back:

- RR-0009 constraint 3 ("list SQL lives in one file") and sub-answer 3
  become two named files.
- `persistence.md:45-47` names both files.
- The trigger row "Shared-list authorization" gains
  `crates/kauppa-core/src/repo/sqlite/list_access.rs` in `feature.md`,
  `scripts/agentic/check_triggers.py` and `.github/CODEOWNERS`
  (`test_check_triggers.sh` keeps the three in step). Without it, the file
  that holds the predicate would not be a trigger path.

- Pros: `list.rs` gets about 135 lines of room; the predicate sits alone
  in a small file that is easy to review; the grant becomes
  compiler-enforced.
- Cons: two files name the list tables; five roll-back edits.
- Risk: medium, because it touches the guard. The MB-0008 bypass
  experiments (a method without a gate, list SQL in another file) are
  re-run in the PR to show the guard still fails on each.
- Cost: small.

**A-minimal: move only the predicate** (`ACCESS_SELECT` to `authorize`,
`begin_read`, `begin_write`; about 135 lines). `list.rs` lands near 725.
The guard needs changes 1, 3 (without the `pub(super)` handling) and 4.
It leaves 75 lines of room, so the next feature brings the question back.

**B: a directory, `repo/sqlite/list/` (`mod.rs`, `access.rs`).** Same
content. `other_sqlite_files` reads only files directly in `repo/sqlite/`,
so the guard would have to start reading a whole directory, and any later
file dropped into it would be allowed to name list tables under rule 3's
headers. Wider than A for no gain.

**C: split the `impl` by area** (items, sharing, store view) into inherent
functions, leaving one-line trait methods. Rule 1 would then see no query
in any method and pass on nothing, so the guard would need rewriting to
follow the calls. This changes how the guard works. Not recommended.

**D: no split; trim the file below 800.** It grew 54 lines in four days.
Not durable.

**`templates/tests.rs` (821 lines, platform; inside `may_decide`).** The
same pattern MB-0051, MB-0053 and MB-0054 used: the navigation-panel code
(`Nav`, `panel`, `SECTIONS` and the bodies of `navigation_panel` and
`navigation_needs_no_script`, about 200 lines) moves to
`templates/tests/navigation.rs`, and one-line wrappers stay in `tests.rs`.
Every test path stays, so no brief check is rewritten. This needs no
decision. It is listed because the brief's context doesn't name the file
and MB-0055 is adding to it (see Q6).

### Q4: leftovers and the catalog keys (owner, to confirm)

**A (recommended): remove what nothing produces as a whole variant.**

- `InvitationState::as_str`.
- `PasswordLengthError`'s `Display` and `Error` impls, and the one unit
  test of the unused text. The rule's other tests stay. RR-0022 sub-answer
  4 ("field-less, never in the log or the plain body") still holds.
- `Entity::Favorite` and its four keys in each catalog.
- If Q2 is A: `Invalid::ChooseFavorite` and `errors.choose_favorite`.
- The other 11 unreachable entity keys stay. The types still allow those
  pairs, and `every_text` rightly asks for a key for every value the types
  allow (RR-0023 sub-answer 6: a reason without a key must not compile or
  pass).
- The Finnish catalog loses five lines and gains none, so there is no new
  Finnish to review.

**B: narrow the types** so only raised pairs exist (a separate entity enum
per reason). That removes all 15 keys, and it is a refactor of core's
error type beyond "the items listed" (non-goal 3). Not recommended here.

### Q5: plain error bodies (owner)

**A (recommended): keep English, and write the reason down.** No code
change in `error.rs` or `csrf/`. The RR rolls a short paragraph into
`templating.md` → Translations, after the "Messages built in Rust" bullet:

> Exception (RR-0023 sub-answer 10, RR-00NN): the plain error bodies from
> `WebError::error_response` and the cross-site 403 are English in every
> language. They are built without the request, so they can't know its
> language, and the forms catch rejected input and show it translated in
> the page. A translated error page is its own brief.

- Pros: honours the brief's non-goal (no user-visible behaviour change);
  touches no trigger path and none of MB-0055's files; the named check
  (`responses_unchanged`) then proves the property as written.
- Cons: a Finnish user who follows a dead link still reads `not found`.
- Risk: none. Cost: a documentation roll-back.

**B: translate through a request-aware error response.** What it would
need:

- `ResponseError::error_response(&self)` has no request, so the language
  can't be chosen there. The response must be rewritten where the request
  is known: an `ErrorHandlers` middleware for 400, 401, 403, 404, 409 and
  500, or a change to every handler's error path.
- The language choice (cookie, account, `Accept-Language`) lives in
  `i18n/request.rs`. It would have to be callable from the middleware.
  That file is a trigger path and MB-0055 is changing it.
- The middleware is wired in `main.rs` (a trigger path) and mirrored in
  the test app. RR-0023 sub-answer 10 said no middleware is added around
  the RR-0010 scope; this would reverse that. The cross-site 403 is made
  inside `csrf/` (a trigger path).
- New keys in both catalogs (MB-0055 is changing them), with Finnish for
  the owner to review.
- A decision on scope: the framework's bodies too (the empty 401 above all)
  or only the six from `error.rs`.
- `responses_unchanged`, `csrf::tests::cross_site` (asserts the body
  `forbidden`) and `admin::tests::access` (`PLAIN_404`) would all change.
- Risk: medium. Cost: a mission of its own, not a cleanup item.

**C: propose a separate brief for an error page** (recommended alongside
A, not instead of it). The real gap for users is not the wording. It is
that a dead link shows bare text with no navigation, and an ended sign-in
shows an empty page instead of the sign-in form. A brief for a translated
HTML error page, with the layout and a sign-in link on 401, would fix
both. It should start after MB-0055, since it needs the same files.

### Q6: order against MB-0055 (for information)

MB-0055 is changing `i18n/request.rs`, `templates/layout.hbs`,
`assets/app.css`, `locales/*.json` and `routes/mod.rs`, and adding tests
to `templates/tests.rs`. With the recommendations above:

| File | MB-0048 touches it for | Overlap |
| --- | --- | --- |
| `i18n/request.rs`, `layout.hbs`, `app.css` | nothing | None |
| `routes/mod.rs` | Q2 A: one line removed (`:58`, in the `/lists` scope) | Unavoidable; a different scope from `/theme`, so a clean merge is expected |
| `locales/en.json`, `fi.json` | Q4 A: five lines removed in `errors.*` | Unavoidable; MB-0055 adds keys elsewhere in the sorted files |
| `templates/tests.rs` | Q3: the navigation code moves out | Real: both change the file |

Proposed: MB-0048's implementation branch starts from `main` after MB-0055
merges. If the owner wants it sooner, the `templates/tests.rs` commit goes
last and is rebased onto MB-0055.

### Q7: brief amendments for the RR's roll-back (owner, for information)

- **Context**: the five bullets become the "Today" column of the table
  above (four test files; 855 and 821 lines; `Favorite` only; the framework
  bodies noted).
- **`context` list**: add `crates/kauppa-web/src/routes/lists/tests/`,
  `crates/kauppa-web/src/templates/tests.rs`, `crates/kauppa-web/src/csrf/mod.rs`,
  `locales/en.json`, `locales/fi.json` and
  `.agentic/mentorship/templating.md`.
- **P1 check**: a new test that reads `crates/kauppa-web/Cargo.toml` and
  fails if `sqlx` is under `[dependencies]`, proposed as
  `cargo test -p kauppa-web state::tests::sqlx_is_a_dev_dependency`. With
  the move made, the compiler proves the rest: `cargo check -p kauppa-web`
  builds the binary without dev-dependencies.
- **P2**: reworded to the decision (removed). Check unchanged.
- **P3**: names both files over 800, and `list_access.rs`. Check
  unchanged. A second check for the size itself:
  `scripts/agentic/check-file-sizes.sh` if the "cheap size check script"
  in `may_decide` is written, else the `wc -l` table in the
  Merge-Readiness Pack as the Validation plan says.
- **P4**: reworded to Q4 A's list. Check becomes a new test,
  `cargo test -p kauppa-web i18n::tests::catalog_has_no_unused_keys`:
  every catalog key is used by a template, by Rust's named keys or by
  `every_text`. If existing keys fail it for reasons outside this brief,
  the implementation stops and asks before deleting any.
- **P5**: reworded to the decision. Under A the check stays, plus
  `csrf::tests::cross_site` for the cross-site body.
- **`resolutions`**: the RR that closes this consult.
- **`autonomy.escalate`**: the four entries are settled by that RR.

## Recommendation and question

Recommended: Q1 A (move the line); Q2 A (remove the route); Q3 A
(`list_access.rs`, with the five guard changes listed and RR-0009
constraint 3 amended to two files); Q4 A (whole unused variants only);
Q5 A (keep English, documented in `templating.md`) with C (a separate
error-page brief) offered; Q6 start after MB-0055; Q7 the brief amendments.

Crisp questions:

1. **Platform**: move `sqlx` from `[dependencies]` to `[dev-dependencies]`
   in `crates/kauppa-web/Cargo.toml`, same version and features, under the
   dependency-bump runbook with the RR approved before the PR opens?
2. **Owner**: remove `POST /lists/{id}/favorites` with its handler, form
   and the test cases that post to it, so the old URL answers 404, and
   note the amendment to RR-0024 sub-answer 3?
3. **Security**: split `repo/sqlite/list.rs` as Q3 A, with the predicate
   and granted-row helpers in `repo/sqlite/list_access.rs`?
4. **Security**: accept the guard changes exactly as listed (one more file
   read by rule 3, one more name skipped by rule 4, `pub(super)` dropped
   from headers, rules 1 and 2 untouched), and amend RR-0009 constraint 3
   and sub-answer 3 from one file to those two?
5. **Security**: add `list_access.rs` to the "Shared-list authorization"
   trigger row in `feature.md`, `check_triggers.py` and `CODEOWNERS`, and
   to `persistence.md`?
6. **Owner**: remove `InvitationState::as_str`, `PasswordLengthError`'s
   `Display` and `Error` impls with their one unit test, `Entity::Favorite`
   and (if question 2 is yes) `Invalid::ChooseFavorite`, with their five
   keys in each catalog, and keep the other 11 unreachable entity keys
   because the types allow them?
7. **Owner**: keep the plain error bodies English, with the paragraph
   above added to `templating.md`?
8. **Owner**: do you want a separate brief for a translated HTML error
   page (dead links, and the sign-in form in place of the empty 401),
   to start after MB-0055?
9. **Owner**: start MB-0048's implementation after MB-0055 merges, and
   accept the brief amendments in Q7?

## Blast radius and rollback

What can break:

- **Q3** is the one with weight. A mistake in the move could leave a list
  query outside the predicate. Guards against it: the `impl` block isn't
  edited; the guard's rules 1 and 2 are untouched; the role matrix,
  revocation and non-grantee tests in `repo::share` run unchanged; the
  bypass experiments are re-run. Because no SQL changes, no data can be
  written differently.
- Q1: a build failure if non-test code used `sqlx`. The scratch build
  shows none does.
- Q2: a stale page or a private script posting to the old URL gets a 404.
- Q4: a missing catalog key fails `catalogs_complete` at test time, not in
  production.
- Q5 A: nothing.

Who is affected: nobody in normal use. No page, response or stored value
changes, except the 404 on the removed URL.

Rollback plan: revert the PR. One commit per item, so one item can be
reverted alone. No schema, no stored data, no `Cargo.lock` change. The
documentation roll-backs revert with the same PR.

## Minimal context

- MB-0048 P1 to P6, `autonomy.escalate` and `forbidden` (never weaken the
  bypass guard or the CSRF coverage test; never change a status code;
  never lower a lint).
- RR-0002 constraint 1 (one central predicate) and constraint 4
  (revocation is immediate); RR-0009 sub-answer 3 and constraints 3 and 4;
  RR-0010 (cross-site check); RR-0022 sub-answer 4; RR-0023 sub-answers 6
  and 10; RR-0024 sub-answer 3.
- `persistence.md:45-47` and `:60`, `actix-web.md` → Testing, `templating.md:30-31`,
  `dependency-bump.md`.
- Files the implementation would touch under the recommendations:
  `crates/kauppa-web/Cargo.toml`; `crates/kauppa-web/src/routes/lists/items.rs`,
  `routes/lists/mod.rs` (the re-export), `routes/mod.rs`, four files under
  `routes/lists/tests/`; `crates/kauppa-core/src/repo/sqlite/list.rs`,
  `list_access.rs` (new), `repo/share.rs`;
  `crates/kauppa-web/src/templates/tests.rs` and `tests/navigation.rs`
  (new); `crates/kauppa-core/src/domain/invitation.rs`,
  `domain/password_rule.rs`, `domain/mod.rs`, `error.rs`;
  `crates/kauppa-web/src/i18n/messages.rs`, `i18n/tests.rs`,
  `state.rs` (a new test module); `locales/en.json`, `locales/fi.json`.
- Human-owned roll-backs: `templating.md`, `persistence.md`, `feature.md`,
  `check_triggers.py`, `CODEOWNERS`, the MB-0048 brief, this consult's
  `state`.

## Evidence bundle (Hardened)

All on `main` at b800583, 2026-11-14.

**Entry gate.** `cargo check --workspace --all-targets`: finished, no
warnings.

**P1.**

- `grep -rn "sqlx" crates/kauppa-web/src`: 16 lines in the four files
  listed above. The crate's `build.rs` doesn't name it, and the crate
  has no `tests/` directory.
- Scratch copy (`git archive` of HEAD, own target directory), with the
  `sqlx` line moved to `[dev-dependencies]`:
  - `cargo check -p kauppa-web --locked --offline`: finished.
  - `cargo check --workspace --all-targets --locked --offline`: finished.
  - `cargo clippy --workspace --all-targets --all-features --locked
    --offline -- -D warnings`: finished.
  - `Cargo.lock` compared with HEAD's: identical.
  - `cargo deny --offline check`: `advisories ok, bans ok, licenses ok,
    sources ok`.

**P2.**

- `grep -rn "add_favorite" crates/`: the handler, its re-export
  (`routes/lists/mod.rs:69`), the registration (`routes/mod.rs:58`), a
  doc comment, the `list_posts` row, and an unrelated core test name.
- `grep -rnE "lists/[^ ]*favorites|\}/favorites" README.md deploy
  templates design-system scripts .github`: no match.
- `grep -rn "favorites" templates/lists/`: only the link to the Favorites
  page (`_list.hbs:26`) and a comment.

**P3.**

- `find crates -name '*.rs' | xargs wc -l | sort -rn`: the table above;
  32,577 lines in all.
- The guard read in full: `share.rs:619-782`.
- `grep -n "grant\.\(list\|access\)" repo/sqlite/list.rs`: the `impl`
  reads `grant.list` (lines 301, 497, 688) and `grant.access` (307), which
  is why the split needs accessor methods.
- `check_triggers.py` matches by prefix
  (`crates/kauppa-core/src/repo/sqlite/list.rs`), so `list_access.rs`
  doesn't match the existing row.

**P4.** Local deletion of `InvitationState::as_str`, the two
`PasswordLengthError` impls and `Entity::Favorite`, then reverted with
`git checkout -- crates` (`git status` clean after):

- `cargo check --workspace`: finished.
- `cargo check -p kauppa-web --all-targets`: finished.
- `cargo check --workspace --all-targets`: two errors, both in
  `password_rule.rs`'s `one_message_names_both_bounds` (`to_string()` on a
  type without `Display`).
- `cargo test -p kauppa-web i18n::`: 9 passed, 1 failed
  (`messages::tests::every_text_covers_every_reason`, the `3 * 7` count).
  `catalogs_complete` passed with the `favorite` keys still present.
- `grep -rnE "Entity::(Favorite|Ingredient|Recipe)" crates`:
  `Ingredient` at `repo/sqlite/list.rs:517` and `recipe.rs:332` (and a
  core test), `Recipe` at `routes/lists/items.rs:61`, `Favorite` nowhere.

**P5.**

- `error.rs:49-58` and `csrf/mod.rs:211` read in full.
- `grep -rnE "CoreError::(Invalid|Conflict)"` over the routes: the forms'
  handlers match on both and render `ui.invalid` or `ui.conflict`.
- `actix-identity` 0.8.0 `src/identity.rs:257-271`: the extractor answers
  with `HttpResponse::new(StatusCode::UNAUTHORIZED)`. `actix-web` 4.18.2
  `src/error/mod.rs`: the path and form error texts. Read, not run.
- Tests that pin the bodies: `error::tests::responses_unchanged`,
  `csrf/tests/cross_site.rs:95`, `routes/admin/tests/access.rs:22`.

**Checks.**

- `bash scripts/agentic/check_brief_tests.sh`: `checked 187 brief checks,
  0 selecting no tests`; MB-0048 listed as skipped (not started).
- Each of the brief's five `cargo test` checks with `-- --list`: one test
  each.

**Not done.** No file outside `.agentic/continuity/MB-0048.md` and this
consult was changed. The split, the guard edits and the route removal were
not tried in code. The line counts given for after the split are
estimates from the line ranges above.
