# Mission Brief — <title>

Copy this file to `MB-<id>-<slug>.md` with the next unused integer id.
Keep this template unchanged.

```yaml
---
id: MB-0001
owner: "@you"
goal: One sentence describing the user value and the outcome.
non_goals:
  - Explicitly out-of-scope item
  - Another explicitly out-of-scope item
acceptance:
  - id: P1
    property: Describe the invariant or user-visible behavior.
    check: Name the test or script that proves it.
  - id: P2
    property: Another property.
    check: Another check.
context:
  - crates/kauppa-core/src/domain/
  - .agentic/mentorship/actix-web.md
autonomy:
  may_decide:
    - internal refactors inside the touched module
  escalate:
    - { when: schema change, to: data-owner }
    - { when: public route added or removed, to: api-owner }
  forbidden:
    - edit `migrations/` without an approved RR
    - add dependencies
    - lower lint strictness
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/actix-web.md@v1
resolutions: []
---
```

## Context

A few paragraphs with the information an agent needs to execute, pulled from
tickets, chats, or incident reports. Keep it short — precision over volume.

## Conceptual plan

Strategy, not steps. Checkpoints. Slow-mode triggers (what makes you stop
and re-plan). Open questions.

## Validation plan

For each acceptance property, how its check will be run (unit, integration,
property, E2E) and what evidence it produces (test name, log, screenshot).
