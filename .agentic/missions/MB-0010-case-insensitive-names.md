# Mission Brief — Case-insensitive category and product names

```yaml
---
id: MB-0010
owner: "@juuso-kalliomaki"
goal: Make category and product names unique per user regardless of letter case — including Finnish letters, so "maito"/"Maito" and "äyräs"/"ÄYRÄS" collide — while still storing and showing each name exactly as the user typed it.
non_goals:
  - Email case-insensitivity — decided (kauppa-web lowercases emails before handling them, see "Related decision"), but it changes `routes/auth.rs` (security tier), so it ships separately.
  - Unicode normalization (NFC vs NFD) or full case folding (e.g. ß → ss) beyond Rust's `str::to_lowercase`; either would need a new crate.
  - Case-insensitive sorting — the name tiebreak in list ordering stays as it is.
  - Free-text shopping-list items (MB-0004); they have no uniqueness rule.
  - MB-0003's ingredient-to-product upsert itself — this brief only provides the matching rule it will use.
acceptance:
  - id: P1
    property: An owner can't have two categories whose names differ only by case, ASCII or not ("hevi" vs "Hevi", "äyräs" vs "ÄYRÄS"); the second create or rename is rejected with `Conflict`. Other owners are unaffected.
    check: cargo test -p kauppa-core repo::category::unique_ignoring_case
  - id: P2
    property: The same holds for product names, on create and on rename.
    check: cargo test -p kauppa-core repo::product::unique_ignoring_case
  - id: P3
    property: Names are stored and displayed exactly as typed (trimmed); only matching ignores case. Renaming a row to a different casing of its own name ("maito" → "Maito") succeeds.
    check: cargo test -p kauppa-core case_only_rename
  - id: P4
    property: "Migration `0004_name_keys.sql` applies to a database that already has categories and products: every existing row gets a match key from SQL `lower()` with Å/Ä/Ö folded by explicit `replace()`, and the new rule holds for writes after the upgrade."
    check: cargo test -p kauppa-core repo::name_key::backfill_existing_rows
  - id: P5
    property: Names that already differ only by case when the migration runs lose no data. In each such group the oldest row (by `created_at`, then `id`) keeps the plain key; every other row keeps its id, name, and data and gets the key `<key>#<row id>`. Renaming such a row to a non-clashing name gives it a plain key again.
    check: cargo test -p kauppa-core repo::name_key::existing_case_duplicates
  - id: P6
    property: On /categories and /favorites, adding a name that differs only by case from an existing one shows the existing "already exists" error, over htmx and without it.
    check: cargo test -p kauppa-web case_duplicate_rejected
  - id: P7
    property: The Rust key helper and the SQL backfill expression produce the same key for ASCII and Å/Ä/Ö names, so rows written before and after the upgrade match each other.
    check: cargo test -p kauppa-core name_key::matches_sql_backfill
context:
  - migrations/0002_categories.sql
  - migrations/0003_products.sql
  - crates/kauppa-core/src/repo/sqlite.rs
  - crates/kauppa-core/src/repo/mod.rs
  - .agentic/resolutions/RR-0001-product-category-data-model.md
  - .agentic/missions/MB-0003-recipe-authoring.md
  - .agentic/mentorship/persistence.md
  - .agentic/runbooks/migration.md
autonomy:
  may_decide:
    - the name and module of the single name-key helper in kauppa-core
    - extra uppercase-to-lowercase replace() pairs in the backfill beyond Å/Ä/Ö (e.g. É, Ü, Ø), as long as P7 covers them
    - test module layout, as long as the check commands above select the tests
    - wording of the Conflict message (it may echo the stored or the typed name)
  escalate:
    - { when: "a migration other than 0004_name_keys.sql, or a schema shape beyond the name_key column and its unique index", to: data-owner }
    - { when: the fix seems to need a new crate (e.g. Unicode normalization), to: platform-owner }
    - { when: "the change touches users, email, or auth paths", to: security-owner }
  forbidden:
    - edit a committed migration (0001–0003) — add a new numbered file
    - delete or merge user rows, or change any stored name, during the migration
    - drop or rename existing columns or constraints
    - add dependencies
    - lower lint strictness
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/persistence.md@v1
  - .agentic/mentorship/actix-web.md@v1
resolutions:
  - RR-0001
