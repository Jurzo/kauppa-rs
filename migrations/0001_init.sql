-- Initial schema for kauppa-rs.
-- UUIDs are stored as TEXT so sqlite inspection stays legible.
-- Timestamps are stored as TEXT (RFC3339, the format sqlx uses for DateTime<Utc>).

CREATE TABLE users (
    id            TEXT PRIMARY KEY,
    email         TEXT NOT NULL UNIQUE,
    display_name  TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    created_at    TEXT NOT NULL
) STRICT;

CREATE TABLE shopping_lists (
    id         TEXT PRIMARY KEY,
    owner_id   TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE INDEX idx_shopping_lists_owner ON shopping_lists(owner_id, updated_at DESC);

CREATE TABLE list_items (
    id       TEXT PRIMARY KEY,
    list_id  TEXT NOT NULL REFERENCES shopping_lists(id) ON DELETE CASCADE,
    text     TEXT NOT NULL,
    quantity TEXT,
    checked  INTEGER NOT NULL DEFAULT 0,
    position INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE INDEX idx_list_items_list ON list_items(list_id, position);

CREATE TABLE recipes (
    id           TEXT PRIMARY KEY,
    owner_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title        TEXT NOT NULL,
    instructions TEXT NOT NULL DEFAULT '',
    servings     INTEGER,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
) STRICT;

CREATE INDEX idx_recipes_owner ON recipes(owner_id, updated_at DESC);

CREATE TABLE ingredients (
    id        TEXT PRIMARY KEY,
    recipe_id TEXT NOT NULL REFERENCES recipes(id) ON DELETE CASCADE,
    text      TEXT NOT NULL,
    quantity  TEXT,
    position  INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE INDEX idx_ingredients_recipe ON ingredients(recipe_id, position);
