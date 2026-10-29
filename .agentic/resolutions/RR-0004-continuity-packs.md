# Resolution Record — A Continuity Pack for every Mission Brief

```yaml
---
id: RR-0004
closes: CRP-0004
mission: MB-0011
decided_by: "@juuso.kalliomaki"
role: platform
risk_tier: low
decided_at: 2026-10-29T00:00:00Z
status: approved
superseded_by: null
rolls_back_into:
  - AGENTS.md
  - .agentic/README.md
  - .agentic/continuity/TEMPLATE.md
  - .agentic/runbooks/feature.md
  - .agentic/runbooks/migration.md
  - .agentic/runbooks/dependency-bump.md
---
```

## Decision and outcome

Approve Option A of CRP-0004. Every Mission Brief has a Continuity Pack,
created with the brief and kept current. The rule lives in `AGENTS.md`, and the
feature, migration, and dependency-bump runbooks gate on it. Packs are
backfilled for MB-0001 through MB-0011.

Not covered: enforcing packs in scripts or CI (Option C). That needs its own
brief, if wanted.

## Rationale and trade-offs

The template existed, but nothing prompted agents to use it, and ten briefs
went by without a pack. `AGENTS.md` is loaded into every agent session, and
the runbook entry gates are where an agent checks preconditions, so the rule
sits in both.

- Rejected: the status quo (B) — it already failed in practice.
- Deferred: script and CI enforcement (C) — stronger, but platform-owned.

## Approvals and authority

Approved by platform, @juuso.kalliomaki

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0004-continuity-packs.md`
- Mission Brief: `.agentic/missions/MB-0011-continuity-packs.md`
- Evidence links: PR #8 (branch `agentic/continuity-packs`).

## Constraints and follow-on rules (Hardened)

1. Every Mission Brief has a Continuity Pack at
   `.agentic/continuity/MB-<id>.md`, created in the same commit as the brief.
2. A pack is updated, with `updated_at` bumped, in the same commit or PR as the
   work it describes, and at least:
   - at each runbook stage checkpoint;
   - when a consult or RR changes state;
   - when a PR opens or merges;
   - at a session's end, at context compaction, and at any handoff.
3. Packs are never deleted. The mission's final PR records the end state.
4. Packs are agent-owned; updating one needs no consult.
5. The feature, migration, and dependency-bump runbooks check for the pack at
   the entry gate. Bugfix inherits from feature.

## Roll-back into the system (Hardened)

The six files in `rolls_back_into` were edited on branch
`agentic/continuity-packs` on the owner's instruction, before this record was
drafted. Approving this RR covers those edits as they stand, and they merge in
the same PR.

## Supersession and history

Status: active
