-- 0012_catalog_shares — share favorites and recipes for import (MB-0061).
-- Resolution: RR-0048 (sub-answer 2), see
-- .agentic/resolutions/RR-0048-catalog-sharing-and-import.md.
-- Intent: one row lets `grantee_id` read `owner_id`'s favorites and/or
-- recipes (the two switches) to import copies; every such read goes through
-- the predicate in kauppa-core's `repo/sqlite/catalog_access.rs`. One share
-- per pair; a share with both switches off is deleted, not kept. Deleting
-- either user deletes the share (CASCADE); no share points at a single
-- favorite or recipe.
-- `recipes.imported_from` records the recipe an imported copy came from, so
-- a repeat import is refused (sub-answer 7); deleting the original sets it to
-- NULL and leaves the copy.
--
-- Purely additive: a new table and index, and one nullable column. An older
-- binary ignores both.

CREATE TABLE catalog_shares (
    id         TEXT PRIMARY KEY,
    owner_id   TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    grantee_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    favorites  INTEGER NOT NULL CHECK (favorites IN (0, 1)),
    recipes    INTEGER NOT NULL CHECK (recipes IN (0, 1)),
    created_at TEXT NOT NULL,
    UNIQUE (owner_id, grantee_id),
    CHECK (owner_id <> grantee_id)
) STRICT;
CREATE INDEX idx_catalog_shares_grantee ON catalog_shares(grantee_id);

ALTER TABLE recipes ADD COLUMN imported_from TEXT
    REFERENCES recipes(id) ON DELETE SET NULL;
