# Consultation Request Pack — Documentation that matches the system, a scaffold that creates the pack, and which branches, worktrees and stashes go

```yaml
---
id: CRP-0042
mission: MB-0050
owner: "@juuso-kalliomaki"
target_role: platform
risk_tier: low
urgency: low
state: open
runbook_trigger: "MB-0050 autonomy.escalate (every AGENTS.md and Mentorship Pack edit as one CRP with the exact text → owner; where the baseline document lives → owner; which branches, worktrees and stashes may be deleted → owner; Cargo.toml comment edits → platform-owner; .gitignore and scripts/agentic/ → platform-owner); feature.md escalation triggers `AGENTS.md`, `.agentic/{mentorship,resolutions}/**`, `Cargo.toml`, `scripts/agentic/**`"
---
```

## Decision statement

Approve the exact corrections below to `AGENTS.md`, `templating.md`,
`actix-web.md` and `Cargo.toml`'s comments (descriptions only, no rule
changes); decide where the Agentic SE baseline document lives; approve the
`scaffold-mission.sh` change (P6) and `.claude/` in `.gitignore`; and
confirm the deletion list (P7).

P5 is done (#96). P1 (`README.md`) and P4 (`deploy/README.md`) need no
consult: wording inside what each document already decides.

## Terms

- **Correction**: an edit that makes a description match the code as it is.
  A rule's meaning doesn't change (brief: forbidden).
- **Merged branch**: its tip is reachable from `origin/main`, or its PR is
  merged and `git cherry` shows every commit's change already in `main`.

## Current state (checked 2026-11-14 on `main` fe05e79)

| Where | Says | Is |
| --- | --- | --- |
| `AGENTS.md:19` | "Multi-user with server-side sessions." | `CookieSessionStore` (`main.rs:117`): the session is a signed, encrypted cookie |
| `AGENTS.md:113` | repo traits `UserRepo`, `ShoppingListRepo`, `RecipeRepo` | seven: also `InvitationRepo`, `AdminRepo`, `CategoryRepo`, `ProductRepo` (`repo/mod.rs`) |
| `AGENTS.md` Map | no row for `locales/`, `deploy/`, `crates/kauppa-web/src/i18n/` | all three exist (MB-0022, MB-0035) |
| `AGENTS.md:3` | "Last reviewed: 2026-10-28" | — |
| `AGENTS.md:6` | links `../Agentic%20SE%20Implementation%20Baseline.md` | outside the repository; a clone doesn't have it. The file is the owner's own document (25 KB, "Oct 28, 2026 · @Juuso"), written for a Java/TypeScript team |
| `templating.md:12-13` | "write a small vanilla-JS module in `assets/js/` and link it" | AGENTS.md → Do-not-touch: nothing executable in `assets/` except `assets/vendor/`; there is no `assets/js/` |
| `actix-web.md:7` | handler signature `async fn name(state: web::Data<AppState>, …)` | handlers take `state`, then `Ui` (language, theme, `ui.render`), then `Identity` (e.g. `categories::index`) |
| `actix-web.md:14` | "HTML pages return `HttpResponse::Ok().content_type("text/html").body(...)`" | pages render with `ui.render(state, template, &view)` and carry `Vary: Cookie, Accept-Language` (`i18n::VARY`) |
| `actix-web.md:26-27` | "Repo trait objects are owned by `AppState` and cloned per-worker." | `AppState` holds `pool: SqlitePool`; the repo traits are implemented on `SqlitePool` itself; no trait objects |
| `actix-web.md:40` | "Reserve full E2E (Playwright) for user-visible flows." | no such test; MB-0047 (CRP-0041) decides it. Left to MB-0047's roll-back |
| `Cargo.toml:34-37` | `compress-zstd` off for RUSTSEC-2026-0391/0398/0403 | all three now have fixes: zstd ≥ 0.14.3, zstd-safe ≥ 8.2.0 (0.14.4 and 8.3.0 released); actix-http 3.21.0 takes `zstd ^0.14`. The comment's reason no longer holds |
| `Cargo.toml:35-36` | `http2` off: actix-http pins h2 0.3, RUSTSEC-2026-0258 | still true: actix-http 3.21.0 needs `h2 ^0.3.27`; the fix is h2 ≥ 0.4.16 only |
| `scaffold-mission.sh` | copies the brief template only | AGENTS.md → Continuity Packs: the commit that adds a brief also adds its pack |
| `.gitignore` | no `.claude/` | `.claude/` (the agent's local settings and worktrees) is untracked in every `git status` |

## Proposed text

### Q1 — `AGENTS.md` (owner)

1. Line 3: `Last reviewed: 2026-10-28` → the date the roll-back lands.
2. Line 19: "Multi-user with server-side sessions." → "Multi-user, with
   signed-in sessions kept in an encrypted, signed cookie
   (actix-session's cookie store)."
3. Map table, two rows added after "Templates" (they cover `locales/`,
   `i18n/`, `deploy/` and `scripts/deploy/`):

   ```markdown
   | Translations | `locales/<tag>.json`, `crates/kauppa-web/src/i18n/` | Every user-facing string; the `{{t}}` helper and the `Ui` extractor (RR-0023). |
   | Deployment | `deploy/`, `scripts/deploy/` | Image, Compose stack, Terraform; `deploy/README.md` is the operator guide (RR-0014). |
   ```
4. Line 113: "Only via repo traits (`UserRepo`, `ShoppingListRepo`,
   `RecipeRepo`)." → "Only via the repo traits in `kauppa_core::repo`
   (`UserRepo`, `InvitationRepo`, `AdminRepo`, `CategoryRepo`,
   `ProductRepo`, `ShoppingListRepo`, `RecipeRepo`)."
5. Line 6: depends on Q2.

### Q2 — The baseline document (owner)

- **A. Copy it into the repository** as `docs/agentic-se-baseline.md` and
  link that. Anyone with the repository can read it; it is the owner's own
  text, so copying is the owner's call. It then needs a note that it was
  written for a Java/TypeScript team and that this repository's runbooks
  win on conflict.
- **B. Link a public copy** (a URL the owner provides).
- **C. Drop the link** and say in one sentence what the baseline sets
  (artifacts, gates, MVP and Hardened tiers), citing the book.

Recommended: C, unless the owner wants the document published; A or B
then.

### Q3 — Mentorship Packs (owner)

`templating.md` lines 12-13 → "No inline JavaScript in templates.
Interactivity is htmx (`hx-*` attributes). Anything htmx can't do needs a
platform decision first: `assets/` holds nothing executable outside
`assets/vendor/` (AGENTS.md → Do-not-touch boundaries)."

`actix-web.md`:
- line 7 → "Signature: `async fn name(state: web::Data<AppState>, ui: Ui,
  …) -> WebResult<HttpResponse>`; `Ui` carries the request's language and
  theme and renders templates."
- line 8 (extractor order) → "`HttpRequest` → shared state → `Ui` →
  `Identity` (or `Option<Identity>`) → `web::Path`/`web::Query`/`web::Form`."
- line 14 → "HTML pages render with `ui.render(&state, template, &view)`
  and answer `text/html` with `Vary: Cookie, Accept-Language`
  (`i18n::VARY`), since the language comes from the cookie or the header."
- lines 26-27 → "`AppState` holds the `SqlitePool`, the templates, the
  origin policy, the auth limiter and settings. The repo traits are
  implemented on `SqlitePool`; handlers call them through `&state.pool`."
- line 40 (Playwright): unchanged here; MB-0047's RR replaces it.

### Q4 — `Cargo.toml` comment (platform)

Lines 34-37 →

```toml
# Default features minus `compress-zstd` (it was off for
# RUSTSEC-2026-0391/0398/0403; fixed in zstd 0.14.3 / zstd-safe 8.2.0, so
# enabling it is now a dependency-bump decision, not a security one) and
# `http2` (actix-http 3.x needs h2 0.3; RUSTSEC-2026-0258 is fixed only in
# h2 0.4.16). Deploy behind a reverse proxy that terminates HTTP/2.
# `ws` dropped — not using websockets.
```

Features unchanged. Whether to turn `compress-zstd` on is not in this
mission (non-goal: no rule changes); it would be a dependency-bump.

### Q5 — `scaffold-mission.sh` and `.gitignore` (platform)

- After creating the brief, the script copies
  `.agentic/continuity/TEMPLATE.md` to `.agentic/continuity/MB-<id>.md`,
  drops the template's instruction paragraph, sets `mission`, `updated_at`
  (now, UTC) and `owner` from the brief, and writes the state lines:
  "- Done: Brief drafted (<date>)." and "- In flight: implementation not
  started; the brief awaits the owner's acceptance." (the line
  `check_brief_tests.sh` skips on). It refuses to overwrite an existing
  pack.
- P6's check names `test_check_triggers.sh`; it fits better as a new
  `scripts/agentic/test_scaffold_mission.sh` (brief: `may_decide`), run
  in the `agentic scripts` CI job next to the other self-tests. It
  scaffolds into a temporary copy of `.agentic/` and asserts both files,
  the ids, the "not started" line and that `validate_artifacts.py` passes.
- `.gitignore` gains `/.claude/`.

### Q6 — Deletion list (owner)

Counted 2026-11-14 after `git fetch --prune`. Kept in every case: `main`,
the branches of open PRs #97 and #98, `agentic/mb-0047-consult` and
`agentic/mb-0050-consult`.

1. **Remote branches: 30.** 29 are merged into `origin/main`; the 30th,
   `origin/agentic/mb-0031-csp`, is one commit past #59's merge whose change
   is in `main` (`git cherry`: none unpicked). Delete all 30
   (`git push origin --delete …`). Before: 32 remote branches; after: 2
   (#97, #98) plus any new ones.
2. **Local branches: about 128.** 115 merged into `origin/main`, plus 13
   whose PR merged and whose every commit is in `main` (`briefs-audit-2026-11`,
   `briefs-mb-0052-0053`, `crp-0005-kauppa-core-standalone-build`,
   `follow-ups-after-mb-0003`, `mb-0014-recipe-created-products`,
   `mb-0017-closeout`, `mb-0022-ci`, `mb-0031-csp`, `mb-0035-conversion`,
   `mb-0045-ci-maintenance`, `mb-0049-consult`,
   `feat/mb-0004-assemble-list`, `closeout-stale-packs-2026-11-14`).
   `agentic/mb-0041-sqlite-fix` (CRP-0021, RR-0021) goes once #98 merges,
   which carries both commits.
3. **Worktrees: 50 besides the main one.** 48 have no uncommitted change
   (`.claude/` ignored); 2 point at folders that no longer exist
   (`git worktree prune`). Remove all, with `git worktree remove` (which
   refuses a dirty one).
4. **Stashes: 3.**
   - `stash@{0}` "owner's P4 ticks on MB-0029 pack (already merged in
     #60)": drop.
   - `stash@{1}` (2026-11-10, on `main` at #59): the owner's checklist
     ticks and notes for MB-0029 and MB-0031. `main`'s packs already have
     the ticks (9 and 8 `[x]`); the notes under MB-0029's first four lines
     ("no error message is shown") describe the bug #60 fixed. Drop, or
     keep the notes in MB-0029's pack first: owner's choice.
   - `stash@{2}` (2026-11-08, on `mb-0026-rate-limit`): MB-0026's pack
     "RR-0013 drafted, uncommitted"; RR-0013 was signed and MB-0026 closed
     since. This is the stash that predates this work and isn't the
     agent's. Drop only on the owner's word.

## Recommendation and question

Recommended: Q1 as written; Q2 C; Q3 as written; Q4 as written; Q5 as
written, with the new test script; Q6 all of it, `stash@{1}` and
`stash@{2}` dropped.

Crisp questions:

1. `AGENTS.md`: apply items 1–4 as written?
2. The baseline: copy in (A), public link (B), or drop the link with a
   one-sentence summary (C)?
3. `templating.md` and `actix-web.md`: apply as written?
4. `Cargo.toml`: the comment as written, features unchanged?
5. Scaffold and `.gitignore`: as written, with
   `test_scaffold_mission.sh` as P6's check?
6. Deletion: all four groups as listed? For `stash@{1}`, drop or keep its
   notes in MB-0029's pack first? For `stash@{2}`, drop?

## Blast radius and rollback

What can break:
- Docs: nothing at run time. A wrong description misleads the next reader;
  each line above names the code it was checked against.
- The scaffold: only new briefs; the self-test runs in CI.
- Deletion: a branch or stash with work not in `main`. Every listed branch
  is checked with `git cherry`; worktrees are removed with `git worktree
  remove`, which refuses uncommitted changes. Deleted remote branches stay
  restorable from their merged PRs on GitHub; local branch tips stay in the
  reflog for 90 days.

Rollback plan: revert the PR. For a deleted branch, recreate it from its
tip (listed in the pack before deletion).

## Minimal context

- `.agentic/missions/MB-0050-docs-and-housekeeping.md`
- `AGENTS.md`, `.agentic/mentorship/templating.md`,
  `.agentic/mentorship/actix-web.md`
- `crates/kauppa-web/src/main.rs`, `state.rs`, `i18n/request.rs`
- `crates/kauppa-core/src/repo/mod.rs`
- `Cargo.toml`, `scripts/agentic/scaffold-mission.sh`, `.gitignore`

## Evidence bundle (Hardened)

```text
crates/kauppa-web/src/main.rs:117  SessionMiddleware::builder(CookieSessionStore::default(), …)
crates/kauppa-core/src/repo/mod.rs  pub trait UserRepo (41), InvitationRepo (71), AdminRepo (114),
                                    CategoryRepo (130), ProductRepo (156), ShoppingListRepo (240), RecipeRepo (418)
crates/kauppa-core/src/repo/sqlite/user.rs:16  impl UserRepo for SqlitePool
crates/kauppa-web/src/state.rs:10  AppState { pool, templates, origin, limiter, cookie_secure }
crates/kauppa-web/src/i18n/request.rs:34  VARY = (VARY, "Cookie, Accept-Language")
advisory-db (fetched 2026-11-13):
  RUSTSEC-2026-0391 zstd       patched ^0.13.4, >= 0.14.3
  RUSTSEC-2026-0398 zstd-safe  patched ^7.4.0, >= 8.2.0
  RUSTSEC-2026-0403 zstd-safe  patched ^7.4.0, >= 8.2.0
  RUSTSEC-2026-0258 h2         patched >= 0.4.16
crates.io: actix-http 3.21.0 → h2 ^0.3.27 (optional), zstd ^0.14 (optional); zstd 0.14.4, zstd-safe 8.3.0
git branch -r (after fetch --prune): 32; --merged origin/main: 29
git branch: 131; --merged origin/main: 115
git worktree list: 51 (48 clean, 2 missing folders, 1 main)
git stash list: 3
```
