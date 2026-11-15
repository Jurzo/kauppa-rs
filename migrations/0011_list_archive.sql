-- 0011_list_archive — archive shopping lists (MB-0062).
-- Resolution: RR-0045 (sub-answer 1), see
-- .agentic/resolutions/RR-0045-archive-lists.md.
-- Intent: `archived_at` is when the owner archived the list (RFC 3339, like
-- the other timestamps); NULL means active, so every existing list stays
-- active. Archiving and restoring don't change `updated_at`. Read-only
-- enforcement is in kauppa-core's `begin_write`, next to the central access
-- predicate (RR-0045 sub-answer 4).
--
-- Purely additive: one nullable column, no backfill, no data rewrite; an
-- older binary ignores it (RR-0045 constraint 3).

ALTER TABLE shopping_lists ADD COLUMN archived_at TEXT;
