# Consultation Request Pack — <title>

Copy to `CRP-<id>-<slug>.md`. Must be opened BEFORE acting at a decision
boundary — not after.

```yaml
---
id: CRP-0001
mission: MB-0001
owner: "@you"
target_role: data-owner | security | api-owner | platform | product
risk_tier: low | medium | high
urgency: low | medium | high
state: open | answered | superseded
runbook_trigger: path or rule in .agentic/runbooks/* that fired
---
```

## Decision statement

One sentence naming the decision to be made.

## Options and trade-offs

### Option A — <name>

- Pros:
- Cons:
- Risk:
- Cost / timeline:

### Option B — <name>

- Pros:
- Cons:
- Risk:
- Cost / timeline:

### Option C — <name> (if applicable)

- Pros:
- Cons:
- Risk:
- Cost / timeline:

## Recommendation and question

Recommended option: …
Because: …

Crisp question to answer: …

## Blast radius and rollback

What can break:
Who is affected:
Rollback plan:

## Minimal context

Relevant brief excerpts, constraints, impacted modules.

## Evidence bundle (Hardened)

Benchmarks, logs, tests, repro, data volume estimates.
