# Resolution Record — Amend RR-0001: case-insensitive names migration

```yaml
---
id: RR-0003
closes: CRP-0003
mission: MB-0010
decided_by: "@juuso.kalliomaki"
role: data-owner
risk_tier: medium
decided_at: 2026-10-29T00:00:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/resolutions/RR-0001-product-category-data-model.md
  - .agentic/missions/MB-0003-recipe-authoring.md
  - .agentic/missions/MB-0004-assemble-list.md
  - .agentic/missions/MB-0005-add-recipe-to-list.md
  - .agentic/missions/MB-0006-list-store-view.md
  - .agentic/missions/MB-0008-share-list.md
---
```

## Decision and outcome

Amend RR-0001, as asked in CRP-0003:

- **(a)** add `migrations/0004_name_keys.sql` (MB-0010) to RR-0001's migration
  list;
- **(b)** renumber the migrations planned after it by one (constraint 1);
- **(c)** authorize the statements in `0004_name_keys.sql` that write existing
  rows (constraint 2).

This amends RR-0001 without superseding it. RR-0001's decision (Option A) and
its constraints 1–9 still stand.

Not covered: lowercasing emails in kauppa-web. That is a security-tier change
to `routes/auth.rs` and needs its own consult.

## Rationale and trade-offs

The owner chose 1A, 2A, and 3A in CRP-0003:

- **1A — backfill in SQL** with `lower()` plus explicit Å/Ä/Ö `replace()`.
  The whole upgrade stays inside one migration transaction, with no startup
  data step.
  - Accepted gap: existing rows with other non-ASCII capitals (e.g. "CAFÉ")
    keep a partly-lowered key until renamed.
  - Rejected: filling the keys from Rust at startup (1B) — exact, but a data
    step outside sqlx migrations.
- **2A — suffix case-only duplicates.** Nothing is deleted, merged, or
  renamed; the user resolves duplicates.
  - Rejected: failing the upgrade (2B) — safe, but the app won't start until
    the data is fixed by hand.
  - Rejected: merging or deleting rows — loses data.
- **3A — `0004` lands before MB-0003**, so MB-0003's product lookup is built on
  the final matching rule.
  - Rejected: the next free number at implementation time (3B) — risks
    MB-0003 landing with case-sensitive lookups and needing rework.

## Approvals and authority

Approved by data-owner @juuso.kalliomaki

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0003-case-insensitive-names.md`
- Mission Brief: `.agentic/missions/MB-0010-case-insensitive-names.md` (PR #6)
- Amends: `.agentic/resolutions/RR-0001-product-category-data-model.md`
- Migration under this decision: `migrations/0004_name_keys.sql` — MB-0010
  (planned)
- Evidence links: SQLite prototype of the chosen SQL (CRP-0003, PR #6); sqlx
  0.8.6 applies each SQLite migration in one transaction.

## Constraints and follow-on rules (Hardened)

1. **RR-0001's migration list becomes:**

   | File | Brief | State |
   | --- | --- | --- |
   | `0002_categories.sql` | MB-0001 | merged |
   | `0003_products.sql` | MB-0002 | merged |
   | `0004_name_keys.sql` | MB-0010 | planned |
   | `0005_ingredient_product.sql` | MB-0003 | planned (was `0004`) |
   | `0006_list_item_products.sql` | MB-0004 | planned (was `0005`) |

   MB-0008's `0006_list_shares.sql` becomes `0007_list_shares.sql`. RR-0002
   governs that file and lists no filenames, so only the MB-0008 brief
   changes.
2. **`0004_name_keys.sql` may contain exactly these statements**, for each of
   `categories` and `products`:
   - `ALTER TABLE … ADD COLUMN name_key TEXT`;
   - one backfill `UPDATE … SET name_key = lower(<replace chain>)`. The chain
     folds Å/Ä/Ö and any further uppercase→lowercase pairs MB-0010 allows;
   - one `UPDATE … SET name_key = name_key || '#' || id`, limited to rows that
     are not the oldest (by `created_at`, then `id`) in their owner's
     case-only duplicate group;
   - `CREATE UNIQUE INDEX … ON <table>(owner_id, name_key)`.

   No other `UPDATE`, no `DELETE`, and no `DROP`. `name` and all other columns
   are never written.
3. **One rule for name matching.** Every write to `categories.name` or
   `products.name` sets `name_key` through a single kauppa-core helper. Every
   lookup by name — including MB-0003's "reuse the product on repeat" — goes
   through `name_key`.
4. **Sequencing.** MB-0010 merges before MB-0003 starts.

## Roll-back into the system (Hardened)

To apply after approval; nothing here is applied yet.

- `RR-0001`:
  - replace the "Migrations under this decision" list with constraint 1;
  - add a history entry "Amended by RR-0003";
  - note that constraint 8's list now lives as amended here.
- `MB-0003`: `0004_ingredient_product.sql` → `0005_ingredient_product.sql`.
- `MB-0004`: `0005_list_item_products.sql` → `0006_list_item_products.sql`.
- `MB-0005` and `MB-0006`: "MB-0004's `0005`" → "MB-0004's `0006`".
- `MB-0008`: `0006_list_shares.sql` → `0007_list_shares.sql`.
- `CRP-0003`: `state: open` → `answered`.

## Supersession and history

Status: active. Amends RR-0001; supersedes nothing.
