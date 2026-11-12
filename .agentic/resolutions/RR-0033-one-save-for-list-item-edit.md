# Resolution Record — One Save for a list item: `POST /lists/{id}/items/{item}` and `update_item` in one transaction

```yaml
---
id: RR-0033
closes: CRP-0033
mission: MB-0052
decided_by: "@Jurzo"
role: security
risk_tier: medium
decided_at: 2026-11-12T16:51:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/resolutions/RR-0002-list-sharing-authz.md
  - .agentic/missions/MB-0052-one-save-for-list-item-edit.md
  - .agentic/consults/CRP-0033-one-save-for-list-item-edit.md
---
```

## Decision and outcome

Approve CRP-0033's **recommended set, questions 1–6**.

Sub-answers:

1. **Route (Q1, security).** One new `POST /lists/{id}/items/{item}`
   (form fields `category_id`, optional, and `quantity`), registered in
   `routes::protected` behind the cross-site check.
   `POST /lists/{id}/items/{item}/category` and `…/quantity` are removed.
   A page loaded before the deploy must be reloaded; its old item forms
   answer 404 until then. Accepted.
2. **One transaction (Q2, security).** A new
   `ShoppingListRepo::update_item(user, list, item, category: Option<Uuid>,
   quantity: Option<&str>)`:
   - opens `begin_write` at `ListAccess::Owner` when a category is sent, at
     `ListAccess::Edit` otherwise;
   - checks that the category is the list owner's in the same statement or
     transaction, as `assign_category` does today;
   - updates active items only (`removed_at IS NULL`); a removed or
     unknown item is `NotFound`;
   - commits both fields or neither: any error changes nothing.
3. **Old repo methods (Q3, security).** `assign_category` and
   `set_item_quantity` stay in the trait for now (tests use them); their
   removal, and the split of `repo/sqlite/list.rs` (about 850 lines after
   this), are left to MB-0048. No production code calls them after this
   mission.
4. **A grantee sending a category (Q4, security).** Refused with 403;
   nothing changes, the quantity included. A blank `category_id` counts as
   not sent. A malformed id gets the usual "unknown category" rejection
   (parsed before the access check, as today).
5. **Untouched fields (Q5, owner).** In the combined form the category
   picker is not `required`: choosing no category leaves the item's
   category as it is and saves the quantity. A blank quantity clears it. A
   categorized item can't be made uncategorized. The shared picker partial
   stays `required` on the favorites and recipe pages.
6. **Details (Q6, owner).** The summary text is unchanged ("Edit" /
   "Choose category", pinned by MB-0037's test). The catalog key
   `common.set` is removed from both catalogs. Save is the last control of
   the form; Remove stays below it. Tests as CRP-0033 Q6 lists them,
   including two `update_item` entries in `repo::share::every_method`
   (13 → 15).

Covers:

- MB-0052's implementation PR, P1–P5;
- the new route and the removal of the two old ones (`routes/lists/`,
  trigger path);
- `update_item` in `repo/mod.rs` and `repo/sqlite/list.rs` (trigger path),
  behind the central access predicate;
- the template, catalog and test changes CRP-0033 lists;
- the roll-backs below.

Does not cover:

- giving `edit` grantees any category write;
- removing `assign_category` or `set_item_quantity` from the trait
  (MB-0048);
- the same change for favorites or recipe ingredients;
- keeping a rejected Save's typed values (today's behaviour stays).

## Rationale and trade-offs

- **One route, old ones removed.** Three write paths to the same two
  columns is more surface on a trigger path; a stale page's 404 lasts until
  a reload.
- **Owner access when a category is sent.** The access check runs before
  the category lookup, so an editor can't probe which category ids the
  owner has (RR-0024 constraint 4).
- **One transaction.** A Save that half-applies would show a quantity
  change with an error message, which reads as a failure that partly
  succeeded.
- **Keeping the old repo methods.** Removing them now edits about 20 test
  call sites outside the changed routes; MB-0048 already plans the list
  repo's split and cleanup.
- **Picker not required here.** Requiring it would stop the owner saving a
  quantity on an uncategorized item, which works today.

## Approvals and authority

Approved by security-owner (Q1–Q4, the trigger paths `routes/lists/` and
`repo/sqlite/list.rs`) and the owner (Q5, Q6).

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0033-one-save-for-list-item-edit.md`
- Mission Brief: `.agentic/missions/MB-0052-one-save-for-list-item-edit.md`
- Related: RR-0002 (shared-list access, amended here), RR-0024 (editors and
  the owner's catalog), RR-0001 (items may be uncategorized), RR-0009 (file
  size, test layout), RR-0027 (trigger paths).
- Evidence: CRP-0033 → Current state and Evidence bundle.

## Constraints and follow-on rules (Hardened)

1. **Categories stay the owner's.** No path lets an `edit` grantee set an
   item's category; `update_item` requires `Owner` whenever a category is
   sent.
2. **All or nothing.** `update_item` commits both fields or neither.
3. **Behind the predicate.** `update_item` opens `begin_write` before any
   query and names the list tables only inside the `ShoppingListRepo`
   implementation; the bypass guard and the role matrix cover it.
4. **The old routes stay removed.** Restoring a separate category or
   quantity route needs an RR.

## Roll-back into the system (Hardened)

To be applied in MB-0052's implementation PR:

- `RR-0002` → constraint 2 (role semantics): add "Saving a list item
  (`update_item`, MB-0052, RR-0033) is `edit` for the quantity and
  owner-only when a category is sent; a grantee's request with a category
  is refused." And under "Supersession and history": "Amended by RR-0033
  (2026-11-12): constraint 2 names `update_item`."
- `MB-0052`: `resolutions: []` → `[RR-0033]`; P3's "the RR decides whether
  it is refused or ignored" becomes "it is refused with 403 and nothing
  changes (RR-0033)"; Conceptual plan step 1 gains "Answered by RR-0033."
- `CRP-0033`: `state: open` → `answered`.

No Mentorship Pack or Runbook change.

## Supersession and history

Status: active once approved. Amends RR-0002 constraint 2.
