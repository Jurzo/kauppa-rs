# kauppa-rs

A self-hosted web app for shopping lists and recipes. Mobile- and
desktop-friendly, built with **Rust + Tokio + actix-web + handlebars + htmx**.

## Status

Scaffold. The runnable baseline (health check, login, register, empty list and
recipe pages) is in place; feature work is tracked as Mission Briefs under
`.agentic/missions/`.

## Running locally

```bash
cp .env.example .env
mkdir -p data
cargo run -p kauppa-web
# open http://127.0.0.1:8080
```

The server creates `data/kauppa.db` on first run and applies the migrations in
`migrations/`.

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
├── assets/                  # static files served at /assets
├── scripts/agentic/         # fast-gate, scaffolds, CI validators
└── .agentic/                # Mission Briefs, Runbooks, Mentorship Packs, …
```

## Agentic workflow

Development follows the baseline in `../Agentic SE Implementation Baseline.md`.

- **Starting work:** `scripts/agentic/scaffold-mission.sh <slug>` creates a
  Mission Brief. Fill it in. The feature runbook
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
