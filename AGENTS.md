# AGENTS.md — Root Mentorship Pack

Version: v1 · Owner: @juuso-kalliomaki · Last reviewed: 2026-11-15

This is the root Mentorship Pack for `kauppa-rs`, following the baseline in
[`Agentic SE Implementation Baseline`](docs/agentic-se-baseline.md). It was
written for a Java and TypeScript team; where it and this repository's
runbooks or packs differ, the repository wins.
It conditions judgment; it does **not** enforce. Enforcement lives in
`.agentic/runbooks/` and in CI (`.github/workflows/agentic-gate.yml`).

Harness-specific rule files (`CLAUDE.md`, `.cursorrules`, Copilot instructions,
etc.) must point at this file rather than diverging from it.

---

## Map

**What the system is.** `kauppa-rs` is a self-hosted web app for creating and
managing shopping lists and recipes on mobile and desktop. Multi-user, with
signed-in sessions kept in an encrypted, signed cookie (actix-session's
cookie store). Server-rendered HTML with progressive enhancement via htmx —
no SPA.

**Where changes belong.**

| Concern | Crate / Path | Notes |
| --- | --- | --- |
| Domain types, business rules | `crates/kauppa-core/src/domain/` | HTTP- and framework-agnostic. |
| Persistence | `crates/kauppa-core/src/repo/` | SQLite + sqlx. Repo traits abstract over the backend. |
| Auth primitives | `crates/kauppa-core/src/password.rs` | Argon2id. Never roll your own in handlers. |
| HTTP handlers | `crates/kauppa-web/src/routes/` | Thin: parse → call core → render. |
| Templates | `templates/*.hbs` | Handlebars partial-block layout. |
| Translations | `locales/<tag>.json`, `crates/kauppa-web/src/i18n/` | Every user-facing string; the `{{t}}` helper and the `Ui` extractor (RR-0023). |
| Deployment | `deploy/`, `scripts/deploy/` | Image, Compose stack, Terraform; `deploy/README.md` is the operator guide (RR-0014). |
| Client interactivity | `assets/*` + `hx-*` attrs | htmx first (vendored at `assets/vendor/`, RR-0015); add JS only when htmx can't, under `assets/app/` (RR-0053). |
| Schema | `migrations/*.sql` | Append-only. New file per change. |
| Agentic artifacts | `.agentic/` | Human-owned governance. |

**Tech stack.** Rust 2024 edition · Tokio · actix-web 4 · actix-session + actix-identity ·
sqlx (SQLite, runtime-tokio-rustls) · handlebars 6 · htmx 2 · tracing + tracing-actix-web ·
argon2 (password hashing) · thiserror / anyhow.

**Agent role.** Execute delegated changes inside the gates in
`.agentic/runbooks/`. Never author or modify Mentorship Packs, Runbooks, or
Resolution Records without an explicit human approval.

---

## Operating playbook

### Setup

```bash
rustup toolchain install stable
cp .env.example .env
mkdir -p data
```

### Build, run, test

```bash
cargo check --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets

# Local dev server (will create data/kauppa.db on first run).
cargo run -p kauppa-web
```

The fast gate (`scripts/agentic/fast-gate.sh`) runs fmt + clippy + unit tests in
~tens of seconds. CI runs the same set plus `cargo deny check`.

### CI and release flow

- Pull requests trigger `.github/workflows/ci.yml` (fast gate) and
  `.github/workflows/agentic-gate.yml` (artifact/evidence checks, see
  `.agentic/runbooks/feature.md`).
- Any PR labeled `agent-authored` must carry a Merge-Readiness Pack (the PR
  description, see `.github/pull_request_template.md`) linking to a Mission
  Brief in `.agentic/missions/`.
- Changes under `migrations/`, auth paths, dependency manifests, or
  `.agentic/{mentorship,runbooks,resolutions}` require CODEOWNERS approval and
  a linked Resolution Record (see `.agentic/runbooks/*` escalation sections).

### Continuity Packs

Every Mission Brief has a Continuity Pack at `.agentic/continuity/MB-<id>.md`,
copied from `.agentic/continuity/TEMPLATE.md`. It is the handoff record: the
next agent or human resumes a mission from it, not from chat history.

- **Create it with the brief.** The commit that adds a Mission Brief also adds
  its pack, recording the state as "not started". If you pick up a brief that
  has no pack, create it before editing anything else; the runbook entry gates
  check for it.
- **Keep it current.** Update it, and bump `updated_at`, in the same commit or
  PR as the work it describes, and at least:
  - at each runbook stage checkpoint;
  - when a consult is opened or answered, or a Resolution Record is approved;
  - when a PR is opened or merged;
  - before a session ends, at context compaction, and at any handoff to
    another agent or a human.
- **Close it out, don't delete it.** The mission's final PR records the end
  state: what was delivered, PR links, follow-ups. The file stays as history.

