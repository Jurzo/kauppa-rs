# Mission Brief — Uncategorized items start with their edit section closed

```yaml
---
id: MB-0037
owner: "@juuso-kalliomaki"
goal: "Adding an item without a category leaves the list tidy: the new item's edit section (category, quantity, remove) is closed until the user opens it, instead of opening by default."
non_goals:
  - "Changing what the edit section contains or how a category is set."
  - "Remembering which sections a user opened across reloads."
acceptance:
  - id: P1
    property: "On the list page and in the list fragment htmx returns after adding an item, no item's edit section (`<details class=\"product-edit\">`) is rendered `open`, whether or not the item has a category and whether or not the user has categories."
    check: cargo test -p kauppa-web routes::lists::tests::items::edit_sections_start_closed
  - id: P2
    property: "An uncategorized item's summary still says it needs a category (today \"Choose category\" for the owner), so the cue stays visible while closed."
    check: cargo test -p kauppa-web routes::lists::tests::items::uncategorized_summary_cue
context:
  - templates/lists/_list.hbs
  - crates/kauppa-web/src/routes/lists/tests/items.rs
  - .agentic/missions/MB-0004-assemble-list.md
autonomy:
  may_decide:
    - "test names inside `routes::lists::tests::items`"
    - "styling the closed summary so the cue stands out (CSS in `app.css`)"
  escalate:
    - { when: "keeping some case open (e.g. only the list's first uncategorized item, or after a category assignment fails)", to: owner }
  forbidden:
    - "JavaScript"
    - "changes to routes or handlers beyond what P1's fragment needs"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
resolutions: []
---
```

## Context

Requested by the owner (2026-11-09). Since MB-0004 (f4de3e9), `_list.hbs`
renders an item's `<details class="product-edit">` with `open` when the
viewer is the owner, the item has no category, and the user has
categories. Every free-text item is added uncategorized, so each addition
opens a full edit form in the list, pushing the rest down on a phone. The
template can't tell a just-added item from older ones, so the change is
simply to drop the default `open`.

MB-0036 (one add field) changes the add form in the same template; either
can land first.

## Conceptual plan

Remove the `open` condition; keep the "Choose category" summary; tests
first (red: an uncategorized item renders `open`).

## Validation plan

- P1, P2: route tests on the full page and the htmx fragment after
  `POST /lists/{id}/items`.
