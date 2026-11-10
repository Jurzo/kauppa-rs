# Consultation Request Pack — Editors' typed text matching the list owner's favorites

```yaml
---
id: CRP-0024
mission: MB-0036
owner: "@juuso-kalliomaki"
target_role: security
risk_tier: medium
urgency: low
state: answered
runbook_trigger: "feature.md escalation trigger: shared-list authorization in `crates/kauppa-core/src/repo/sqlite/list.rs` and `crates/kauppa-web/src/routes/lists/` (RR-0002); MB-0036 autonomy.escalate (any change to list access checks or `begin_write` → security)"
---
```

## Decision statement

Decide whether an `edit` grantee's text in the list page's one add field
should match the **list owner's** favorites by `name_key` (RR-0003). A match
would add the owner's product with the owner's category. That needs
`ShoppingListRepo::add_product` at `ListAccess::Edit` instead of `Owner`,
which is an RR-0002 change, and it would show the owner's favorite names to
editors as suggestions.

## Current state

- MB-0036, in this PR: the owner's text matches the owner's favorites
  (`add_product`, unchanged). An editor's text is always a free-form,
  uncategorized item, as before MB-0036, and editors get no suggestions.
  The owner decided this on 2026-11-09: "Only owner can set the category and
  editor sees owners categories for the items". On a shared list,
  categories are the list owner's. Only the owner assigns them, and editors
  see them.
- `crates/kauppa-core/src/repo/sqlite/list.rs`:
  - ~391: `add_product` opens `begin_write(user_id, list_id, ListAccess::Owner)`,
    so an editor gets `Forbidden` (403).
  - ~397–404: the single INSERT copies `p.name` and `p.category_id`
    `FROM products p WHERE p.id = ?4 AND p.owner_id = ?5`. `?5` is
    `grant.owner_id()`, the list owner, so only the owner's products can be
    added, whoever calls.
  - 136 (`ITEM_SELECT`) and 155 (`STORE_VIEW_SELECT`):
    `LEFT JOIN categories c ON c.id = i.category_id AND c.owner_id = l.owner_id`.
    Item reads show only the list owner's categories. That is why an
    editor's own product or category must not be used, and this CRP doesn't
    propose it.
- `crates/kauppa-core/src/repo/mod.rs` ~218–224: the trait doc lists
  `add_product` among the methods that are "the owner's alone".
- `crates/kauppa-web/src/routes/lists/items.rs` ~76–88: `owner_favorite`
  looks up a favorite only when `ShoppingListRepo::access` is `Owner`.
- `crates/kauppa-web/src/routes/lists/mod.rs` ~210: `render_list` reads the
  catalog (categories, favorites, recipes) only for the owner. The suggestions
  are rendered only for the owner (`templates/lists/show.hbs` line 11 and
  `templates/lists/_list.hbs` line 11).
- The bypass guard `repo::share::list_queries_route_through_central_predicate`
  accepts either access level. It checks only that each method authorizes
  before it queries. RR-0002 constraint 2 sets the role semantics ("`edit` = add /
  check / remove"), and the trait doc, `security.md` and the MB-0008 tests
  (`sharing_roles::edit_role_cannot_administer`, which expects a 403 from
  `add_favorite`) encode "the owner's catalog stays the owner's".

## Options and trade-offs

### Option A — do nothing (recommended for now)

Editors keep free-form text, and the owner categorizes their items later.

- Pros: no change to authorization. Editors never see the owner's catalog.
  The behaviour matches today's role model and the owner's 2026-11-09
  answer. MB-0036 ships complete for the owner.
- Cons: an editor who types "maito" gets an uncategorized "maito" even
  when the owner has a favorite "Maito" in Kylmä. The owner, or the store
  view's uncategorized group, has to sort it out.
- Risk: none new.
- Cost / timeline: none.

### Option B — editors match the owner's favorites, without suggestions

`add_product` (or a new `add_owner_favorite_by_name`) runs at
`ListAccess::Edit`. It keeps `p.owner_id = grant.owner_id()` and
`is_favorite = 1`, and the name lookup moves inside the authorized write
transaction. Editors see no datalist.

- Pros: an editor's items land in the owner's categories automatically.
  The owner's product and category never leave the owner's catalog.
- Cons: it is an RR-0002 change: a catalog read at `edit`, and the trait doc,
  `security.md` and MB-0008's 403 expectations would need updating. It is
  also an oracle. An editor can learn whether the owner has a favorite of a
  given name, and its category, by adding an item and reading the result,
  though they already see the owner's category names on categorized items.
- Risk: medium. It widens what `edit` can read from the owner's catalog.
  Any id-based path (`add_favorite` by `product_id`) must stay owner-only.
- Cost / timeline: about a day plus an RR. The new method needs a bypass-guard
  entry and role tests.

### Option C — Option B plus the owner's favorite names as suggestions

The owner's favorite names go to editors in the datalist.

- Pros: the editor's experience matches the owner's.
- Cons: it exposes the owner's whole favorites list (names) to every
  editor. The names are personal catalog data the RR-0002 role model hasn't
  shared so far. It needs a `ProductRepo` read authorized through the list's
  central predicate, which is a new cross-module path.
- Risk: medium–high (data exposure beyond the list's own items).
- Cost / timeline: Option B plus about half a day and the RR text.

## Recommendation and question

Recommended option: A, and revisit if editors' uncategorized items become a
real annoyance.
Because: MB-0036's goal holds for the owner without touching authorization,
and B and C widen what an `edit` grant reveals of the owner's catalog.

Questions to answer:

1. Should an `edit` grantee's typed text match the list owner's favorites
   (adding the owner's product with the owner's category)? Yes means an
   RR-0002 amendment that lets `add_product` (or a name-based variant) run at
   `ListAccess::Edit`.
2. If yes, may editors also see the owner's favorite names as suggestions
   (Option C), or must matching be silent (Option B)?
3. If yes, does `POST /lists/{id}/favorites` (by product id) stay owner-only,
   so editors match only by name?
4. If no (Option A), close this CRP as answered. No RR is needed, since
   nothing changes.

## Blast radius and rollback

What can break: under B or C, a mis-scoped query could let an editor add or
see products outside the list owner's favorites.
Who is affected: owners who share lists with `edit`, and their editors.
Rollback plan: revert the PR. There is no schema change, and items already
added keep their copied name and category (RR-0001).

## Minimal context

- Brief: `.agentic/missions/MB-0036-one-add-field-with-favorite-suggestions.md`
  (P4: "Whose favorites are suggested and matched follows the RR for shared
  lists"; escalation "any change to list access checks or `begin_write`
  (RR-0002)" → security).
- RR-0002 (list sharing authz: central predicate, role semantics), RR-0003
  (`name_key`), RR-0001 (items copy name and category at add-time).

## Evidence bundle (Hardened)

- `cargo test -p kauppa-web routes::lists::tests::sharing_roles::add_follows_roles`
  pins today's split. The owner's "MAITO" adds the linked favorite in Kylmä.
  The editor's "maito" (the owner's favorite name) and "Omena" (the editor's
  own favorite name) are free-form and uncategorized, with no datalist. Only
  the owner can then set the category, and the editor sees it. A viewer's
  add is a 403.
- `cargo test -p kauppa-core repo::share::list_queries_route_through_central_predicate`
  is green, with no change to the predicate or the guard.
- Continuity: `.agentic/continuity/MB-0036.md`.
