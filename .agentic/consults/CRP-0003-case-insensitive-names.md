# Consultation Request Pack — Case-insensitive names: migration, backfill, numbering

```yaml
---
id: CRP-0003
mission: MB-0010
owner: "@juuso-kalliomaki"
target_role: data-owner
risk_tier: medium
urgency: medium
state: answered
runbook_trigger: "RR-0001 constraint 8 (a migration not listed in the RR needs an amendment through a consult); .agentic/runbooks/migration.md entry gate 2–3 and Forbidden (UPDATE … SET in a migration needs explicit RR authorization); feature.md escalation trigger `migrations/**` → data-owner"
---
```

## Decision statement

Amend RR-0001 to authorize `migrations/0004_name_keys.sql` for MB-0010, a
per-owner, case-insensitive `name_key` on `categories` and `products`, and to
renumber the migrations planned after it.

The owner already chose an answer to each sub-decision while reviewing
MB-0010 (PR #6, 2026-10-29). This pack records those choices for the RR-0001
amendment, which closes it.

## Options and trade-offs

### 1. Backfilling keys for existing rows

- **A — SQL `lower()` plus explicit `replace()` for Å/Ä/Ö. Chosen.**
  - Pros: everything happens inside the migration's transaction; no startup
    code.
  - Cons: an existing row with another non-ASCII capital (e.g. "CAFÉ") keeps a
    partly-lowered key until it's renamed. MB-0010 lets the implementer add
    more `replace()` pairs, and its P7 test checks that SQL and Rust agree.
- **B — a nullable column filled by Rust at startup.**
  - Pros: exact Unicode lowercasing for every row.
  - Cons: a data step that lives outside sqlx migrations, and rows without a
    key until it has run.

### 2. Existing names that differ only by case

- **A — suffix them. Chosen.** In each group, the oldest row (by
  `created_at`, then `id`) keeps the plain key. Every other row gets
  `<key>#<row id>`. No names or data change; the user resolves duplicates by
  renaming or deleting one.
- **B — fail the upgrade loudly**, leaving the database unchanged. Safe, but
  the app won't start until someone fixes the data by hand.
- Merging or deleting rows was excluded, because it loses data.

### 3. Migration number and sequencing

- **A — take `0004`, land before MB-0003, renumber the planned files. Chosen.**
  MB-0003's "reuse the product on repeat" lookup is then built on the final
  matching rule.
- **B — take the next free number at implementation time.** No renumbering,
  but MB-0003 could land first with case-sensitive lookups and need rework.

## Recommendation and question

Owner decisions: **1A, 2A, 3A**, as recorded in MB-0010.

Crisp question to answer: **Approve amending RR-0001 to:**

- **(a)** list `migrations/0004_name_keys.sql` (MB-0010);
- **(b)** renumber the planned migrations — MB-0003 → `0005_ingredient_product.sql`
  and MB-0004 → `0006_list_item_products.sql` in RR-0001 and in both briefs
  (plus MB-0005 and MB-0006, which refer to "MB-0004's `0005`"), and MB-0008 →
  `0007_list_shares.sql` in the MB-0008 brief only (RR-0002 lists no
  filenames);
- **(c)** explicitly authorize, for each of the two tables, the key-backfill and
  duplicate-suffix `UPDATE … SET` statements in `0004_name_keys.sql`?

## Blast radius and rollback

- **What can break:** `0004` runs on every existing database at the next boot.
  sqlx applies each SQLite migration and its bookkeeping row in one
  transaction, so if any statement fails — e.g. the unique index — the
  database stays at `0003` and the app doesn't start.
- **Who is affected:** every user with categories or products. Names and data
  are unchanged. Case-only duplicates stay visible until the user resolves
  them, and creating a new case-only duplicate is rejected from then on.
- **Rollback plan:** a follow-up migration drops the two unique indexes. The
  `name_key` column can stay unused, since SQLite `DROP COLUMN` isn't
  available everywhere. Then revert the repo code to case-sensitive matching.

## Minimal context

- RR-0001's resolved open question: names match case-insensitively.
  Constraint 8: a migration not listed in the RR needs an amendment through a
  consult first.
- MB-0010 has the full design: a Rust-computed `name_key` plus a unique index
  on `(owner_id, name_key)`. SQLite's `NOCASE` and `lower()` fold only ASCII
  without ICU, and sqlx's bundled SQLite has no ICU.
- Out of scope here: lowercasing emails in kauppa-web is a separate,
  security-tier change to `routes/auth.rs` that needs its own consult.

## Evidence bundle (Hardened)

- SQLite prototype of the chosen SQL (PR #6):
  - `ÄYRÄS` → `äyräs` and `Åland` → `åland`, matching Unicode lowercasing.
  - `CAFÉ` → `cafÉ`: the documented gap for 1A.
  - One owner's "Maito", "maito", "MAITO" → `maito`, `maito#<id>`, `maito#<id>`.
  - Another owner's "maito" stays `maito`.
  - The unique index builds without errors.
- sqlx 0.8.6 wraps each SQLite migration in a single transaction
  (`sqlx-sqlite/src/migrate.rs`, `apply`).
- Data volume: tens to low hundreds of rows per user, so the backfill is
  trivial.
