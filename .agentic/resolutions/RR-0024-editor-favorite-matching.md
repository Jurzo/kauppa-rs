# Resolution Record — Amend RR-0002: editors' typed text matches the list owner's favorites, silently

```yaml
---
id: RR-0024
closes: CRP-0024
mission: MB-0036
decided_by: "@juuso.kalliomaki"
role: security
risk_tier: medium
decided_at: 2026-11-10T08:09:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/resolutions/RR-0002-list-sharing-authz.md
  - .agentic/missions/MB-0036-one-add-field-with-favorite-suggestions.md
  - .agentic/consults/CRP-0024-editor-favorite-matching.md
---
```

## Decision and outcome

Approve CRP-0024 **Option B ("Yes, silently")**, amending RR-0002. When an
`edit` grantee's typed text matches one of the **list owner's** favorites by
`name_key` (RR-0003), the item gets the owner's product and the owner's
category. Editors are shown **no suggestions** of the owner's favorite names.

Sub-answers:

1. **Q1: match at `edit`.**
   - Matching runs in a name-based add at `ListAccess::Edit` (e.g.
     `ShoppingListRepo::add_owner_favorite_by_name`; the name is
     `may_decide`).
   - It opens the write with `begin_write(user_id, list_id, ListAccess::Edit)`
     and does the name lookup **inside that authorized transaction**.
   - It selects only `products p WHERE p.owner_id = grant.owner_id() AND
     p.name_key = ? AND p.is_favorite = 1`. The products come from the list
     owner's catalog (never the editor's), are favorites only (never
     recipe-created products, MB-0014), and the insert copies `p.name` and
     `p.category_id` in one statement, as `add_product` does.
   - With no match, the same call adds the text as a free-form,
     uncategorized item in the same transaction (today's `add_item`
     behaviour). An editor's own favorites never match.
2. **Q2: silent (no Option C).**
   - Editors get no `<datalist>` and no other listing of the owner's favorite
     names.
   - `render_list` keeps reading the owner's catalog only for the owner.
3. **Q3: id-based add stays owner-only.**
   - `POST /lists/{id}/favorites` (`add_favorite`) and
     `ShoppingListRepo::add_product` (by product id) stay at
     `ListAccess::Owner`.
   - Editors match only by typed name.
   - `sharing_roles::edit_role_cannot_administer`'s 403 from `add_favorite`
     is unchanged.
   <!-- check: Q3 inferred from "silently"; owner to confirm -->
4. **View grantees** still get 403 from every add, unchanged.
5. **Bypass guard.** The new method is covered by
   `repo::share::list_queries_route_through_central_predicate`: it
   authorizes through `begin_write` before any query. The guard and the
   central predicate themselves are unchanged.

**Accepted leak (low).** An editor can learn whether a name is one of the
owner's favorites, and its category and canonical spelling, by adding it and
seeing the item get a category. This is accepted. Editors already see the
owner's category names on categorized items, and each probe leaves a visible
item on the shared list.

**Amends RR-0002.** Two parts of RR-0002 change:

- **The decision's scope.** It reads: "the relaxation of the
  `owner_id = $current_user` authorization invariant to 'owner OR grantee with
  sufficient role' for shopping-list and list-item resources only". It gains
  one narrow exception. Inside an `edit`-authorized item add, the list owner's
  **favorites** may be read by `name_key` to resolve the typed text.
- **Constraint 2 (role semantics).** It reads: "`view` = read-only; `edit` =
  add / check / remove but not re-share and not delete the list; only the
  owner shares, revokes, deletes". `edit`'s "add" now includes this silent
  match. Every other read of the owner's catalog, and adding by product id,
  stay the owner's.

RR-0002's constraints 1 and 3–6 are unchanged.

Covers:

- a **follow-up implementation PR inside MB-0036**. PR #49, the owner-only
  part, is already open and is unaffected. The follow-up contains:
  - the name-based add at `Edit` (sub-answer 1);
  - `routes/lists/items.rs` calling it for editors. The owner path may use it
    too or keep `owner_favorite` → `add_product` (`may_decide`);
  - the `ShoppingListRepo` trait doc updated: the new method needs `edit`, and
    `add_product` stays "the owner's alone";
  - role tests (Constraints, rule 5);
  - MB-0036's P4 delta below.

Does not cover:

- suggestions of the owner's favorites to editors (Option C);
- editors adding by product id, `add_recipe` or `assign_category` at
  `edit`;
- matching an editor's own favorites or products, or any category other than
  the list owner's;
- matching non-favorite products, or any fuzzy matching;
- `view` grantees, re-sharing or list deletion;
- any schema change.

## Rationale and trade-offs

- **B, silent — chosen.** Editors' items land in the owner's categories
  without the owner sorting them later, and the owner's product and category
  never leave the owner's catalog. The only new disclosure is the per-name
  oracle above, bounded and visible on the list.
