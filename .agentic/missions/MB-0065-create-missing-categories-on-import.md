# Mission Brief — Create a missing category from the import and export review

```yaml
---
id: MB-0065
owner: "@juuso-kalliomaki"
goal: "When a favorite being imported (MB-0061) or exported from a list (MB-0060) carries a category name the user doesn't have, the review step lets the user create that category in their own catalog, instead of only picking one of theirs or skipping."
non_goals:
  - "Creating categories without the user choosing it on the review step (no silent creation)."
  - "Copying anything of the source's category but its name: never its id, order position or other products (RR-0001 constraint 9)."
  - "Changing how names match (by name, ignoring case, RR-0003), the conflict rows, the fingerprint or the summary's other counts."
  - "Moving products between categories after import, or drag and drop (MB-0066's follow-up)."
  - "JavaScript."
acceptance:
  - id: P1
    property: "On the review step, a row whose source category name matches none of the user's categories (a shared favorite, a list item for a grantee, and a recipe ingredient's new product if the consult includes it) offers 'Create ‹name›' beside the user's categories (and 'Skip' where the row offers it today); nothing is preselected; every row still needs an answer."
    check: cargo test -p kauppa-web routes::sharing::tests::import::create_missing_category_offered
  - id: P2
    property: "Confirming creates each chosen category once, in the user's own catalog, with the source's name (trimmed, as typed by the source), placed as the consult decides; rows that chose the same name share it; the category and the products are written in the same transaction as the rest of the import, or nothing is saved."
    check: cargo test -p kauppa-web routes::sharing::tests::import::created_category_is_shared_by_rows
  - id: P3
    property: "If the user already has a category with that name (ignoring case) when the confirmation runs, it is used instead of a second one, or the review is shown again as a stale plan (MB-0060 P4), as the consult decides; a 'Create' answer for a name that isn't in the plan is refused and saves nothing."
    check: cargo test -p kauppa-web routes::sharing::tests::import::create_race_and_unknown_name
  - id: P4
    property: "List export (MB-0060) behaves the same: a grantee's item whose list category name they don't have can be created on their review; the list owner's export, whose categories all match, is unchanged."
    check: cargo test -p kauppa-web routes::lists::tests::export::create_missing_category
  - id: P5
    property: "The summary counts created categories; every string in both catalogs; works without htmx and with htmx; the cross-site check covers the routes; existing tests unchanged except where the consult amends RR-0047 sub-answer 2 and RR-0048 sub-answer 5 (listed in the PR)."
    check: cargo test -p kauppa-web --all-targets
  - id: P6
    property: "In a browser on a phone width: an import from a share with a category the user lacks, created on the review, lands in the new category; the same for a grantee's list export."
    check: "manual: browser check on the release binary, recorded in MB-0065's Continuity Pack"
context:
  - crates/kauppa-core/src/export.rs
  - crates/kauppa-core/src/repo/sqlite/product.rs
  - crates/kauppa-core/src/repo/sqlite/recipe.rs
  - crates/kauppa-core/src/repo/sqlite/category.rs
  - crates/kauppa-web/src/routes/review.rs
  - crates/kauppa-web/src/routes/sharing/
  - crates/kauppa-web/src/routes/lists/
  - templates/review/
  - .agentic/missions/MB-0060-list-export-to-favorites.md
  - .agentic/missions/MB-0061-share-favorites-and-recipes.md
  - .agentic/resolutions/RR-0001-product-category-data-model.md
  - .agentic/resolutions/RR-0003-case-insensitive-names-amendment.md
  - .agentic/resolutions/RR-0047-list-export-to-favorites.md
  - .agentic/resolutions/RR-0048-catalog-sharing-and-import.md
autonomy:
  may_decide:
    - "wording of the new choice and the summary line, through catalog keys in both languages"
    - "test names inside the modules above"
  escalate:
    - { when: "reversing 'no category is created' (RR-0047 sub-answer 2, RR-0048 sub-answer 5, both rejected creating categories): which flows get it (shared favorites, recipe ingredients, a grantee's list export) (decided: RR-0052, all three flows)", to: owner }
    - { when: "where a created category goes in the user's order (last, or after its neighbours in the source), and its name's spelling (the source's, or normalised) (decided: RR-0052, last in the user's order, the source's name trimmed)", to: owner }
    - { when: "a review answer that creates rows outside `products` (the import and export transactions write `products` and `recipes` only today; RR-0047 constraint, RR-0048), and the race with a category of the same name made meanwhile (decided: RR-0052, inside the same transaction, one insert path; a category made meanwhile shows the review again)", to: data-owner }
    - { when: "what a recipient or grantee learns or keeps of the source's catalog (a category name only, never ids; RR-0001 constraint 9, RR-0048 sub-answer 5) (decided: RR-0052, the name only, sent as an index)", to: security }
    - { when: "the review's behaviour changes for list export and both imports at once (RR-0048 constraint 5 asks for its own consult) (decided: RR-0052, RR-0052 is that consult)", to: owner }
  forbidden:
    - "create a category the user didn't choose on the review"
    - "link the user's data to the source's category or product ids"
    - "write to the source's catalog"
    - "a migration (none is expected; one needs its own RR)"
    - "JavaScript or inline scripts"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
  - .agentic/mentorship/security.md@v1
  - .agentic/mentorship/persistence.md@v1
resolutions: [RR-0052]
---
```

## Context

Requested by the owner (2026-11-17): "Build upon the favorite import from
mb-0060 and mb-0061 so that the user can choose to create the categories
when the source item has a category that they don't yet have themselves."

What exists:

- **The shared review** (`routes/review.rs`, `templates/review/`) serves
  list export (MB-0060) and both imports (MB-0061). `export::plan` and
  `export::plan_ingredients` match the source's category names to the
  user's own by `name_key`; with no match, the row is uncategorized for the
  user and asks for one of their categories (or "Skip" where offered; a
  recipe ingredient's new product has no skip).
- **"No category is created"** was decided twice: RR-0047 sub-answer 2 (a
  grantee's export) and RR-0048 sub-answer 5 (imports); both rejected
  created or offered categories (RR-0047 Q2 B; RR-0048 Q5 B, C). RR-0048
  constraint 5: a change to the review's behaviour applies to list export
  and both imports and needs its own consult. This mission is that change.
- **The answers** are a stateless form with a SHA-256 fingerprint of the
  plan; the confirmation recomputes the plan inside one `BEGIN IMMEDIATE`
  transaction and accepts only category ids the user owns (MB-0060 P4). A
  "Create" answer carries a name, not an id, so the fingerprint and the
  answer validation must cover it.
- **Categories** are per user with case-insensitive unique names (RR-0003)
  and a position (`categories.position`, reordered by
  `CategoryRepo::reorder`).

## Conceptual plan

1. **Clarify (S1).** One CRP: the flows and placement (owner); writing
   categories inside the import and export transactions, and the name race
   (data); what is kept of the source (security); amending RR-0047
   sub-answer 2 and RR-0048 sub-answer 5. Done: CRP-0052 answered,
   RR-0052 approved (all A).
2. Core: the plan offers creation for unmatched names; the apply step
   creates them first in the same transaction (P2, P3).
3. Web: the review's new choice, the summary line, both flows (P1, P4,
   P5).
4. Browser check (P6).

## Validation plan

- Core tests for the plan and the transaction; route tests per flow; the
  owner's browser check.
