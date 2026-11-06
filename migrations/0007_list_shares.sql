-- 0007_list_shares — share a shopping list with another user (MB-0008).
-- Resolution: RR-0002 (Option A, share to an existing user by email with
-- view / edit roles), see .agentic/resolutions/RR-0002-list-sharing-authz.md.
-- The filename follows RR-0003's renumbering (PR #7).
-- Intent: one row grants one existing user (`grantee_id`) access to one list
-- at a `role`: `view` reads, `edit` also adds, checks, removes and restores
-- items. Owners never have a row; ownership stays `shopping_lists.owner_id`.
-- Every list and list-item query authorizes through the single access
-- predicate in kauppa-core `repo::sqlite` (RR-0002 constraint 1); revoking is
-- deleting the row, so it takes effect on the next request (constraint 4).
--
-- Both foreign keys are the default ON DELETE CASCADE: deleting the list or
-- the grantee's account deletes the grant. Purely additive: a new table and
-- index, no change to existing rows.

CREATE TABLE list_shares (
    id         TEXT PRIMARY KEY,
    list_id    TEXT NOT NULL REFERENCES shopping_lists(id) ON DELETE CASCADE,
    grantee_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role       TEXT NOT NULL CHECK (role IN ('view', 'edit')),
    created_at TEXT NOT NULL,
    UNIQUE (list_id, grantee_id)
) STRICT;

-- "Lists shared with me" and the access predicate look up by grantee.
CREATE INDEX idx_list_shares_grantee ON list_shares(grantee_id, list_id);
