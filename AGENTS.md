# AGENTS.md — Root Mentorship Pack

Version: v1 · Owner: @juuso-kalliomaki · Last reviewed: 2026-10-28

This is the root Mentorship Pack for `kauppa-rs`, following the baseline in
[`Agentic SE Implementation Baseline`](../Agentic%20SE%20Implementation%20Baseline.md).
It conditions judgment; it does **not** enforce. Enforcement lives in
`.agentic/runbooks/` and in CI (`.github/workflows/agentic-gate.yml`).

Harness-specific rule files (`CLAUDE.md`, `.cursorrules`, Copilot instructions,
etc.) must point at this file rather than diverging from it.

---

## Map

**What the system is.** `kauppa-rs` is a self-hosted web app for creating and
managing shopping lists and recipes on mobile and desktop. Multi-user with
server-side sessions. Server-rendered HTML with progressive enhancement via
htmx — no SPA.

**Where changes belong.**

| Concern | Crate / Path | Notes |
| --- | --- | --- |
| Domain types, business rules | `crates/kauppa-core/src/domain/` | HTTP- and framework-agnostic. |
| Persistence | `crates/kauppa-core/src/repo/` | SQLite + sqlx. Repo traits abstract over the backend. |
| Auth primitives | `crates/kauppa-core/src/password.rs` | Argon2id. Never roll your own in handlers. |
| HTTP handlers | `crates/kauppa-web/src/routes/` | Thin: parse → call core → render. |
| Templates | `templates/*.hbs` | Handlebars partial-block layout. |
| Client interactivity | `assets/*` + `hx-*` attrs | htmx first; add JS only when htmx can't. |
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

### Conventions

- **Errors.** Core returns `CoreResult<T>` (`CoreError`). Web returns
  `WebResult<T>` (`WebError`). Never `.unwrap()`, `.expect(...)` or `panic!()`
  in library or handler code outside tests. If it looks like you need to, raise
  a consult (Table 4) with the invariant you can't prove locally.
- **Logging.** Use `tracing` macros with structured fields
  (`tracing::info!(user_id = %id, "...")`), never `println!` or `eprintln!`.
- **DB access.** Only via repo traits (`UserRepo`, `ShoppingListRepo`,
  `RecipeRepo`). Hand-rolled `sqlx` calls in handlers are a red flag.
- **Templates.** Each page is `{{#> layout title="..."}}...{{/layout}}`. Keep
  layout markup in one place.
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
  beyond vendored, pinned JS from a reviewed source (right now htmx is loaded
  from CDN; vendoring it is a platform decision).

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
