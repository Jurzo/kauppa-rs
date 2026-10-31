-- 0005_ingredient_product — recipe ingredients reference catalog products (MB-0003).
-- Resolution: RR-0001 (Option A, product-centric catalog; file listed as
-- amended by RR-0003), see
-- .agentic/resolutions/RR-0001-product-category-data-model.md.
-- Intent: an ingredient points at a product in the owner's catalog, which
-- carries its category. The existing `text` column stays as the ingredient's
-- denormalized display name, used when the product link is gone.
--
-- FK divergence: `product_id` is `ON DELETE SET NULL`, not the default CASCADE
-- (RR-0001 constraint 4). Deleting a product must never delete recipe
-- ingredients — they keep their text and become unlinked. A nullable FK column
-- can be added in place, without a table rebuild.

ALTER TABLE ingredients ADD COLUMN product_id TEXT REFERENCES products(id) ON DELETE SET NULL;
