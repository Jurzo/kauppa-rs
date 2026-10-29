# Mission Brief — A Continuity Pack for every Mission Brief

```yaml
---
id: MB-0011
owner: "@juuso-kalliomaki"
goal: Give every Mission Brief a Continuity Pack that is created with the brief and kept current, so any agent or human can resume a mission from the repo instead of from chat history.
non_goals:
  - Enforcing packs in scripts or CI (pack creation in scaffold-mission.sh, a missing-pack check in validate_artifacts.py) — a possible follow-up; scripts are platform-owned.
  - A JSON Schema for Continuity Pack front matter.
  - Changing the sections of the Continuity Pack template.
acceptance:
  - id: P1
    property: Every Mission Brief in `.agentic/missions/` has a Continuity Pack at `.agentic/continuity/MB-<id>.md`.
    check: 'for b in .agentic/missions/MB-*.md; do id=$(basename "$b" | cut -d- -f1-2); test -f ".agentic/continuity/$id.md" || echo "missing $id"; done — prints nothing'
  - id: P2
    property: AGENTS.md states the rule — create the pack with the brief; update it at checkpoints, consult/RR changes, PR open/merge, and handoffs; close it out without deleting it.
    check: review of the "Continuity Packs" section in AGENTS.md
  - id: P3
    property: The feature, migration, and dependency-bump runbooks gate on the pack (entry gate item 4); the feature runbook also requires updates at checkpoints and lists the pack under required outputs. Bugfix inherits from feature.
    check: review of the entry gates in .agentic/runbooks/feature.md, migration.md, and dependency-bump.md
  - id: P4
    property: The Continuity Pack template and `.agentic/README.md` say one pack per Mission Brief, not one per active mission.
    check: review of .agentic/continuity/TEMPLATE.md and .agentic/README.md
context:
  - AGENTS.md
  - .agentic/README.md
  - .agentic/continuity/TEMPLATE.md
  - .agentic/runbooks/feature.md
  - .agentic/runbooks/migration.md
  - .agentic/runbooks/dependency-bump.md
autonomy:
  may_decide:
    - wording of the instructions, within RR-0004
    - the content of each backfilled pack, taken from the repo's artifacts and PR history
  escalate:
    - { when: changing scripts or CI to enforce packs, to: platform-owner }
    - { when: "any change to a runbook, Mentorship Pack, or AGENTS.md beyond RR-0004", to: platform-owner }
  forbidden:
    - delete or rewrite an existing Mission Brief, consult, or Resolution Record
    - lower lint strictness
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
resolutions:
  - RR-0004
---
```

## Context

`.agentic/continuity/TEMPLATE.md` has existed since the scaffold, and
`.agentic/README.md` said "one per active mission". Nothing in the runbooks or
in `AGENTS.md` mentioned Continuity Packs, and no script creates or checks
them. Through ten Mission Briefs and seven PRs, not one pack was written. The
state of each mission lived in PR descriptions and chat sessions instead.

On 2026-10-29 the owner asked for one pack per Mission Brief, kept up to date,
with instructions so this happens from now on.

## Conceptual plan

- Write the rule into `AGENTS.md`, which the harness loads into every agent
  session.
- Make it an entry-gate item in the feature, migration, and dependency-bump
  runbooks, the ones a brief's `runbook:` field can name. Bugfix inherits from
  feature.
- Align the template and `.agentic/README.md`.
- Backfill packs for MB-0001 through MB-0010 from the repo's artifacts and PR
  history, and create this brief's own pack with it.

Runbook and `AGENTS.md` edits need a consult and an RR (`.agentic/README.md`
rule 1; `AGENTS.md` → Lifecycle). CRP-0004 asks; RR-0004 records the
decision.

## Validation plan

- **P1:** the shell check in the front matter, run from the repo root; it
  prints nothing when every brief has a pack.
- **P2–P4:** reviewing the named sections in the PR diff.
- `validate_artifacts.py` stays green, apart from RR-0004's placeholders until
  the resolver signs it.
