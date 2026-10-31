-- 0004_name_keys — case-insensitive category and product names (MB-0010).
-- Resolution: RR-0003 (amends RR-0001), see
-- .agentic/resolutions/RR-0003-case-insensitive-names-amendment.md.
-- Intent: names stay as typed; uniqueness per owner moves to `name_key`, which
-- the app computes with Rust's `to_lowercase` (kauppa-core `repo::name_key`).
--
-- `name_key` is nullable: ADD COLUMN can't add NOT NULL without a default, and
-- a shared default would collide in the unique index. The repo sets it on
-- every write. `UNIQUE(owner_id, name)` from 0002/0003 stays (the new index
-- implies it; dropping it would mean a table rebuild).
--
-- Per table, in order (RR-0003 constraint 2 authorizes exactly these):
--   1. add the column;
--   2. backfill keys for existing rows. SQLite's lower() folds only ASCII, so
--      each non-ASCII capital is replaced first (Å Ä Ö, plus Õ Ü É Ø Æ, which
--      MB-0010 allows). Known gap: other non-ASCII capitals keep a partly
--      lowered key until the row is renamed;
--   3. suffix case-only duplicates: in each owner's group, the oldest row (by
--      created_at, then id) keeps the plain key and every other row gets
--      `<key>#<id>`. Nothing is deleted, merged, or renamed;
--   4. create the unique index.
-- `name` and every other column are never written.

ALTER TABLE categories ADD COLUMN name_key TEXT;

UPDATE categories
   SET name_key = lower(replace(replace(replace(replace(replace(replace(replace(replace(name, 'Å', 'å'), 'Ä', 'ä'), 'Ö', 'ö'), 'Õ', 'õ'), 'Ü', 'ü'), 'É', 'é'), 'Ø', 'ø'), 'Æ', 'æ'));

UPDATE categories
   SET name_key = name_key || '#' || id
 WHERE EXISTS (
       SELECT 1 FROM categories AS older
        WHERE older.owner_id = categories.owner_id
          AND older.name_key = categories.name_key
          AND (older.created_at < categories.created_at
               OR (older.created_at = categories.created_at AND older.id < categories.id)));

CREATE UNIQUE INDEX idx_categories_owner_name_key ON categories(owner_id, name_key);

ALTER TABLE products ADD COLUMN name_key TEXT;

UPDATE products
   SET name_key = lower(replace(replace(replace(replace(replace(replace(replace(replace(name, 'Å', 'å'), 'Ä', 'ä'), 'Ö', 'ö'), 'Õ', 'õ'), 'Ü', 'ü'), 'É', 'é'), 'Ø', 'ø'), 'Æ', 'æ'));

UPDATE products
   SET name_key = name_key || '#' || id
 WHERE EXISTS (
       SELECT 1 FROM products AS older
        WHERE older.owner_id = products.owner_id
          AND older.name_key = products.name_key
          AND (older.created_at < products.created_at
               OR (older.created_at = products.created_at AND older.id < products.id)));

CREATE UNIQUE INDEX idx_products_owner_name_key ON products(owner_id, name_key);
