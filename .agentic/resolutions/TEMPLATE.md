# Resolution Record — <title>

Copy to `RR-<id>-<slug>.md`. One RR closes one consult (or one merge review).
RRs are authored by the resolver, not the agent. Status `approved` is what
unlocks CI gates that require an RR.

```yaml
---
id: RR-0001
closes: CRP-0001
mission: MB-0001
decided_by: "@juuso-kalliomaki"
role: data-owner | security | api-owner | platform | product
risk_tier: low | medium | high
decided_at: 2026-10-28T00:00:00Z
status: approved | rejected | superseded
superseded_by: null
rolls_back_into:
  - .agentic/mentorship/<file>.md
  - .agentic/runbooks/<file>.md
---
```

## Decision and outcome

One sentence. Approved option. Scope of approval (what this covers and does
not cover).

## Rationale and trade-offs

Why chosen. What was rejected and why.

## Approvals and authority

Who approved and in what role. If risk tier is high, record co-approvers.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-….md`
- Mission Brief: `.agentic/missions/MB-….md`
- Evidence links:

## Constraints and follow-on rules (Hardened)

New invariants. Must/should rules for future work in this area.

## Roll-back into the system (Hardened)

Which Mentorship Pack / Runbook / Mission Brief template is updated as a
result. Link the PR that applies the roll-back.

## Supersession and history

Status: active | superseded.  Superseded by: RR-….