---
```

## Context

RR-0001 settled that names should match case-insensitively. The schema
doesn't do that yet: `categories` (0002) and `products` (0003) both enforce
`UNIQUE(owner_id, name)` with SQLite's default binary collation, so "Maito"
and "maito" can coexist. Names are trimmed before saving (`clean_name` in
`repo/sqlite.rs`), and a unique violation maps to `CoreError::Conflict`, which
the routes already show to the user.

The obvious SQLite tools don't fix this for Finnish. `COLLATE NOCASE`,
`lower()`, and `upper()` only fold ASCII unless SQLite is built with ICU, and
sqlx's bundled SQLite isn't: `lower('ÄYRÄS')` gives `ÄyrÄs`. Rust's
`str::to_lowercase` is Unicode-aware, and the crate already has it.

This matters beyond duplicate prevention. MB-0003 P2 says adding an ingredient
for a product name "reuses it on repeat"; that lookup has to use the same
matching rule, or "Maito" in a recipe would create a second product next to
the favorite "maito".

## Decisions (owner, 2026-10-29)

1. **Backfill: explicit SQL with replacements.** Existing rows get
   `lower(replace(replace(replace(name, 'Å', 'å'), 'Ä', 'ä'), 'Ö', 'ö'))`. New
   and renamed rows get their key from Rust. Known gap: an existing row with
   another non-ASCII capital (e.g. "CAFÉ") keeps a partly-lowered key until
   it's renamed. The implementer may add more `replace()` pairs (see
   `may_decide`).
2. **Duplicates: suffix them.** In each case-only duplicate group, the oldest
   row keeps the plain key and every other row gets `<key>#<row id>` (P5).
   Using the row id rather than `#2` means the suffixed key can't clash with
   a real name. Both rows stay visible; the user resolves them by renaming or
   deleting one.
3. **Numbering: this brief takes `0004`, and the planned files move up one.**
   This lands before MB-0003.

   | Brief | Planned file before RR-0003 | After renumbering |
   | --- | --- | --- |
   | MB-0010 | — | `0004_name_keys.sql` |
   | MB-0003 | `0004_ingredient_product.sql` | `0005_ingredient_product.sql` |
   | MB-0004 | `0005_list_item_products.sql` | `0006_list_item_products.sql` |
   | MB-0008 | `0006_list_shares.sql` | `0007_list_shares.sql` |

   RR-0001 lists the MB-0003/MB-0004 files, so applying this is the RR-0001
   amendment step (constraint 8: consult first). That amendment must also
   explicitly authorize the two backfill `UPDATE … SET` statements, which the
   migration runbook otherwise forbids in a migration. Done: CRP-0003 → RR-0003
   (approved); the renumbering was applied in PR #7.

## Conceptual plan

Add a `name_key` column to `categories` and `products`, plus a unique index on
`(owner_id, name_key)`. The column is nullable at the schema level:
`ADD COLUMN` can't add `NOT NULL` without a default, and a shared default would
collide in the unique index. The repo sets it on every write. The old
`UNIQUE(owner_id, name)` stays: the new index implies it, and dropping it would
mean a table rebuild.

Migration order in `0004_name_keys.sql`, per table:

1. add the column;
2. backfill the keys (decision 1);
3. suffix the duplicates (decision 2);
4. create the unique index.

One helper in kauppa-core computes keys (trimmed `to_lowercase()`). Both repos
use it, and so will MB-0003's lookup.

Checkpoints: P7 is green before the migration is written (it pins the SQL
expression to the Rust helper). P1–P3 are green at the repo layer before the
web test. P4/P5 are run against a database migrated to `0003` and seeded with
data, not only a fresh one.

Slow-mode triggers — stop and re-plan if:

- the backfill seems to need deleting or merging rows, or editing names;
- any option starts to need a new crate;
- MB-0003 has started meanwhile.

## Validation plan

- **P1–P3:** `kauppa-core` repo tests on a temp SQLite pool, with ASCII and
  non-ASCII pairs, create and rename, and a second owner as the control.
- **P4/P5:** run migrations `0001`–`0003` (a filtered sqlx `Migrator`), insert
  rows — including a case-only duplicate group for P5 — then run `0004`.
  Assert the keys, row ids, unchanged names, and the suffix outcome.
- **P6:** `routes::categories::case_duplicate_rejected` and
  `routes::products::case_duplicate_rejected`, through the shared
  `routes::test_support` fixtures.
- **P7:** a table of sample names, each run through both the helper and the
  SQL expression in SQLite, asserting equal keys.
- **Migration runbook S3:** boot against a copy of a populated
  `data/kauppa.db`, not only a fresh `kauppa-check.db`.

The `-p kauppa-core` checks only compile standalone once MB-0009 lands. Until
then, run them with `--workspace`.

## Related decision: email case (separate work)

Owner decision (2026-10-29): **the front end (kauppa-web) lowercases emails
before handling them**, so email case doesn't matter. That covers registration,
login, and any later email lookup, such as MB-0008's share-by-email. It isn't
part of this brief because it changes `crates/kauppa-web/src/routes/auth.rs`,
a security trigger path (consult + RR).

Notes for that work:

- Lowercase in the handlers, on the server. A browser-side transform alone can
  be bypassed. `autocapitalize="none"` on the email inputs is only a typing
  aid.
- Existing accounts registered with capitals won't match a lowercased lookup.
  The plan needs a data step for them, plus a rule for accounts that already
  differ only by case.
