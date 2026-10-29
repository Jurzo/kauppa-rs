# Resolution Record — Product/category data model

```yaml
---
id: RR-0001
closes: CRP-0001
mission: MB-0001
decided_by: "@juuso-kalliomaki"
role: data-owner
risk_tier: medium
decided_at: 2026-10-28T00:00:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/mentorship/persistence.md
  - .agentic/runbooks/migration.md
---
```

## Decision and outcome

Approved option A.

## Rationale and trade-offs

Avoids data duplication and cost of change is still low.

## Approvals and authority

Approved by @juuso.kalliomaki

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0001-product-category-data-model.md`
- Mission Brief:
  - `.agentic/missions/MB-0001-categories.md`
  - `.agentic/missions/MB-0002-products-favorites.md`
  - `.agentic/missions/MB-0003-recipe-authoring.md`
  - `.agentic/missions/MB-0004-assemble-list.md`
- Migrations under this decision (one append-only file per Mission Brief, as
  the briefs split it; CRP-0001 had sketched a single
  `0002_products_categories.sql`):
  - `migrations/0002_categories.sql` — MB-0001 (merged)
  - `migrations/0003_products.sql` — MB-0002 (merged)
  - `migrations/0004_ingredient_product.sql` — MB-0003 (planned)
  - `migrations/0005_list_item_products.sql` — MB-0004 (planned)
- Evidence links:
  - MB-0001: PR #2 (merged into `agentic/scaffold-mission-briefs`; reached
    `main` via PR #4)
  - MB-0002: PR #4

## Constraints and follow-on rules (Hardened)

-

## Roll-back into the system (Hardened)

-

## Supersession and history

Status: active

- 2026-10-29: clerical update. Linked-artifact paths corrected, migration
  filenames listed (migration runbook entry gate, item 3), evidence links
  added. Decision unchanged.
