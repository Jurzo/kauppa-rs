# Consultation Request Pack — A Continuity Pack for every Mission Brief

```yaml
---
id: CRP-0004
mission: MB-0011
owner: "@juuso-kalliomaki"
target_role: platform
risk_tier: low
urgency: low
state: answered
runbook_trigger: ".agentic/README.md rule 1 (editing a Runbook needs a consult and a closing RR); AGENTS.md → Lifecycle (agent-proposed changes to AGENTS.md go through a CRP and an RR); feature.md escalation trigger `.agentic/{mentorship,runbooks,resolutions}/**`"
---
```

## Decision statement

Make "one Continuity Pack per Mission Brief, created with the brief and kept
current" a gated rule in `AGENTS.md` and the runbooks, and backfill packs for
the existing briefs.

## Options and trade-offs

### Option A — rule in AGENTS.md and the runbook entry gates, plus a backfill (owner's instruction)

- Pros: every agent session loads the rule from `AGENTS.md`, and the runbooks
  check it at the entry gate. Every existing brief gets a resumable state
  record now.
- Cons: some overhead on every brief, and a pack for a brief that hasn't
  started is mostly empty.
- Risk: low — documentation only; no code or CI changes.
- Cost / timeline: one PR (MB-0011).

### Option B — status quo ("one per active mission" in `.agentic/README.md` only)

- Pros: no change.
- Cons: nothing prompts an agent to write a pack. Through ten briefs, none was
  written.
- Risk: missions stay resumable only from PR descriptions and chat history.

### Option C — Option A plus enforcement in scripts and CI

- Pros: `scaffold-mission.sh` creates the pack, and `validate_artifacts.py`
  fails on a brief without one, so it can't be forgotten.
- Cons: changes platform-owned scripts and the CI gate.
- Cost / timeline: a follow-up PR. Not part of this decision unless the
  resolver says so.

## Recommendation and question

Recommended option: **A**, as the owner instructed on 2026-10-29. C is a
follow-up.

Crisp question to answer: **Approve Option A?** That covers:

- the edits on branch `agentic/continuity-packs` to `AGENTS.md`,
  `.agentic/runbooks/feature.md`, `migration.md`, `dependency-bump.md`,
  `.agentic/continuity/TEMPLATE.md`, and `.agentic/README.md`;
- the backfilled packs for MB-0001 through MB-0011.

And should Option C follow as a separate brief?

## Blast radius and rollback

- **What can break:** nothing at runtime. Agents gain an entry-gate step: a
  brief must have a pack before work starts.
- **Who is affected:** agents and reviewers working on Mission Briefs.
- **Rollback plan:** revert the PR.

## Minimal context

- `ls .agentic/continuity/` before this change: `TEMPLATE.md` only, against
  ten Mission Briefs.
- `validate_artifacts.py` checks `missions/`, `consults/`, and `resolutions/`,
  but not `continuity/`.
- The harness loads `AGENTS.md` into each agent session. There is no
  `CLAUDE.md`.

## Evidence bundle (Hardened)

- PR history #1–#7: mission state was recorded only in PR descriptions.
- The packs are backfilled from the briefs, consults, RRs, and PRs #1–#7.
