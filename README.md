# kauppa-rs

A self-hosted web app for shopping lists and recipes. Mobile- and
desktop-friendly, built with **Rust + Tokio + actix-web + handlebars + htmx**.

The app calls itself **kauppalista.app**; `kauppa-rs` is the name of this
repository and its crates.

## Status

In use. Signed-in users keep shopping lists (with a shopping mode that ticks
items off by category), share lists with other users, keep recipes and add
their ingredients to a list, and keep favorites, products and categories.
Accounts are invite-only. Feature work is tracked as Mission Briefs under
`.agentic/missions/`.

**Live updates:** an open list page and its shopping view show other
people's changes as they happen, over one event stream per open page (at most
eight per user). Browsers allow about six HTTP/1.1 connections per origin, so
with many list tabs open serve the app over HTTP/2, as the Compose stack's
Caddy does. Without JavaScript the pages work, without live updates.

**Languages:** English and Finnish. The language follows the browser
(`Accept-Language`) until the user picks one from the language menu; every
user-facing string lives in `locales/<tag>.json`.

## Running locally

Needs Rust (the version in `rust-toolchain.toml`; `rustup` installs it on
the first build).

```bash
cp .env.example .env
mkdir -p data
cargo run -p kauppa-web
# open http://127.0.0.1:8080
```

The server creates `data/kauppa.db` on first run and applies the migrations in
`migrations/`. The `.env.example` values are for a loopback address only.

### The first account

Registration is invite-only, and a fresh database has no accounts. Create the
first one from a shell, with the server running or not:

```bash
cargo run -p kauppa-web -- admin invite --note "me"
# prints: /auth/register?invite=<token>
```

Open `http://127.0.0.1:8080` followed by the printed path, register, and you
are signed in. To make that account an administrator (it can then invite
others at `/admin/invitations`):

```bash
cargo run -p kauppa-web -- admin grant you@example.com
```

## Deployment

`deploy/README.md` is the operator guide: a container image, a Compose
stack with Caddy for HTTPS, backups to object storage and Terraform for a
DigitalOcean droplet. The same `admin invite` and `admin grant` commands
bootstrap the first account there.

## Testing

```bash
scripts/agentic/fast-gate.sh   # fmt + clippy + test
cargo test --workspace --all-targets
```

CI (`.github/workflows/ci.yml`) runs the same gates plus `cargo deny check`.
A second workflow (`agentic-gate.yml`) validates artifact schemas and PR
evidence; see `.agentic/`.

## Project layout

```
kauppa-rs/
├── AGENTS.md                # root Mentorship Pack (humans own; agents follow)
├── crates/
│   ├── kauppa-core/         # domain, repo traits, persistence (SQLite + sqlx)
│   └── kauppa-web/          # actix-web binary, handlebars, htmx
├── migrations/              # append-only SQL, applied on startup
├── templates/               # handlebars page templates
├── locales/                 # translations, one JSON catalog per language
├── assets/                  # static files served at /assets (htmx vendored)
├── deploy/                  # image, Compose stack, Terraform, operator guide
├── docs/                    # the Agentic SE baseline this repo follows
├── scripts/agentic/         # fast-gate, scaffolds, CI validators
├── scripts/deploy/          # deployment checks and drills
└── .agentic/                # Mission Briefs, Runbooks, Mentorship Packs, …
```

## Agentic workflow

Development follows the baseline in
[`docs/agentic-se-baseline.md`](docs/agentic-se-baseline.md); where it and
this repository's runbooks or packs differ, the repository wins.

- **Starting work:** `scripts/agentic/scaffold-mission.sh <slug>` creates a
  Mission Brief and its Continuity Pack. Fill them in. The feature runbook
  (`.agentic/runbooks/feature.md`) governs execution.
- **Hitting a decision boundary:**
  `scripts/agentic/scaffold-consult.sh <slug>` opens a Consultation Request
  Pack. Do not proceed without an approved Resolution Record for the
  escalation-trigger paths listed in the runbook.
- **Opening a PR:** fill the Merge-Readiness Pack in the PR body
  (`.github/pull_request_template.md`). Scope-to-proof map must cover every
  acceptance ID in the Mission Brief.

## License

MIT. See [LICENSE](LICENSE).
