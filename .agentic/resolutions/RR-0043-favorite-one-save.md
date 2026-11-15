# Resolution Record — One Save for a favorite, with an explicit "No category" choice

```yaml
---
id: RR-0043
closes: CRP-0043
mission: MB-0058
decided_by: "<resolver's GitHub handle, e.g. @Jurzo>"
role: product
risk_tier: low
decided_at: <RFC 3339 UTC time of signing, e.g. 2026-11-15T00:00:00Z>
status: <approved | rejected>
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0058-favorite-one-save.md
  - .agentic/consults/CRP-0043-favorite-one-save.md
---
```

## Decision and outcome

Approve CRP-0043 option **C**.

1. A product's edit section on `/favorites` (favorites and the "From
   recipes" products) has one form: the name, the category picker and one
   **Save**.
2. In that form, the picker lists the user's categories and a **"No
   category"** choice. It opens with the product's current category
   selected, or "No category" when it has none, so Save always sends a
   choice and never needs a "Choose a category" placeholder.
3. Saving with "No category" clears the product's category. A favorite
   then shows under Uncategorized; a recipe product stays uncategorized
   (it can still be renamed without picking one).
4. Name and category are saved in one transaction; if either is rejected,
   neither changes (MB-0058 P2).

Covers: MB-0058's implementation and the roll-back below.

Does not cover:

- the add form on `/favorites` (a new favorite still needs a category);
- list items' or recipe ingredients' pickers (MB-0052 and others keep
  theirs);
- making "Make favorite" work for an uncategorized recipe product (today it
  requires a category; unchanged).

## Rationale and trade-offs

- The owner chose an explicit choice over "empty means unchanged" (A) and
  "always require one" (B): what Save does is visible in the form, and a
  category can be removed on purpose.
- Trade-off accepted: favorites can now be moved to Uncategorized by hand,
  where before only deleting a category did that.

## Approvals and authority

<resolver: e.g. "@Jurzo as owner (product).">

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0043-favorite-one-save.md`
- Mission Brief: `.agentic/missions/MB-0058-favorite-one-save.md`
- Evidence: CRP-0043 → Minimal context and Evidence bundle.

## Constraints and follow-on rules (Hardened)

1. Clearing a category happens only when "No category" is chosen; a Save
   that keeps the current choice never changes the category.
2. The "No category" value is a fixed token the server parses explicitly
   (not an empty or missing field); a missing field is rejected, so a form
   that omits the picker can't clear a category by accident.
3. The choice is labelled in both catalogs.

## Roll-back into the system (Hardened)

To be applied after this record is signed, with the owner's OK:

- `MB-0058`:
  - P1's property names the "No category" choice and the preselection;
  - a new P3 replaces today's (unchanged fields), adding: "Save with 'No
    category' clears the category; the product shows under Uncategorized
    (favorites) or stays uncategorized (recipe products); Save with the
    current choice changes only the name";
  - the escalate entry is marked decided (RR-0043);
  - `resolutions: [RR-0043]`;
  - plan step 1 is marked done.
- `CRP-0043`: `state: open` → `answered`.

## Supersession and history

Status: active once signed. Supersedes nothing.
