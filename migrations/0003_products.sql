-- 0003_products — per-user product catalog / favorites (MB-0002).
-- Resolution: RR-0001 (Option A, product-centric catalog), see
-- .agentic/resolutions/RR-0001-product-category-data-model.md.
-- Intent: one definition of "a product" per user, classified by a per-user
-- category (0002). Favorites use it now; recipe ingredients (MB-0003) and
-- shopping-list items (MB-0004) will reference it later.
--
-- FK divergence: `category_id` is `ON DELETE SET NULL`, not the default
-- CASCADE. Deleting a taxonomy entry must never destroy products — they become
-- uncategorized instead. That is also why `category_id` is nullable here;
-- "a favorite must have a category" is enforced at the handler layer (RR-0001).
--
-- Consistent with 0001/0002: UUIDs as TEXT, timestamps as RFC3339 TEXT,
-- booleans as INTEGER NOT NULL, STRICT.

CREATE TABLE products (
    id          TEXT PRIMARY KEY,
    owner_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    category_id TEXT REFERENCES categories(id) ON DELETE SET NULL,
    position    INTEGER NOT NULL DEFAULT 0,
    is_favorite INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    UNIQUE (owner_id, name)
) STRICT;

CREATE INDEX idx_products_owner_category ON products(owner_id, category_id, position);
