# Consultation Request Pack — Archived lists: the schema, what an archived list allows, and what grantees see

```yaml
---
id: CRP-0045
mission: MB-0062
owner: "@juuso-kalliomaki"
target_role: data-owner
risk_tier: medium
urgency: low
state: open
runbook_trigger: "MB-0062 autonomy.escalate (schema → data-owner; archived list pages, grantees, order → owner; the access predicate and owner-only routes → security); feature.md escalation triggers `migrations/`, `crates/kauppa-web/src/routes/lists/`, `crates/kauppa-core/src/repo/sqlite/list.rs`, `crates/kauppa-core/src/repo/sqlite/list_access.rs` (RR-0027)"
---
```

## Decision statement

Decide how an archived state is stored, what an archived list allows its
owner and grantees, and where the rule is enforced, so that the owner can
archive and restore lists and `/lists` shows archived ones in a section
collapsed by default.

## Options and trade-offs

### Q1 — Schema (data-owner)

#### Option A — `shopping_lists.archived_at TEXT NULL` (recommended)

Migration `0011_list_archive.sql`:
`ALTER TABLE shopping_lists ADD COLUMN archived_at TEXT;` (RFC 3339 like
the other timestamps). NULL means active; existing lists start active.
Archiving and restoring don't touch `updated_at`, so a restored list
returns to its usual place in the order.

- Pros: one nullable column; the timestamp shows "archived on" and orders
  the Archived section; no backfill.
- Risk: none for existing rows (append-only, nullable).

#### Option B — `archived INTEGER NOT NULL DEFAULT 0`

- Cons: no date to show or order by.

### Q2 — What an archived list allows (owner)

#### Option A — Read-only, with Restore (recommended)

The list page and the shopping view still open (from the Archived section
or a bookmark) and show a notice "Archived on … — Restore". Items can't be
added, edited, checked, removed or restored, and recipes can't be added;
those controls aren't rendered. The owner can still rename it, delete it,
and manage its sharing.

- Pros: archived means finished; nothing changes by accident in a shop;
  one click brings it back.
- Cons: to use it again, restore first.

#### Option B — Fully usable; archived only moves it on `/lists`

- Pros: simplest.
- Cons: "archived" lists can still change; a grantee's ticks land in a
  list the owner thinks is put away.

### Q3 — Grantees (owner)

#### Option A — Grantees see it in their own "Archived" section (recommended)

"Shared with you" leaves it out; a collapsed "Archived" section on the
grantee's `/lists` shows it, read-only as Q2 decides. Only the owner can
archive or restore. Shares stay as they are.

- Pros: nothing disappears without trace; consistent with the owner's view.

#### Option B — Hidden from grantees while archived

The list leaves the grantee's `/lists`; opening it directly answers like
a list without access (404).

- Cons: a grantee loses a list with no explanation; access and visibility
  would then differ.

### Q4 — Enforcement (security)

#### Option A — In the repository's write path (recommended)

The access predicate (`ACCESS_SELECT`, `list_access.rs`) also returns
`archived_at`. `begin_write`, which every list write already calls with
the access it needs, refuses item and recipe writes on an archived list
with a new `Conflict` kind (409, the translated error page, RR-0038), so no
route can forget it. Archive and restore are owner-level writes through
the same call. Reads are unchanged.

- Pros: one place; matches RR-0002's single predicate.
- Risk: a write that should still be allowed (rename, delete, sharing) has
  to be listed; tests cover each.

#### Option B — In each route

- Cons: every current and future route must remember the check.

### Q5 — Order (owner)

Archived section: most recently archived first (recommended).

## Recommendation and question

Recommended: Q1 A, Q2 A, Q3 A, Q4 A, Q5 as above.
Because: one nullable column; archived means read-only and restorable;
grantees keep sight of it; the rule lives with the access predicate.

Crisp questions to answer:
1. (data-owner) `archived_at TEXT NULL`, without bumping `updated_at`?
2. (owner) Archived lists read-only with Restore (A), or fully usable (B)?
   Rename, delete and sharing still allowed while archived?
3. (owner) Grantees see archived lists in their own Archived section (A),
   or not at all (B)?
4. (security) Enforce read-only in `begin_write` with a new `Conflict`
   kind (A)?
5. (owner) Most recently archived first?

## Blast radius and rollback

What can break: every list write (Q4 changes the shared write path); the
lists page.
Who is affected: all users with lists, owners and grantees.
Rollback plan: revert the implementation PR; the migration's column is
nullable and ignored by the old code, so a rolled-back binary runs on the
migrated database (archived lists would show as active again).

## Minimal context

- `shopping_lists (id, owner_id, name, created_at, updated_at)` (0001);
  `list_shares` (0007).
- `list_access.rs`: `ACCESS_SELECT` is the only query that decides access;
  lists are ordered by `updated_at DESC`. `ShoppingListRepo` writes go
  through `begin_write(user, list, required ListAccess)` (for example
  `delete` requires `Owner`).
- `CoreError` has `NotFound`, `Conflict(kind)`, `Invalid`, `Unauthorized`,
  `Forbidden`; RR-0038 renders each as a translated page.

## Evidence bundle (Hardened)

- `crates/kauppa-core/src/repo/sqlite/list_access.rs` (`ACCESS_SELECT`,
  `accessible_lists`).
- `crates/kauppa-core/src/repo/sqlite/list.rs` (`begin_write` callers:
  every item, recipe, rename, delete and share method).
- `migrations/` ends at `0010_email_nocase.sql`.
