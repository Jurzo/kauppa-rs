# Consultation Request Pack — Collapsible favorite categories: which groups stay open after an action, and the "From recipes" section

```yaml
---
id: CRP-0044
mission: MB-0059
owner: "@juuso-kalliomaki"
target_role: product
risk_tier: low
urgency: low
state: open
runbook_trigger: "MB-0059 autonomy.escalate (other groups after an action; the 'From recipes' section; the add form → owner)"
---
```

## Decision statement

With every category on `/favorites` collapsed by default, decide which
groups are open after an action re-renders the page, and how the "From
recipes" section collapses.

## Options and trade-offs

### Q1 — Groups after an action (add, save, ↑/↓, delete, make favorite)

Every action today swaps the whole `#favorites` fragment. A `<details>`
element's open state is not sent with a form, so without JavaScript the
server can't know which groups the user had open; it knows only the group
the action touched.

#### Option A — Only the touched group(s) open (recommended)

The swapped catalog has the group the action happened in open (two groups
when a Save moves a product to another category, MB-0058), all others
closed.

- Pros: simple, stateless, no JavaScript; the user stays where they were.
- Cons: another group the user had opened closes after the action.
- Cost: a flag in the view model; small.

#### Option B — Swap only the touched group

Actions target their own group's element instead of `#favorites`, so
groups the user opened stay open because they aren't replaced. Actions
that cross groups (a category change, adding to another category) still
swap the whole catalog as in A.

- Pros: open groups survive most actions.
- Cons: two swap paths per action and more fragments to test; ↑/↓ and the
  add form need per-group targets.
- Cost: medium.

#### Option C — Remember open groups

A cookie or per-user setting of open groups.

- Cons: setting it on toggle needs JavaScript (forbidden) or a request per
  toggle; disproportionate.

### Q2 — The "From recipes" section

#### Option A — The whole section is one collapsed `<details>` (recommended)

Its heading ("From recipes") is the summary; inside, its category groups
are shown as today (headings, not collapsible). Opened after an action
inside it.

- Pros: it is a secondary section; one click shows all of it.

#### Option B — Its groups collapse like the favorites' groups

- Pros: consistent.
- Cons: two levels of collapsing.

### Q3 — Add form and empty categories

- After adding a favorite, its category group is open (the touched group,
  Q1). Recommended: yes.
- Empty categories: today a category appears on `/favorites` only when it
  has favorites (`list_for_owner` groups favorites by category), so there
  are no empty groups to show; no change recommended.

## Recommendation and question

Recommended: Q1 A, Q2 A, Q3 as above.
Because: no JavaScript; small change; the user lands on what they just
did.

Crisp question to answer: (1) After an action, is only the touched
group open (A), or should untouched groups keep their state by swapping
per group (B)? (2) Is "From recipes" one collapsed section (A) or
collapsible per group (B)? (3) Agree that the add form opens its group and
empty categories stay hidden?

## Blast radius and rollback

What can break: the favorites page's markup and its tests.
Who is affected: every user's `/favorites`.
Rollback plan: revert the implementation PR.

## Minimal context

- `templates/products/_catalog.hbs`: `<section class="product-group">`
  with `<h2>` per category; then the 'From recipes' section with `<h3>`
  groups.
- MB-0040's navigation panel and the item edit sections already use native
  `<details>`/popover with no JavaScript.

## Evidence bundle (Hardened)

- `crates/kauppa-core/src/repo/sqlite/product.rs` `list_for_owner`: groups
  only favorites (`is_favorite = 1`) by category, LEFT JOIN so
  uncategorized favorites form one group.
- Every form in `_catalog.hbs` and `_product_edit.hbs` targets
  `#favorites` with `hx-swap="outerHTML"`.

## Owner's answer (2026-11-15)

1. B: swap only the touched group, so groups the user opened stay open;
   actions that cross groups swap the whole catalog with the touched groups
   open.
2. As recommended (A): "From recipes" is one collapsed section.
3. As recommended: the add form opens its group; empty categories stay
   hidden.

Recorded in RR-0044 (scaffold; the owner signs).
