-- 0006_list_item_products — list items from favorites, categories, soft delete (MB-0004).
-- Resolution: RR-0001 (Option A, product-centric catalog; file listed as
-- amended by RR-0003), see
-- .agentic/resolutions/RR-0001-product-category-data-model.md.
-- Intent: a list item can come from a catalog product (`product_id`) and
-- belongs to one of the owner's categories (`category_id`), or is free text
-- with neither. `removed_at` makes removal a soft delete (RR-0001 constraint
-- 7): NULL is active, an RFC3339 timestamp is removed.
--
-- Point in time (RR-0001 constraint 6): `list_items.text` and `category_id`
-- are copied from the product when the item is added and never follow later
-- catalog edits. The copy freezes category *membership* only: the category's
-- name and order stay live.
--
-- FK divergence: `product_id` and `category_id` are `ON DELETE SET NULL`, not
-- the default CASCADE (RR-0001 constraint 4). Deleting a product or category
-- must never delete list items — they become unlinked or uncategorized. All
-- three columns are nullable, so each ADD COLUMN applies in place.

ALTER TABLE list_items ADD COLUMN product_id TEXT REFERENCES products(id) ON DELETE SET NULL;
ALTER TABLE list_items ADD COLUMN category_id TEXT REFERENCES categories(id) ON DELETE SET NULL;
ALTER TABLE list_items ADD COLUMN removed_at TEXT;
