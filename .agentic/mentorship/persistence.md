# Mentorship Pack — persistence (sqlx + SQLite)

Version: v1 · Scope: `crates/kauppa-core/src/repo/**` and `migrations/**`.

## Schema

- Every schema change is a **new numbered file** in `migrations/` (`NNNN_<slug>.sql`).
  Never edit a committed migration — add a new one. Even for a dev-only app,
  this preserves reproducibility and matches the Hardened tier escalation path.
- All tables use `STRICT`. UUIDs as `TEXT`. Timestamps as `TEXT` (RFC3339;
  SQLite `STRICT` does not accept `TIMESTAMP` — sqlx stores
  `DateTime<Utc>` as RFC3339 text).
- Foreign keys use `ON DELETE CASCADE` by default; justify any divergence in
  the migration file's comment header.

## Queries

- Repository traits (`UserRepo`, `ShoppingListRepo`, `RecipeRepo`) are the only
  surface the web layer sees. Add methods there, not in handlers.
- The scaffold uses runtime `sqlx::query(...)` so builds don't require a live
  DB. **Target state:** once the schema is stable, migrate to `sqlx::query!`
  + `sqlx::query_as!` + `cargo sqlx prepare` offline cache under `.sqlx/`,
  then add `SQLX_OFFLINE=true` to CI. Track in a Mission Brief.
- Prefer named columns in SELECT, never `*`.

## Pool

- `SqlitePool` wraps `sqlx::SqlitePool`. The web layer never imports `sqlx`.
- `connect` sets `create_if_missing(true)` and `foreign_keys(true)`.
- Max 8 connections for SQLite. Don't raise this without benchmarking write
  contention — SQLite serializes writers.

## Transactions

- Multi-statement writes use `sqlx::Transaction`. Acquire → operate → commit;
  rollback on error. Repository methods that mutate multiple rows must own the
  transaction, not accept one from a caller.

## Red flags

- Changing a committed migration file → stop, open a new migration.
- Reading or writing `sqlite_master` directly → use a migration.
- Dynamic SQL built via `format!` with user input → forbidden; use binds.
