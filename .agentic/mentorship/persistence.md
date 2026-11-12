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
- Exception (RR-0001): references to catalog and taxonomy rows (`category_id`,
  `product_id`) use `ON DELETE SET NULL` and stay nullable, so deleting a
  category or product never deletes the rows that point at it. Whether a
  category is required is a handler rule, not a `NOT NULL` column.
- Exception (RR-0012): audit references to the acting user
  (`admins.granted_by`, `invitations.created_by`, `invitations.used_by`) use
  `ON DELETE SET NULL` and stay nullable, so deleting a user keeps the
  record. Timestamps that SQL compares (`expires_at > ?`) are truncated to
  whole seconds before binding.

## Queries

- Repository traits (`UserRepo`, `CategoryRepo`, `ProductRepo`,
  `ShoppingListRepo`, `RecipeRepo`, `InvitationRepo`, `AdminRepo`) are the
  only surface the web layer sees.
  Add methods there, not in handlers.
- Foreign keys prove that a referenced row exists, not that it belongs to the
  acting user (RR-0001). A write that sets a reference to another owner-scoped
  row (`category_id`, `product_id`) verifies ownership in the same statement —
  `INSERT … SELECT … FROM categories WHERE id = ? AND owner_id = ?` — or in the
  same transaction, and cross-table reads also join on `owner_id`.
- The scaffold uses runtime `sqlx::query(...)` so builds don't require a live
  DB. **Target state:** once the schema is stable, migrate to `sqlx::query!`
  + `sqlx::query_as!` + `cargo sqlx prepare` offline cache under `.sqlx/`,
  then add `SQLX_OFFLINE=true` to CI. Track in a Mission Brief.
- Prefer named columns in SELECT, never `*`.
- Email addresses are unique and compared ignoring ASCII case: the
  `users_email_nocase` index, and `COLLATE NOCASE` in every comparison on
  `users.email` (MB-0044, RR-0028). The address is stored as typed.
- One `repo/sqlite/<area>.rs` per repo trait implementation (RR-0009).
- The list tables (`shopping_lists`, `list_items`, `list_shares`) are named
  only in `repo/sqlite/list.rs`, behind the central access predicate;
  `repo::share`'s bypass guard enforces it (RR-0009 constraint 3).

## Point-in-time data

- A shopping-list item copies the product's name, quantity, and category when
  it is added, and later catalog edits don't change it (RR-0001). The category
  copy freezes *membership* only: the item keeps its `category_id`, while that
  category's name and position stay live.
- Removing a list item sets `removed_at` (soft delete). Default reads filter
  on `removed_at IS NULL`, and a removed item can be restored.

## Pool

- `SqlitePool` wraps `sqlx::SqlitePool`. The web layer never imports `sqlx`.
- `connect` sets, on every connection (MB-0025):
  - `create_if_missing(true)` and `foreign_keys(true)`;
  - `journal_mode(Wal)`: readers don't block the writer or each other. WAL is
    stored in the file, so an existing rollback-journal database is switched
    over on first open. The database is then three files (`.db`, `-wal`,
    `-shm`) that must stay together on a local filesystem (not a network
    share). Copying the `.db` alone is not a backup: use `.backup` or
    `VACUUM INTO`.
  - `synchronous(Normal)`: in WAL mode the database can't be corrupted and an
    app crash loses nothing; a power or OS failure can roll back the last
    commits. Switching to `Full` is a data-owner decision.
  - `busy_timeout` of 10 s (`BUSY_TIMEOUT`): a write that finds another in
    progress waits instead of failing with "database is locked". It stays
    below the pool's 30 s acquire timeout, so a stuck lock surfaces as an
    error, not a hung request.
- Settings go through the typed `SqliteConnectOptions` methods, not raw
  `PRAGMA` statements, so every new connection gets them.
- Max 8 connections (`MAX_CONNECTIONS`). SQLite serializes writers, so more
  connections only add readers. Don't raise this without benchmarking write
  contention.
- One process per database file. Running a second instance against the same
  file is unsupported.
- Tests that need WAL or locking behaviour use a temp-file database; `:memory:`
  reports journal mode `memory`.

## Transactions

- Multi-statement writes use `sqlx::Transaction`. Acquire → operate → commit;
  rollback on error. Repository methods that mutate multiple rows must own the
  transaction, not accept one from a caller.

## Red flags

- Changing a committed migration file → stop, open a new migration.
- Reading or writing `sqlite_master` directly → use a migration.
- Dynamic SQL built via `format!` with user input → forbidden; use binds.
