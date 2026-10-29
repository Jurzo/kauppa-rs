-- Per-user product categories (MB-0001, approved via RR-0001).
-- Categories are the per-user taxonomy that favorites, recipe ingredients, and
-- shopping-list items are grouped by. Owner-scoped and user-ordered.
-- Consistent with 0001: UUIDs as TEXT, timestamps as RFC3339 TEXT, STRICT.

CREATE TABLE categories (
    id         TEXT PRIMARY KEY,
    owner_id   TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    position   INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    UNIQUE (owner_id, name)
) STRICT;

CREATE INDEX idx_categories_owner ON categories(owner_id, position);
