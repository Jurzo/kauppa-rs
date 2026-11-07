# Mission Brief — Edit a list item's quantity

```yaml
---
id: MB-0021
owner: "@juuso-kalliomaki"
goal: Let the user fix or add a list item's quantity after adding it ("2 dl" → "3 dl", or "2 kpl" on an item that had none), from the list page, without removing and re-adding the item; the store view shows the new quantity.
non_goals:
  - Editing an item's name (text) or its product link.
  - Editing quantities in the store view (`/lists/{id}/shop`) — it stays a check-off view.
  - Arithmetic on quantities (merging "2 dl" + "1 dl", unit conversion).
  - Changing the recipe an item came from; recipe ingredients keep their own quantities (MB-0003).
acceptance:
  - id: P1
    property: "`ShoppingListRepo::set_item_quantity` sets an active item's quantity (trimmed) or clears it (`None` or blank), returns the updated item, and touches nothing else — not its text, category, product, checked state, or other items. A removed item, or an item of another list, is `NotFound`."
    check: cargo test -p kauppa-core repo::list::items::set_item_quantity
  - id: P2
    property: "On the list page each active item can have its quantity edited inline: `POST /lists/{id}/items/{item}/quantity` answers htmx with the re-rendered `#list-{id}` fragment and a plain request with a 303 back to the list. The new quantity shows on the list page and in the store view; an empty value clears it."
    check: cargo test -p kauppa-web routes::lists::tests::items::edit_quantity
  - id: P3
    property: "Access follows the list's roles through the central access predicate: the owner and an `edit` grantee can change a quantity; a `view` grantee gets 403 and a stranger 404, and the quantity is unchanged."
    check: cargo test -p kauppa-web routes::lists::tests::sharing_roles::quantity_follows_roles
  - id: P4
    property: "The new repo method routes through the central access predicate (RR-0002 constraint 1); the bypass guard checks it like every other `ShoppingListRepo` method."
    check: cargo test -p kauppa-core repo::share::list_queries_route_through_central_predicate
context:
  - crates/kauppa-core/src/repo/mod.rs
  - crates/kauppa-core/src/repo/sqlite/list.rs
  - crates/kauppa-core/src/repo/list/items.rs
  - crates/kauppa-core/src/repo/sqlite/recipe.rs
  - crates/kauppa-web/src/routes/lists/items.rs
  - crates/kauppa-web/src/routes/lists/mod.rs
  - crates/kauppa-web/src/routes/lists/tests/items.rs
  - crates/kauppa-web/src/routes/lists/tests/sharing_roles.rs
  - crates/kauppa-web/src/routes/recipes/ingredients.rs
  - templates/lists/_list.hbs
  - templates/recipes/_ingredients.hbs
  - .agentic/resolutions/RR-0002-list-sharing-authz.md
autonomy:
  may_decide:
    - where the quantity field sits in the item row (e.g. inside the existing "Edit" disclosure, beside the category picker)
    - the field's placeholder, label and length handling (free text, like ingredient quantities)
    - test placement within the named area files
  escalate:
    - { when: "the edit role should NOT be allowed to change quantities (the brief assumes it is, like check/remove)", to: owner }
    - { when: "any route besides POST /lists/{id}/items/{item}/quantity is added or removed", to: api-owner }
    - { when: "the method can't use `begin_write` with `ListAccess::Edit`, or the bypass guard needs changing", to: security }
    - { when: any schema change, to: data-owner }
  forbidden:
    - change the central access predicate or the bypass guard
    - "edit migrations (none needed: `list_items.quantity` exists since 0001)"
    - add dependencies or lower lint strictness
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/actix-web.md@v1
  - .agentic/mentorship/templating.md@v1
  - .agentic/mentorship/persistence.md@v1
  - .agentic/mentorship/security.md@v1
resolutions:
  - RR-0001
  - RR-0002
---
```

## Context

Requested by the owner (2026-11-07). Each list item has a free-text
`quantity`: the `list_items.quantity` column has existed since
`0001_init.sql`, and the domain type has `ListItem::quantity`. It is set in
three ways:

- when a free-text item is added with the optional quantity field;
- when a favorite is added;
- when a recipe is added, where MB-0005 scales it by the multiplier
  (`domain::quantity::scale`).

There is no way to change it afterwards. A typo, or a recipe added once when
two batches are needed, means removing and re-adding the item.

Recipes already have the pattern: `RecipeRepo::set_ingredient_quantity`
(sets or clears, trims, owner-scoped) and an inline quantity form per
ingredient in `templates/recipes/_ingredients.hbs`. List items differ in two
ways:

- they can be shared (MB-0008), so the write goes through the list repo's
  central access predicate (`begin_write`), not an owner check;
- items are soft-deleted, so a removed item must not be editable.

**Roles (assumed, for the owner to confirm at acceptance).** An `edit`
grantee can add free-text items, check, remove and restore (MB-0008), so
changing a quantity fits that role. A `view` grantee can't. Category
assignment stays owner-only, because it reads the owner's catalog; a quantity
doesn't.

## Conceptual plan

- **Repo (P1, P4).** `ShoppingListRepo::set_item_quantity(user, list, item,
  quantity)`:
  - `begin_write(user, list, ListAccess::Edit)`;
  - one `UPDATE list_items SET quantity = ? WHERE id = ? AND list_id = ? AND
    removed_at IS NULL`, with zero rows meaning `NotFound`;
  - `touch_list` so the list rises in the index;
  - return the item through `fetch_item`.

  It lives in `repo/sqlite/list.rs`, the only file allowed to name the list
  tables (RR-0009 constraint 3). Blank-to-`None` trimming can reuse the
  shared `clean_quantity`.
- **Web (P2, P3).** One handler in `routes/lists/items.rs`, in the style of
  `assign_category`: parse the form, call the repo, then `respond_item` (the
  fragment for htmx, 303 otherwise).
  - The item row gets a small form (text input plus Save), probably inside the
    existing per-item "Edit" disclosure.
  - It's shown to the owner and to `edit` grantees, using the same access
    flags as the other item controls.
  - The store view needs no change, since it already renders
    `item.quantity`.

Checkpoints: P1 and P4 green at the repo layer before the web slice.

Slow-mode triggers:
- the bypass guard fails on the new method; fix the method, never the guard;
- the item row's "Edit" disclosure becomes crowded on a 320 px screen
  (`templating.md` → Accessibility and mobile).

## Validation plan

- **P1:** `kauppa-core` test in `repo/list/items.rs`. It sets, trims, clears
  (with `None` and with blank), checks that other fields and items are
  unchanged, and that removed and foreign items give `NotFound`.
- **P2:** `kauppa-web` test in `routes/lists/tests/items.rs`. It covers htmx
  (fragment `#list-{id}` with the new quantity) and plain requests (303,
  then the full page shows it), the store view showing it, and clearing.
- **P3:** `kauppa-web` test in `routes/lists/tests/sharing_roles.rs`, with
  the owner, an `edit` grantee, a `view` grantee (403, unchanged) and a
  stranger (404, unchanged).
- **P4:** the existing bypass guard. It counts the trait's methods, so a new
  method that skips the predicate fails it.
- **Smoke:** curl on the built binary. Add a recipe ×1, change one item's
  quantity, and see it on the list page and in the store view.
