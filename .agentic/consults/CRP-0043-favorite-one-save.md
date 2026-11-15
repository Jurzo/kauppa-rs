# Consultation Request Pack — One Save for a favorite: what Save does with a product that has no category

```yaml
---
id: CRP-0043
mission: MB-0058
owner: "@juuso-kalliomaki"
target_role: product
risk_tier: low
urgency: low
state: answered
runbook_trigger: "MB-0058 autonomy.escalate (a product from recipes that has no category: may a rename leave it uncategorized, or must Save require a category → owner)"
---
```

## Decision statement

When the name and category of a product on `/favorites` are saved by one
Save button, decide what Save does when the category picker is left at
"Choose a category".

## Options and trade-offs

### Option A — Keep what's there (recommended)

An empty picker means "don't change the category". A product without one
(a recipe product, or a favorite whose category was deleted) can be
renamed and stays uncategorized; picking a category sets it. No one can
clear a category, which matches today (there is no "clear" today either).

- Pros: every edit possible today stays possible; no new state.
- Cons: the picker's "Choose a category" means "unchanged" only for
  uncategorized products; for others it's preselected with the current
  one, so there's no ambiguity in practice.
- Risk: none beyond today's.

### Option B — Save always requires a category

The picker is `required`; renaming an uncategorized product forces a pick.

- Pros: every product edited gets a category; recipe products become
  ready to favorite.
- Cons: a rename alone becomes impossible for uncategorized products
  (today it works).
- Risk: users pick an arbitrary category just to rename.

### Option C — Add a "No category" choice

The picker gains "No category", which clears it.

- Pros: explicit.
- Cons: a new ability (clearing a favorite's category) the app doesn't
  have; favorites are created only with a category.
- Risk: favorites drifting into Uncategorized.

## Recommendation and question

Recommended option: A.
Because: one Save should combine the two existing forms, not change what
either allows.

Crisp question to answer: When a product has no category and the picker
is left empty, does Save rename it and leave it uncategorized (A), require
a category (B), or should there be an explicit "No category" choice (C)?

## Blast radius and rollback

What can break: product edits on `/favorites`.
Who is affected: every user editing their catalog.
Rollback plan: revert the implementation PR.

## Minimal context

- Today `_product_edit.hbs` has **Rename** (`POST /favorites/{id}/rename`,
  name only) and **Change** (`POST /favorites/{id}/category`), and the
  category route rejects an empty value (`parse_category`).
- Products from recipes are created without a category
  (`is_favorite = 0`); a favorite can lose its category when the category
  is deleted (`ON DELETE SET NULL`, migration 0003), and then shows under
  Uncategorized.
- MB-0052 / RR-0033 did the same for list items.

## Evidence bundle (Hardened)

- `crates/kauppa-web/src/routes/products/mod.rs`: `rename`, `recategorize`,
  `parse_category`.
- `templates/products/_category_picker.hbs`: the empty option is
  `disabled`, selected only when there is no current category.

## Owner's answer (2026-11-15)

C: the category picker gains an explicit "No category" choice, which
clears the category. Recorded in RR-0043 (scaffold; the owner signs).
