# Mission Brief — Drag flows that pass when the browser suite runs more than once

```yaml
---
id: MB-0069
owner: "@juuso-kalliomaki"
goal: "The browser suite passes when its flows run more than once against one database (`scripts/browser/run.sh --repeat-each=N`), so a repeated run finds flaky flows rather than failing on rows earlier runs left behind: the drag flows bring the rows they drag into view first, and a drag that can't reach its rows fails at once with a message naming them."
non_goals:
  - "Changing the app, its templates or `assets/app/sortable.js`: this mission changes `tests/browser/` only."
  - "A fresh database per flow or per repeat: `scripts/browser/run.sh` keeps one throwaway database per run (RR-0041)."
  - "Retries or longer timeouts: `retries: 0` stays (RR-0041), and so do the 30 s test and 5 s expect timeouts."
  - "Changing the other flows, which pass on a repeated run today."
acceptance:
  - id: P1
    property: "`scripts/browser/run.sh --repeat-each=3` passes every flow (today 16 flows after the 2 setup steps, which run once: 50 runs). Today the second run of both drag flows in `reorder.spec.js` fails."
    check: "scripts/browser/run.sh --repeat-each=3"
  - id: P2
    property: "Before pressing the mouse, the drag helper scrolls the dragged handle and its target row into view and checks that both lie inside the viewport. If they can't both be shown, the flow fails within the 5 s expect timeout with a message naming the two rows, not after the 30 s test timeout."
    check: "scripts/browser/run.sh (reorder.spec.js), plus one run with the viewport shortened so the rows can't both show (output recorded in the PR)"
  - id: P3
    property: "The flows still prove what they prove today. Every assertion in `reorder.spec.js` and `rows.spec.js` stays (MB-0066 P1, P4; MB-0068 P1, P2), a single run passes as before, and each drag still saves the order in one request."
    check: "scripts/browser/run.sh"
context:
  - tests/browser/flows/reorder.spec.js
  - tests/browser/flows/rows.spec.js
  - tests/browser/flows/fixtures.js
  - tests/browser/playwright.config.js
  - scripts/browser/run.sh
  - .github/workflows/browser.yml
  - .agentic/resolutions/RR-0041-browser-level-test.md
  - .agentic/resolutions/RR-0053-drag-to-reorder.md
autonomy:
  may_decide:
    - "the drag helper's code, and whether it stays in `reorder.spec.js` or moves to `fixtures.js`"
    - "the flows' test data (names, how many rows they create), keeping what each flow checks"
    - "test names inside `tests/browser/flows/`"
  escalate:
    - { when: "running the CI browser job with `--repeat-each=2` or more, so a flow that stops being repeatable fails in CI: the job takes about 1.5 minutes longer per run (RR-0041 sub-answer 3 set when and how it runs) (decided by the owner on acceptance, 2026-11-22: CI runs the suite once)", to: platform-owner }
    - { when: "a flow can be made repeatable only by changing the app or a template (a non-goal)", to: owner }
  forbidden:
    - "remove or weaken an assertion"
    - "add retries or raise a timeout to make a flow pass"
    - "change a file outside `tests/browser/` (CI keeps running the suite once)"
runbook: .agentic/runbooks/bugfix.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/actix-web.md@v1
resolutions: []
---
```

## Context

Found while building MB-0068 (2026-11-20) and noted in its Continuity Pack
as a follow-up. The owner asked for a brief on 2026-11-22.

**Repro**, on main at `4d4c240`, run on 2026-11-22:

- **Command:** `scripts/browser/run.sh --repeat-each=2`.
- **Result:** 32 passed, 2 failed. The failures were the second runs of
  "dragging reorders categories, a favorites group and ingredients" and
  "dragging works after an in-app navigation (RR-0054)".
- **Where they fail:** both time out after 30 s waiting for
  `POST /categories/order` (`reorder.spec.js:88` and `:158`), because the
  drop never happens.
- **Every other flow**, including both of `rows.spec.js`, passed twice.

**Cause:**

- `run.sh` makes one throwaway database per run, and the flows share one
  account (`workers: 1`).
- Each flow names its rows with a run-unique suffix, so on a repeated run
  the categories page still holds every earlier run's categories. New ones
  are added at the end of the list.
- The drag helper (`drag` in `reorder.spec.js`) reads both rows'
  `boundingBox()`, which is in viewport coordinates, and moves the mouse
  there without scrolling.
- Once the rows are below the 1280 × 720 viewport, the mouse is pressed
  outside the page, Sortable never starts, and the flow waits for the order
  request until the test timeout.

**Why it matters:**

- A repeated run is the local way to look for flaky flows.
- RR-0041 kept the browser job optional until its flake rate is known.
- MB-0068's taller rows (about 68 px at 360 px) bring the failure sooner.
- A later drag flow on the favorites page (the drag-between-categories
  follow-up) will drag on a page longer than the viewport.

## Conceptual plan

1. **Red.** Record the failing `--repeat-each=2` run (above). The PR links
   it, or shows the failure in a commit (bugfix runbook, rule 2).
2. **Fix the drag helper (P2).** Scroll the handle and then the target row
   into view (`locator.scrollIntoViewIfNeeded()`), then measure both and
   check they lie inside the viewport. Then drag as today, at a pointer's
   pace, keeping the pause Sortable needs while a row animates.
3. **Check the rest (P1, P3).** Look for other leftover-data assumptions
   with `--repeat-each=3`: a lookup that matches an earlier run's row, or a
   list the test expects to be short.
4. **Clarify.** Whether CI should repeat the flows was the platform
   owner's call. Decided on the brief's acceptance (2026-11-22): CI runs
   the suite once; a repeated run stays a local check. No consult.

Slow-mode triggers:

- A flow that only fails on a repeat for a reason other than position:
  shared state the suffix doesn't separate.
- The dragged and target rows can't both fit in the viewport for a flow's
  data. Change the data, not the assertion.

## Validation plan

- **P1:** `scripts/browser/run.sh --repeat-each=3`, before the fix (red,
  the two drag flows) and after it (green, 50 of 50). Output summarised in
  the PR.
- **P2:**
  - the drag flows pass on a single run and on a repeated run;
  - one run with a temporary viewport too short for both rows fails within
    5 s with the rows' names. Output in the PR; the temporary change is not
    committed.
- **P3:** a single `scripts/browser/run.sh` passes, and the PR diff shows no
  assertion removed.
