# Consultation Request Pack — One Save for a list item's category and quantity

```yaml
---
id: CRP-0033
mission: MB-0052
owner: "@juuso-kalliomaki"
target_role: security
risk_tier: medium
urgency: medium
state: answered
runbook_trigger: "feature.md security trigger: shared-list authorization in `crates/kauppa-web/src/routes/lists/` and `crates/kauppa-core/src/repo/sqlite/list.rs` (RR-0002, RR-0027); MB-0052 autonomy.escalate (route, atomicity, grantee category → security; untouched fields → owner)"
---
```

## Decision statement

Decide how one Save button writes an item's category and quantity together:
which route, whether it is one transaction, what a grantee's request that
carries a category gets, and what Save does with a field the user left alone.

Terms used below:

- **Owner**: the user who owns the list. **`edit` grantee** (editor) and
  **`view` grantee** (viewer): users the list is shared with (RR-0002).
- **Access predicate**: the one query in `repo/sqlite/list.rs` that decides
  what a user may do with a list. `begin_write(user, list, required)` opens
  a write transaction and runs it; too weak a role is `Forbidden` (403), no
  access at all is `NotFound` (404).
- **Picker**: the category `<select>` (`templates/products/_category_picker.hbs`).

## Current state

The edit section (`templates/lists/_list.hbs:57-84`) has three forms:

- Category (owner, when the owner has categories): picker and **Set**,
  to `POST /lists/{id}/items/{item}/category` (`_list.hbs:61-67`).
- Quantity (owner and editor): text field and **Save**, to
  `POST /lists/{id}/items/{item}/quantity` (`_list.hbs:72-79`).
- Remove (`_list.hbs:80-83`). Not part of this change.

Handlers (`crates/kauppa-web/src/routes/lists/items.rs`):

