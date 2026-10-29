# Workflow Runbook — Schema Migration

Version: v1 · Owner: @juuso-kalliomaki (data).

Schema changes are a trigger path. All migrations run through this runbook.

## Entry gate

1. A Mission Brief exists and names the data change explicitly.
2. A Consultation Request Pack (`CRP-*`) is open **before** any file in
   `migrations/` is created, with options and trade-offs for the schema shape.
3. A Resolution Record (`RR-*`) is approved before the PR is opened. The RR
   references the exact migration filename to be created. When one RR covers
   several Mission Briefs, it lists each brief's migration filename; creating
   a migration that isn't listed needs an RR amendment through a new consult
   first (RR-0001).
4. The brief's Continuity Pack exists and is kept current
   (`AGENTS.md` → Continuity Packs; feature runbook entry gate, item 4).

## Stages

### S1 — Design

In the consult: candidate schemas (two or three), backfill strategy, backward
compatibility window, rollback plan.

### S2 — Author

- Create a new file `migrations/NNNN_<slug>.sql`. Never edit a committed one.
- `STRICT` on all new tables.
- UUIDs as `TEXT`, timestamps as `TEXT` (RFC3339), booleans as `INTEGER NOT NULL`.
- Include a comment header: migration ID, RR link, intent.
- A foreign key that diverges from `ON DELETE CASCADE` names the RR that
  allows it in that header (e.g. `0003_products.sql` → RR-0001).

### S3 — Verify

- `cargo run -p kauppa-web` against a fresh `data/kauppa-check.db` runs the
  migration cleanly.
- Add a test that exercises the new shape at the repo-trait level.

### S4 — Package

The Merge-Readiness Pack must link the RR and describe the rollback path
(usually: write a reverse migration and release a hotfix; SQLite does not
support `DROP COLUMN` on all versions, so prefer additive changes).

## Forbidden

- Editing a committed migration file.
- Column renames without a two-phase rollout (add new, backfill, switch reads,
  switch writes, drop old — each a separate migration + release).
- Destructive statements (`DROP TABLE`, `DELETE FROM`, `UPDATE ... SET`) in a
  migration without an approved RR explicitly authorizing them.
