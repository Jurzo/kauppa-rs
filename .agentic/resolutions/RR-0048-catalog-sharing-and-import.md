# Resolution Record — Share favorites and recipes for import: one share per person, one predicate, imports as copies through MB-0060's review

```yaml
---
id: RR-0048
closes: CRP-0048
mission: MB-0061
decided_by: "@Jurzo"
role: security
risk_tier: medium
decided_at: 2026-11-15t15:14:00z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0061-share-favorites-and-recipes.md
  - .agentic/consults/CRP-0048-catalog-sharing-and-import.md
---
```

## Decision and outcome

Approve CRP-0048 as recommended: Q1–Q8 A.

1. **The model (Q1 A, owner).**
   - A user shares with another existing user by email, with two
     switches: favorites and recipes.
   - The recipient sees the sharer's current favorites and recipes of the
     kinds switched on, and imports copies of what they choose.
   - There is one share per pair of users. Editing it changes the
     switches. Turning both off, or revoking, deletes it.
   - Imported copies are never removed.
2. **The schema (Q2 A, data-owner).** Exactly one new migration,
   `migrations/0012_catalog_shares.sql`, containing:

   ```sql
   CREATE TABLE catalog_shares (
       id         TEXT PRIMARY KEY,
       owner_id   TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
       grantee_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
       favorites  INTEGER NOT NULL CHECK (favorites IN (0, 1)),
       recipes    INTEGER NOT NULL CHECK (recipes IN (0, 1)),
       created_at TEXT NOT NULL,
       UNIQUE (owner_id, grantee_id),
       CHECK (owner_id <> grantee_id)
   ) STRICT;
   CREATE INDEX idx_catalog_shares_grantee ON catalog_shares(grantee_id);

   ALTER TABLE recipes ADD COLUMN imported_from TEXT
       REFERENCES recipes(id) ON DELETE SET NULL;
   ```