- `assign_category` (`:192-210`) parses `category_id` first
  (`parse_category`, `routes/products/mod.rs:67-73`: blank is "choose a
  category", a non-UUID is "unknown category"), then calls the repo.
- `set_item_quantity` (`:214-234`) passes the text through. Blank or
  missing clears the quantity (`QuantityForm`, `:46-51`).
- Both answer through `respond_item` (`routes/lists/mod.rs:274-301`):
  htmx gets the `#list-{id}` fragment (a rejection too, with 200 and the
  message at the top of the list); a plain request gets a 303, or the full
  page with 400 for a rejection. `Forbidden` and `NotFound` are not
  rejections: they become a bare 403 or 404 (`error.rs:30-32`).

Repo (`crates/kauppa-core/src/repo/sqlite/list.rs`):

- `assign_category` (`:551-592`): `begin_write(.., ListAccess::Owner)`.
  The category must be the list owner's, checked in the UPDATE itself:
  `AND EXISTS (SELECT 1 FROM categories WHERE id = ?1 AND owner_id = ?2)`
  with `grant.owner_id()` (`:575-583`). No row updated is
  `Invalid(Unknown(Category))`. It does **not** check `removed_at`, so it
  also works on a removed item.
- `set_item_quantity` (`:594-619`): `begin_write(.., ListAccess::Edit)`,
  `AND removed_at IS NULL`; a removed item is `NotFound`.
- Both call `touch_list` first, inside the transaction. An error returns
  before `commit`, so nothing is written.

What a grantee gets today for a category request:

- Editor or viewer with a well-formed id: 403, plain and htmx, nothing
  changed (`sharing_roles::view_role_is_read_only` `:92-105`,
  `edit_role_cannot_administer` `:200-217`, both through `list_posts`'
  `"category"` entry, `tests/mod.rs:150-154`).
- With a blank or malformed id the handler's parse fails before the repo is
  asked, so the answer is the rejection message (400 or the fragment), also
  with nothing changed.
- RR-0024 constraint 4: "`add_product`, `add_favorite`, `add_recipe` and
  `assign_category` stay at `ListAccess::Owner`."

Guards:

- `repo::share::list_queries_route_through_central_predicate`
  (`repo/share.rs:664-758`) checks every trait method: a gate
  (`self.begin_write(` …) before any `sqlx::query`, and it counts the
  trait's methods, so a new method is picked up without editing the guard.
- `repo::share::every_method` (`share.rs:136-247`) feeds the stranger and
  role-matrix tests. It has 18 calls (`READS.len() + 13`, `:296`).
  `set_item_quantity` is not among them; its roles are tested in
  `repo::list::items::set_item_quantity` and
  `sharing_roles::quantity_follows_roles`.
- `csrf::tests::coverage::every_write_route_is_covered` finds routes from
  the handler attributes and `routes::protected`, so a new `#[post]` is
  covered once registered. There are 40 write routes; the floor is 34
  (`csrf/tests/mod.rs:166-171`). `World::uri` already fills `{id}` and
  `{item}`, and `World::form` already sends `quantity` and `category_id`
  (`:275-327`).

Size: `repo/sqlite/list.rs` is 801 lines. RR-0009's guideline is about 800
(not enforced in CI). MB-0048 plans to split it; that brief is not accepted
yet.

## Options and trade-offs

### Q1 — Route (security)

**A (recommended): one new route, `POST /lists/{id}/items/{item}`, and
remove `/category` and `/quantity`.**

- Pros:
  - One write path for the two fields, so one place to authorize and test.
  - The path reads as "update this item". It does not collide with
    `POST /lists/{id}/items` (add) or `/items/{item}/remove|restore`.
  - The cross-site check covers it with no test edit (40 → 39 routes).
- Cons:
  - A list page loaded before the deploy still posts to the old routes and
    gets a 404 until it is reloaded.
  - Tests that post to the old routes change (list under Q6). The brief's
    P4 allows exactly that.
- Risk: low. Removing routes removes surface.
- Cost: small.

**B: add the new route and keep both old ones.**

- Pros: stale pages keep working. Precedent: `add_favorite` stayed "for
  pages loaded before that change" (`items.rs:136-139`). No old test
  changes.
- Cons: three routes write the same two columns, and all three must stay
  right. The old ones have no form and would only be reached by stale pages
  or hand-made requests.
- Risk: low to medium (more surface on a trigger path).
- Cost: smallest now, cleanup later.

**C: keep `/quantity` and let it take `category_id` too.**

- Cons: the name no longer says what it does; a quantity route that needs
  owner rights for some bodies is a trap for the next reader.
- Not recommended.

### Q2 — Atomicity (security)

**A (recommended): one new repo method, one transaction.**

```rust
/// Save an active item's quantity and, when `category_id` is `Some`, its
/// category. `edit` for the quantity alone; owner only with a category.
async fn update_item(
    &self, user_id: Uuid, list_id: Uuid, item_id: Uuid,
    category_id: Option<Uuid>, quantity: Option<&str>,
) -> CoreResult<ListItem>;
```

- It opens `begin_write(user, list, required)` where `required` is
  `ListAccess::Owner` when `category_id` is `Some`, else `ListAccess::Edit`.
  Authorization comes before every query, as the bypass guard demands.
- The quantity is set as `set_item_quantity` does (`None` or blank clears).
- The category check is today's: the UPDATE only matches when the category
  belongs to `grant.owner_id()` (`persistence.md` → Queries: ownership in
  the same statement or transaction). Not the owner's → `Invalid(Unknown
  (Category))`.
- Active items only (`removed_at IS NULL`); a removed, foreign or unknown
  item is `NotFound`. This is `set_item_quantity`'s rule. It is stricter
  than `assign_category`, which accepts a removed item today; the page
  never offers that.
- Any error returns before `commit`, so neither field, nor the list's
  `updated_at`, changes. That is P2.
- Pros: P2 holds by construction; one write lock; the role rule sits in the
  repo, not the handler.
- Cons: about 45 more lines in `list.rs` (see Q2b).
- Risk: low. Cost: small.

**B: the handler calls `assign_category`, then `set_item_quantity`.**

- Pros: no new repo code.
- Cons: two transactions. If the second fails (the item is removed, or the
  grant revoked, in between) the first has already been saved. The handler
  would also have to pick the calls by role, which puts an authorization
  choice in the handler. The list is touched twice.
- Risk: medium (P2 depends on call order and timing). Not recommended.

### Q2b — The old repo methods and the file's size (security)

After Q1-A no handler calls `assign_category` or `set_item_quantity`. Tests
still do: about 20 calls in `repo/list/items.rs`, `repo/list/store.rs`,
`repo/share.rs`, and as setup in `routes/lists/tests/{items,shop}.rs`.

**A (recommended): keep both methods in this mission.**

- Pros: no test outside the changed routes is edited, which is what P4
  says. RR-0024 constraint 4 names `assign_category`; it stays true as
  written. The diff on the trigger path is one added method.
- Cons: two trait methods with no production caller. `list.rs` grows to
  about 850 lines, over the guideline, until MB-0048 splits it.
- Follow-up: MB-0048 decides whether they go, when it splits the file. I
  record that in MB-0052's Continuity Pack.

**B: remove both now and move their tests to `update_item`.**

- Pros: `list.rs` ends near 780 lines; no unused methods.
- Cons: edits about 20 call sites in tests that this mission does not
  otherwise touch, so P4's wording needs an amendment; a larger diff in
  security tests (`repo/share.rs`).

Either way the implementation adds no helper function that names a list
table. The guard's allow-list (`share.rs:668-676`) stays as it is.

### Q3 — A grantee's request that carries a category (security)

The page never gives a grantee the picker, so this is only a hand-made
request.

**A (recommended): refuse with 403; change nothing, the quantity
included.**

- Follows from Q2-A: a category needs `Owner`, so `begin_write` answers
  `Forbidden` before any write and before the category is looked up.
- Pros:
  - Same answer as today's `/category` and as every other owner-only write
    (RR-0002 constraint 2, RR-0024 constraint 4).
  - No oracle: the refusal comes before the lookup, so an editor can't
    learn whether an id is one of the owner's categories.
  - A request that asks for something it may not do fails as a whole. It is
    not half-applied.
- Cons: a hand-made request loses its quantity change too. No real form
  sends it.
- A viewer gets 403 with or without a category (the quantity needs `edit`).
  A stranger gets 404.
- Blank `category_id` counts as "no category sent" for everyone (Q4), so it
  is not refused. A malformed id (not a UUID) is the "unknown category"
  rejection for everyone, as the parse runs before the repo, like today;
  nothing changes in either case.

**B: ignore the field silently and save the quantity.**

- Pros: lenient.
- Cons: answers success to a request that was partly dropped. The handler
  or repo needs a "strip the category for non-owners" branch, which is one
  more place where the owner-only rule could be lost. RR-0024's "silently"
  was about not revealing a favorite match; here there is nothing to hide,
  since an editor knows they are not the owner.

### Q4 — A field the user didn't touch (owner)

The browser sends every field of the one form on each Save.

- **Category, item already categorized.** The picker shows the current
  category, so Save sends it again. Writing the same value is harmless.
- **Category, item uncategorized.** The picker starts on "Choose a
  category". That option is `disabled`, and browsers don't send a disabled
  option, so no `category_id` arrives. Today the picker is also `required`
  (`_category_picker.hbs:1`), which in a combined form would stop Save
  until a category is chosen.

  **A (recommended): not required in this form.** No (or blank)
  `category_id` means "leave the category as it is"; the quantity is saved.
  - Pros: the owner can still set a quantity on an uncategorized item, as
    today. RR-0001 constraint 5: uncategorized is a real state for list
    items; only favorites must have a category.
  - Cons: Save on "Choose a category" gives no reminder. The summary keeps
    saying "Choose category", so the cue stays.
  - The picker stays `required` on the favorites page and the recipe
    editor; the partial gets an option for this form.
  - A categorized item can't be made uncategorized again, as today (the
    placeholder is disabled). Not in scope.

  **B: required.** Save is blocked until the owner picks a category.
  - Cons: takes away "quantity now, category later" for the owner, while an
    editor can still do it. Not recommended.
- **Quantity.** The field holds the current value, so an untouched quantity
  is sent back unchanged. Blank clears it, as today. A request with no
  `quantity` field at all also clears it, as today (`items.rs:46-51`).

### Q5 — Details inside the brief (may_decide; say if you want otherwise)

- One form: category picker (owner with categories), then quantity, then
  **Save** (`common.save`) last. Remove stays its own form below. The "create
  a category" hint for an owner without categories stays above the form.
- Summary text unchanged: "Choose category" for the owner's uncategorized
  items, else "Edit" (`items::uncategorized_summary_cue` pins it, MB-0037).
- `common.set` ("Set" / "Aseta") is used only at `_list.hbs:66`. Remove it
  from both catalogs.
- After a rejected Save the list shows the message at the top, the section
  is closed, and the typed quantity is not kept. Today's item forms behave
  the same; drafts exist only for the add forms.

### Q6 — Tests

New, in `routes::lists::tests::item_edit` (new file `tests/item_edit.rs`):

- `one_save_button` (P1): owner with categories, owner without, editor,
  viewer. Per edit section: one form besides Remove, one submit labelled
  Save, no "Set", no `/category` or `/quantity` action; the picker only for
  the owner; no section for the viewer.
- `saves_category_and_quantity_together` (P2): one request stores both,
  plain (303) and htmx (fragment). An unknown category, another user's
  category and a malformed id each change neither field and show the
  message (400 page, or the fragment). No category on an uncategorized item
  saves the quantity only. Blank quantity clears. A removed item is 404.
- `grantee_saves_quantity_only` (P3): editor without a category → saved;
  editor with the owner's category id → 403, category and quantity
  unchanged; with their own category id → 403; viewer → 403; stranger →
  404; plain and htmx.

New in core (`kauppa-core`):

- `repo::list::items::update_item`: both fields in one call; rollback on a
  foreign category; the same-owner rule; removed and foreign items.
- `repo::share::every_method` gains two calls, `update_item` with and
  without a category. The count goes from 13 to 15; the one without joins
  `ITEM_WRITES`; the one with is owner-only. This also puts a quantity
  write in the stranger and role-matrix tests, which lack one today.

Existing tests that change, all because they post to a removed route or
assert its form:

- `lists/tests/items.rs`: `owner_isolation` (`:67`), `htmx_fragments`
  (`:180`, `:218`, `:254`; the blank-category case becomes "saves the
  quantity only" or moves to `item_edit`), `edit_quantity` (`:323`,
  `:351-353`).
- `lists/tests/mod.rs:150-154` (`list_posts`' `"category"` entry), used by
  `sharing_roles::view_role_is_read_only` and
  `edit_role_cannot_administer`; `sharing_roles.rs:152` (the "no
  `/category`" control check) and `quantity_follows_roles` (`:542`).
- Nothing in `csrf/tests/` (routes are discovered).

With Q2b-A no other test changes.

## Recommendation and question

Recommended: Q1-A, Q2-A, Q2b-A, Q3-A, Q4-A, and Q5/Q6 as listed.

Because: one route and one transaction give one place where the
owner-only rule for categories is decided, in the repo and behind the
access predicate. A refusal changes nothing. The owner keeps everything
they can do today in one submission.

Crisp questions (each can be answered "as recommended"):

1. **Route.** Add `POST /lists/{id}/items/{item}` and remove
   `POST …/category` and `POST …/quantity`, accepting that a page loaded
   before the deploy must be reloaded? (Alternative: keep the old routes
   too.)
2. **Atomicity.** One new `ShoppingListRepo::update_item(user, list, item,
   category: Option<Uuid>, quantity: Option<&str>)` in one `begin_write`
   transaction: `Owner` when a category is sent, `Edit` otherwise; the
   category must be the list owner's, checked in the same statement or
   transaction; active items only; any error changes nothing?
3. **Old repo methods and size.** Keep `assign_category` and
   `set_item_quantity` in the trait for now (tests use them), accept
   `list.rs` at about 850 lines until MB-0048 splits it, and leave their
   removal to MB-0048? (Alternative: remove them now and amend P4.)
4. **Grantee with a category.** Refuse with 403 and change nothing, the
   quantity included? Blank `category_id` counts as not sent; a malformed
   id is the usual "unknown category" rejection.
5. **Untouched fields (owner).** In this form the picker is not required:
   no category chosen means the category stays as it is and the quantity is
   saved. Blank quantity clears it. A categorized item still can't be made
   uncategorized.
6. **Details.** Summary text unchanged; `common.set` removed from both
   catalogs; Save last in the form, Remove below it; the test plan in Q6?

## Blast radius and rollback

What can break:

- A mistake in `update_item`'s required access would let an editor change a
  category. Guarded by `item_edit::grantee_saves_quantity_only`, the
  role-matrix entry, and the bypass guard.
- A mistake in the category check would let an owner point an item at
  another user's category. `ITEM_SELECT` joins categories on the list
  owner, so such an item would show as uncategorized, not leak a name;
  the P2 and core tests cover it.
- Pages open during the deploy: Set and Save answer 404 until reload
  (Q1-A).

Who is affected: owners and editors of lists, on the list page only. The
store view, favorites and recipes are untouched. No schema change, no new
dependency, no JavaScript.

Rollback plan: revert the PR. No data migration either way; items written
through the new route are ordinary rows.

## Minimal context

- MB-0052 P1–P5; `forbidden`: an editor changing a category, a list query
  outside the access predicate, JavaScript.
- RR-0002 constraints 1, 2, 4; RR-0024 constraint 4; RR-0001 constraints 5
  and 9; RR-0009 constraints 3 and 4; `security.md` → Authorization;
  `persistence.md` → Queries.
- Files the implementation would touch: `templates/lists/_list.hbs`,
  `templates/products/_category_picker.hbs`, `locales/{en,fi}.json`,
  `routes/lists/items.rs`, `routes/lists/mod.rs` (re-exports),
  `routes/mod.rs` (registration), `repo/mod.rs` (trait and its
  access table in the doc comment), `repo/sqlite/list.rs`, and the tests in
  Q6.
- Suggested roll-backs for the RR: MB-0052 `resolutions` gains the RR and
  P3's "the RR decides" becomes the answer to question 4; RR-0002
  constraint 2 gains a line that `update_item` is `edit` for the quantity
  and owner-only with a category. `security.md` needs no change.

## Evidence bundle (Hardened)

All on `agentic/mb-0052-one-save` at `0d08fc3` (2026-11-12).

- `templates/lists/_list.hbs:57-84`: the three forms; `:58` the summary;
  `:66` the only use of `common.set` (`grep -rn "common.set" templates
  crates` → that line only).
- `templates/products/_category_picker.hbs:1-2`: `required`, and the
  placeholder option `disabled`.
- `routes/lists/items.rs:192-234`: both handlers; `:46-57` the forms.
- `routes/lists/mod.rs:274-301`: `respond_item`. `error.rs:30-34`:
  `NotFound` 404, `Forbidden` 403, `Invalid` 400.
- `routes/mod.rs:63-64`: the two registrations.
- `repo/sqlite/list.rs:238-247` `begin_write` (`BEGIN IMMEDIATE`, then
  `authorize`); `:102-123` `authorize`; `:551-592` `assign_category`;
  `:594-619` `set_item_quantity`; `:129-140` `ITEM_SELECT`'s owner joins.
- `repo/mod.rs:229-237`: the trait's access table; `:325-345` the two
  methods' contracts.
- `repo/share.rs:664-758`: the bypass guard; `:136-258` `every_method`,
  `READS`, `ITEM_WRITES`; `:292-298` the count.
- `routes/lists/tests/sharing_roles.rs:92-105`, `:200-217`: a grantee's
  category post is 403 today, plain and htmx; `:522-633`
  `quantity_follows_roles`.
- `csrf/tests/mod.rs:161-173`, `:275-327`; `csrf/tests/coverage.rs:115-166`.
- `grep -rhn '#\[\(post\|put\|delete\|patch\)(' crates/kauppa-web/src | wc -l`
  → 40.
- `wc -l crates/kauppa-core/src/repo/sqlite/list.rs` → 801.
- Nothing was run against a browser; P5 is the owner's check after the
  implementation.
