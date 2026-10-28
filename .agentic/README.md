# `.agentic/` — Agentic SE artifacts

This directory holds the governance and evidence artifacts for `kauppa-rs`,
per the [baseline](../../Agentic%20SE%20Implementation%20Baseline.md).

| Subdir | What goes here | Owner |
| --- | --- | --- |
| `mentorship/` | Task-class Mentorship Packs that refine `AGENTS.md`. | Human |
| `runbooks/` | Workflow Runbooks with enforced gates. | Human |
| `missions/` | Mission Briefs (`MB-<id>-<slug>.md`). | Agent drafts, human accepts |
| `continuity/` | Continuity Packs, one per active mission. | Agent |
| `consults/` | Consultation Request Packs (`CRP-<id>-<slug>.md`). | Agent |
| `resolutions/` | Resolution Records (`RR-<id>-<slug>.md`). | Human (risk-tier dependent) |
| `schemas/` | JSON Schemas validated by the `agentic-gate` CI job. | Human |

### Rules

1. **Never edit a Mentorship Pack, Runbook, or Resolution Record without
   consulting first.** Agents may draft, humans approve. The approval is the
   Resolution Record that closes the consult.
2. **Every change under this directory is reviewed in a PR with CODEOWNERS
   approval.** See `.github/CODEOWNERS`.
3. **IDs are monotonic.** Pick the next integer; don't reuse slots.
4. **Every artifact has front matter** validated against `schemas/`.
