# Mission Brief — Trigger-path check keeps the clone's history

```yaml
---
id: MB-0033
owner: "@juuso-kalliomaki"
goal: "`scripts/agentic/check_triggers.py` stops turning a full clone into a shallow one, so agents and humans can run it locally, and it computes the same changed-file list in CI as today."
non_goals:
  - "Changing which paths are trigger paths, or how RRs are matched."
  - "Changes to the other agentic scripts."
acceptance:
  - id: P1
    property: "Run in a full clone, the script leaves `git rev-parse --is-shallow-repository` at `false` and doesn't rewrite `.git/shallow`; run in a shallow clone, it still finds the merge base (deepening only as needed)."
    check: scripts/agentic/test_check_triggers.sh
  - id: P2
    property: "For a branch with known changes, the changed-file list and the triggered categories are the same as before the change (e.g. a branch touching `migrations/` and `routes/auth/` fires `schema` and `security:auth`)."
    check: scripts/agentic/test_check_triggers.sh
  - id: P3
    property: "The `agentic-gate` workflow's trigger-path job passes on this mission's own PR."
    check: "CI: `.github/workflows/agentic-gate.yml` on the PR"
context:
  - scripts/agentic/check_triggers.py
  - .github/workflows/agentic-gate.yml
  - .agentic/runbooks/feature.md
autonomy:
  may_decide:
    - "the fix (e.g. no `--depth` when the repo isn't shallow, or `--deepen`), and the test script's shape (temporary clones under a temp directory)"
  escalate:
    - { when: "any change under `.github/` (e.g. the checkout's `fetch-depth`)", to: platform-owner }
    - { when: "the fix changes which files the check reports, or how it treats a missing merge base", to: owner }
  forbidden:
    - "change the trigger path list or RR matching"
    - "run the old script in the main working copy while testing (use temporary clones)"
runbook: .agentic/runbooks/bugfix.md@v1
mentorship:
  - AGENTS.md@v1
resolutions: []
---
```

## Context

Found on 2026-11-08: a sub-agent ran `check_triggers.py` locally and the
shared repo became shallow, which broke later merges until
`git fetch --unshallow origin`. The cause is `changed_files()`:
`git fetch --no-tags --depth=1 origin <base>` truncates history even in a
full clone. CI checks out with `fetch-depth: 0`, so there the fetch is
unnecessary as well. Since then agents are told not to run the script
locally and replicate its logic with `git diff` instead.

## Conceptual plan

1. Reproduce in a temporary clone (P1 fails).
2. Fix the fetch; keep the fallback for a missing merge base.
3. The test script builds a throwaway repo with a base and a branch, runs
   the check in full and shallow clones, and compares output.

## Validation plan

- P1, P2: `scripts/agentic/test_check_triggers.sh` (new), red before the
  fix, green after.
- P3: the PR's `agentic-gate` run.