- **A (do nothing) — not chosen.** It was the CRP's recommendation and
  needs no authorization change. But editors' "maito" stays uncategorized
  even when the owner has the favorite "Maito", which is the friction
  MB-0036 set out to remove.
- **C (B plus suggestions) — not chosen.** It would expose the owner's whole
  favorites list to every editor, and it would need a `ProductRepo` read
  authorized through the list predicate, a new cross-module path.
- **Owner-only id-based add (Q3) — chosen.** Ids are guessable only as
  UUIDs, but an id path at `edit` would let an editor add any owner product
  regardless of favorite status. Name matching restricted to favorites is
  narrower.

## Approvals and authority

Approved by security-owner @juuso.kalliomaki. MB-0036's P4 delta is approved by owner
@juuso.kalliomaki. Risk tier medium.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0024-editor-favorite-matching.md` (branch
  `agentic/mb-0036-add-field`)
- Mission Brief: `.agentic/missions/MB-0036-one-add-field-with-favorite-suggestions.md`
- Continuity Pack: `.agentic/continuity/MB-0036.md`
- PR #49 (owner-only part): https://github.com/Jurzo/kauppa-rs/pull/49
- Amends: RR-0002 (scope, constraint 2). Related: RR-0003 (`name_key`),
  RR-0001 (items copy name and category at add time), RR-0009 constraint 3
  (bypass guard).
- Evidence links: CRP-0024 → Current state and Evidence bundle:
  - `repo/sqlite/list.rs` `add_product`: `begin_write(.., ListAccess::Owner)`,
    `WHERE p.id = ?4 AND p.owner_id = ?5` with `grant.owner_id()`;
  - `ITEM_SELECT` / `STORE_VIEW_SELECT` join categories on `l.owner_id`;
  - `repo/mod.rs` trait doc: `add_product` is "the owner's alone";
  - `sharing_roles::add_follows_roles` pins today's split, and the bypass
    guard is green.

## Constraints and follow-on rules (Hardened)

1. **Through the central predicate.** The editor match authorizes with
   `begin_write(.., ListAccess::Edit)` before any query, and lives in
   `repo/sqlite/list.rs`. The bypass guard must cover it.
2. **The list owner's favorites only.** Every product lookup for a list item
   is scoped to `grant.owner_id()`, never the acting user's id, and the editor
   path also requires `is_favorite = 1`. A lookup keyed on the acting
   editor's id is a review failure.
3. **No disclosure beyond the result.** No suggestions, counts, or
   different error messages for editors. A match and a non-match return the
   same response shape. Neither the typed text nor the matched product is
   logged at INFO or higher.
4. **Id paths stay owner-only.** `add_product`, `add_favorite`,
   `add_recipe` and `assign_category` stay at `ListAccess::Owner`.
5. **Tests.**
   - `sharing_roles::add_follows_roles` (P4) changes. An editor's "maito"
     adds the owner's "Maito" in Kylmä. The editor's own favorite "Omena"
     stays free-form and uncategorized. An owner product that isn't a
     favorite stays free-form. Editors get no datalist. A viewer's add is
     403.
   - `edit_role_cannot_administer` is unchanged.
   - A revoked editor's add is denied on the next request (RR-0002
     constraint 4).
6. **Wider access needs a new RR.** Suggestions for editors, or any other
   read of the owner's catalog at `edit`, needs a new consult and RR.

## Roll-back into the system (Hardened)

To be applied in MB-0036's follow-up PR:

- `.agentic/resolutions/RR-0002-list-sharing-authz.md`:
  - add after constraint 2: "Amended by RR-0024: `edit`'s add also matches
    typed text against the list owner's favorites by `name_key`, silently.
    Adding by product id and every other read of the owner's catalog stay
    owner-only."
  - add to Supersession and history: "Amended by RR-0024 (scope and
    constraint 2); not superseded."
- `.agentic/missions/MB-0036-one-add-field-with-favorite-suggestions.md`:
  - `resolutions` gains `RR-0024`;
  - P4's property becomes: "On a shared list, an `edit` grantee's typed name
    that matches the list owner's favorites (by `name_key`) gets the owner's
    product and category; editors are shown no suggestions; a `view` grantee
    still can't add anything (403) (RR-0024)." The check is unchanged.
  - Context: the CRP-0024 deferral sentence records RR-0024's answer.
- `CRP-0024`: `state: open` → `answered`.
- `.agentic/mentorship/security.md`: an `edit` grant may match the owner's favorites by name, RR-0024
- `.agentic/mentorship/persistence.md`: no change. The insert verifies
  ownership in the same statement (`p.owner_id = grant.owner_id()`), as its
  Queries rule requires.

## Supersession and history

Status: active once signed. Amends RR-0002 (scope, constraint 2) and
supersedes nothing.

Amended by RR-0036 (2026-11-14): `POST /lists/{id}/favorites`
(`add_favorite`) is removed, since no template posts to it (sub-answer 3).
`ShoppingListRepo::add_product` stays owner-only; the rest of sub-answer 3
stands.