3. **Access and disclosure (Q3 A, security).**
   - **One predicate.** `repo/sqlite/catalog_access.rs` holds the only query
     that decides whether a user may read another user's favorites or
     recipes: a `catalog_shares` row with that switch on.
     - It alone makes the grant value that every cross-user read of
       products, category names and recipes takes. As with `ListGrant`,
       its fields are private (RR-0036 constraint 2).
     - `catalog_shares` is named only in `catalog_access.rs` and the file
       with the share methods.
     - The bypass guard (`repo::share`'s tests) is widened to cover both.
       This RR is the record RR-0036 constraint 3 requires.
   - **404.** No share, a revoked share, or a switch that is off answers 404.
   - **The sharer's identity.** The recipient sees the sharer's display
     name only.
   - **The share form.**
     - It answers with the same notice whether or not the email matches
       (RR-0002 constraint 3).
     - It matches the email with `COLLATE NOCASE` to exactly one account
       other than the sharer (RR-0028).
     - The sharer's own list of shares shows the matched people's names
       and emails, as a list's sharing page does.
   - **No rate limit.**
   - **Logging.** Share, change and revoke are logged at INFO with ids,
     never emails.
4. **Favorites to import (Q4 A, owner).**
   - The shared favorites are grouped by the sharer's categories, with an
     unticked checkbox each.
   - "Import selected" and "Import all" both go through MB-0060's plan and
     review.
5. **Categories (Q5 A, owner).** As RR-0047 sub-answers 2 and 4:
   - each of the sharer's categories is matched by name, ignoring case, to
     the recipient's own;
   - with no match, the review asks for one of the recipient's categories,
     or to skip;
   - no category is created;
   - the sharer's category or product ids never enter the recipient's
     catalog (RR-0001 constraint 9).
6. **A recipe's ingredients (Q6 A, owner).**
   - **An existing product** of the recipient's, matched by name ignoring
     case, is linked as it is.
   - **If that product has no category,** the review asks for one (a
     picker, no skip; RR-0001 constraint 3 for ingredients).
   - **A new product** is created hidden ("From recipes") in the sharer's
     category mapped by name. Without a match, the review asks (a picker,
     no skip).
   - **The copy.** The recipe, with its title, instructions, servings, and
     ingredients with quantities and order, is created with its products in
     one transaction, with `imported_from` set.
7. **Importing again (Q7 A, owner).**
   - A recipient who already has a recipe whose `imported_from` is this
     recipe is refused, with a link to their copy.
   - After they delete the copy, importing is allowed again.
8. **Delivery (Q8 A, owner).** One mission, two implementation PRs:
   1. the share, the predicate, browsing and importing favorites (with the
      migration);
   2. importing recipes.

Covers:
- MB-0061's implementation as above, including generalizing MB-0060's
  plan input from list items to candidates (a name and a foreign category);
- the roll-back below.

Does not cover:
- per-item shares;
- public or account-less sharing;
- updating an earlier copy;
- creating categories on import;
- any change to list sharing.

## Rationale and trade-offs

- It mirrors list sharing: one revocable row, one predicate, the same
  notice and 404s. A new cross-user read path is reviewed once, in one
  place.
- Imports are copies decided in the existing review, so nothing of the
  sharer's is linked or written.
- Repeats are caught by origin (`imported_from`), since titles aren't
  unique.
- Rejected:
  - per-item shares (Q1 B, C);
  - a table per kind (Q2 B);
  - showing the sharer's email (Q3 B);
  - importing all only (Q4 B);
  - created or offered categories (Q5 B, C);
  - conflict review or favorites for ingredients (Q6 B, C);
  - second copies or updates (Q7 B, C);
  - a new brief for recipes (Q8 B).

## Approvals and authority

@Jurzo as security for sub-answer 3; as data-owner for sub-answer 2; as owner for sub-answers 1, 4, 5, 6, 7 and 8.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0048-catalog-sharing-and-import.md`
- Mission Brief: `.agentic/missions/MB-0061-share-favorites-and-recipes.md`
- Related: RR-0001, RR-0002, RR-0003, RR-0012, RR-0013, RR-0028, RR-0036,
  RR-0046, RR-0047.

## Constraints and follow-on rules (Hardened)

1. **One predicate.** A read of another user's products, categories or
   recipes outside `catalog_access.rs`'s grant is a review failure. The
   guard test enforces where `catalog_shares` is named and that cross-user
   reads take the grant.
2. **Own catalog only.** Imports write only the importing user's rows, and
   every category id they set is checked as the user's in the same
   transaction.
3. **Schema.** `0012_catalog_shares.sql` is the only migration this RR
   allows. It is append-only and additive.
4. **Tests.**
   - Route tests cover the sharer, the recipient, a stranger, a revoked
     share, and a switch that is off (404).
   - They cover the share form's same answer for a matched and an
     unmatched email.
   - They cover the cascades (deleting a user removes the share; deleting
     a recipe leaves imported copies, with `imported_from` set to NULL).
   - They cover a repeat import and a stale review.
5. **Reuse.** Imports use MB-0060's plan and review (RR-0047 constraint
   5). A change to the review's behaviour applies to both features and
   needs its own consult.

## Roll-back into the system (Hardened)

To be applied after this record is signed, with the owner's OK:

- `MB-0061`:
  - P1: one share per person with favorites and recipes switches;
  - P2: the predicate in `catalog_access.rs`, 404s, display name only;
  - P3: chosen or all favorites; categories as RR-0047;
  - P4: ingredients linked or created hidden, a category asked only when
    missing, `imported_from` set;
  - P5: a repeat refused with a link to the copy;
  - P6: the same notice; no rate limit;
  - P7: `0012_catalog_shares.sql` and its cascades;
  - the seven escalate entries marked decided (RR-0048);
  - `resolutions: [RR-0048]`;
  - plan step 1 marked done, with the two-PR split noted.
- `CRP-0048`: `state: open` → `answered`.

## Supersession and history

Status: active once signed. Supersedes nothing.