Agents own Continuity Packs (`.agentic/README.md`), so updating one needs no
consult.

### Conventions

- **Errors.** Core returns `CoreResult<T>` (`CoreError`). Web returns
  `WebResult<T>` (`WebError`). Never `.unwrap()`, `.expect(...)` or `panic!()`
  in library or handler code outside tests. If it looks like you need to, raise
  a consult (Table 4) with the invariant you can't prove locally.
- **Logging.** Use `tracing` macros with structured fields
  (`tracing::info!(user_id = %id, "...")`), never `println!` or `eprintln!`.
- **DB access.** Only via the repo traits in `kauppa_core::repo` (`UserRepo`,
  `InvitationRepo`, `AdminRepo`, `CategoryRepo`, `ProductRepo`,
  `ShoppingListRepo`, `RecipeRepo`). Hand-rolled `sqlx` calls in handlers are a red flag.
- **Templates.** Each page is `{{#> layout page_title="..."}}...{{/layout}}`.
  Keep layout markup in one place. Layout parameters are prefixed `page_`
  (RR-0006).
- **htmx.** Prefer `hx-*` attributes for interactivity. Server returns HTML
  fragments on `HX-Request: true` and full pages otherwise.

---

## Engineering intent

### Goals

1. Fast, keyboard- and mobile-friendly list and recipe management.
2. Server-rendered, minimal JS, works on a slow phone over 3G.
3. Easy to self-host: one binary, one SQLite file, one `.env`.
4. Readable code: the next person (human or agent) can navigate the whole
   system in under an hour.

### Non-functional priorities (ordered)

1. **Correctness.** Lost data is unacceptable. Prefer explicit types and
   compile-time checks.
2. **Security.** Treat every unauthenticated request as hostile. See
   `.agentic/mentorship/security.md`.
3. **Observability.** Every request is traced; failures carry context.
4. **Performance.** Keep p95 under 100 ms for pages that read a single user's
   data (local SQLite). Set budgets per Mission Brief when relevant.
5. **Developer ergonomics.** Fast feedback loop; small, understandable units.

### Do-not-touch boundaries

- `migrations/*.sql` is append-only. Never edit a committed migration — add a
  new one. Schema changes require an approved Resolution Record.
- `kauppa-core/src/password.rs` encodes the project's password KDF. Changing
  the algorithm or parameters is a security-tier change.
- `Cargo.toml` dependency additions, removals, or version bumps route through
  the dependency-bump runbook.
- `assets/` is served directly; nothing executable may be committed there
  except under `assets/vendor/` and `assets/app/`. Under `assets/vendor/`:
  pinned third-party JS from a reviewed
  source, byte-for-byte, with its checksum checked in the test suite (htmx
  2.0.11, RR-0017, its response-targets extension 2.0.4, RR-0018, and its
  SSE extension 2.2.4, RR-0049, all from the npm registry). Adding, upgrading or modifying a vendored file is
  a platform decision. First-party scripts may live under `assets/app/`
  only, one file per approving RR (`assets/app/sortable.js`, RR-0053); a
  test lists them. They carry no `eval`, no `innerHTML` and no
  event-handler attributes, and load through `<script defer>` with SRI
  from the layout (RR-0054). SVG images under `assets/flags/` are not executable:
  they are shown through `<img>`, pinned by checksum, and a test keeps them
  free of script, links and styles (RR-0031). The app icons under
  `assets/icons/` (PNG, ICO and one SVG) and the font files under
  `assets/fonts/` (WOFF2) are not executable: they come from
  `design-system/`, are pinned by checksum in the test suite, and the icon
  SVG's style block may only colour its shapes (RR-0034).

---

## Governance and safety (Hardened tier)

See `.agentic/mentorship/security.md` for authn/authz rules,
`.agentic/mentorship/persistence.md` for schema and query rules, and
`.agentic/runbooks/*.md` for the enforced workflow.

Decision rights:

| Area | Owner |
| --- | --- |
| Product scope, Mission Brief acceptance | @juuso-kalliomaki |
| Mentorship Pack / Runbook changes | @juuso-kalliomaki |
| Schema / `migrations/` | @juuso-kalliomaki (data) |
| Auth, sessions, cookies | @juuso-kalliomaki (security) |
| Dependencies, build settings | @juuso-kalliomaki (platform) |

---

## Lifecycle and references

- **Status.** v1, active.
- **Change procedure.** Any agent-proposed change to this file goes through a
  Consultation Request Pack (`.agentic/consults/`) and closes in a Resolution
  Record (`.agentic/resolutions/`). Humans merge.
- **Precedence.** This file governs the whole repo. Task-class packs under
  `.agentic/mentorship/` refine it; on conflict, this file wins and the
  task-class pack is updated via a Resolution Record.
- **Retirement.** Deprecate sections by replacement, not deletion. Keep the
  version history in git.
