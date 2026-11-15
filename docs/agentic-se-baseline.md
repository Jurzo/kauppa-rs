# Agentic SE Implementation Baseline

Oct 28, 2026 · @Juuso

## Purpose and scope

This baseline defines the minimum artifacts, gates and repository conventions a team needs to run Agentic Software Engineering (Agentic SE) as described in Ahmed E. Hassan's *Agentic Software Engineering* (2026). It turns the book's Chapter 1 mental model and the Appendix A reference schemas into something a Java and TypeScript/Angular team can adopt incrementally.

The book is explicit that the Appendix A tables are starter schemas: adopt a minimum viable subset first, then harden. This baseline follows that rule. Every artifact section below is split into an **MVP** tier (adopt now) and a **Hardened** tier (add when volume or risk grows).

- **In scope:** the seven coordination artifacts from Chapter 1, their Appendix A section schemas, the principles that govern them, and how to wire them into Maven/Gradle, Angular CLI and CI.
- **Out of scope:** the interaction patterns (Chapter 2), paradoxes (Chapter 3) and the trust, capability and coordination engineering chapters. These refine the baseline but are not required to start.
- **Not a prompt guide:** the book frames Agentic SE as an engineering system across actors, process, tools and artifacts, not as prompt technique. This baseline does the same.

## Core concepts from Chapter 1

Agentic SE is the discipline of producing high-quality, trustworthy software from stochastic contributors, AI and human alike, by making the whole engineering system ready. Reliability comes from the system around the contributors, not from perfect contributors.

### Four pillars, two modalities

The system has four pillars: **actors, process, tools and artifacts**. Each pillar shows up differently in two modalities:

|  | SE4Humans | SE4Agents |
| --- | --- | --- |
| Role | Intent, judgment, governance, mentorship, ownership of the product | Execution at machine speed and scale |
| Workbench needs | Impact summaries, risk views, evidence bundles, a structured mailbox instead of chat | Deterministic checks, stable interfaces, machine-readable feedback, fast inner loops |
| Mailbox contents | Briefs to approve, consults to decide, merge packs to audit, mentorship updates to review | Tasks derived from a Mission Brief, governed by a Workflow Runbook |
| Escape hatch | Drop into a normal IDE for a surgical edit without breaking the record | Escalate only significant problems to the human side |

Treat the agentic harness as a **new actor**, not a tool upgrade. Today's harnesses mostly serve the human side and are early prototypes of the eventual workbench.

### The spectrum of practice

| Mode | Optimizes for | Fits | Done means |
| --- | --- | --- | --- |
| Vibe coding | Creation speed | Prototypes, one-off scripts, personal automation | It works for me |
| Vibe engineering | Durable outcomes | Small team tools that must be maintained | It works, I can explain why, and it can evolve |
| Agentic SE | Trust at scale | Production systems with parallel AI output | It meets the spec under constraints, with auditable evidence |

The baseline targets the third row. Vibe coding stays legitimate for exploration; the key move is crystallizing informal exploration into versioned artifacts once stakes rise.

### Why the bottleneck moves

When output explodes, attention becomes the scarce resource. "We will just review more" does not scale. Review shifts from reading diffs to auditing evidence, and in the longer run integration and coordination become the real constraint.

## Guiding principles

Ten rules, distilled from Chapter 1, decide every design choice in the rest of this baseline. When a team debate is stuck, check it against this list.

1. **Engineer the system, not the agent.** Invest in actors, process, tools and artifacts together. A new harness without a new system only produces faster disasters.
2. **Trust comes from deterministic evidence, not deterministic steps.** Demand that work meets the spec under constraints with proof; do not demand it be done the way a human would.
3. **Artifacts are the interface.** Explore informally, then crystallize into explicit, versioned artifacts once the stakes rise.
4. **Humans author the coordination system; agents generate task artifacts.** Mission Briefs, Mentorship Packs, Workflow Runbooks and Resolution Records need a human owner who accepts them. Consults, merge packs and manifests are agent output.
5. **Mentorship shapes judgment; runbooks enforce.** If a practice is essential, put it behind a gate, command or trigger. Mentorship alone is never enough for a must-have.
6. **Encode outcomes and boundaries, not cognitive strategies.** Write goals, constraints and quality criteria. Do not script how the agent should debug, analyze or design.
7. **Curate the smallest governance set.** More rules create conflicts, conflicts create ambiguity, and ambiguity creates non-determinism.
8. **Escalate at decision boundaries, and close every escalation in a record.** Schema, security, compatibility, performance and operational decisions go to a human or designated resolver. Outcomes left in chat are lost.
9. **Every resolution feeds back into the system.** A new norm updates the Mentorship Pack; a missing gate updates the Workflow Runbook; a recurring constraint updates the Mission Brief template.
10. **Artifacts are a platform layer.** Version them, review them, refactor them and assign owners, exactly like CI pipelines. Unowned artifacts become artifact debt.

## Artifact baseline (from Appendix A)

Seven artifacts carry the whole system. Four are human-owned governance; three are agent-generated evidence. Each table below condenses the matching Appendix A schema into what to include and how the baseline checks it.

| Artifact | Purpose | Authored by | Produced when | Determinism | Book table |
| --- | --- | --- | --- | --- | --- |
| Mission Brief | Bounded intent: the unit of delegation | Human (agent may draft) | Before work starts | Free-form text, deterministic checkpoints via runbook | Table 1 |
| Continuity Pack | Resumable state across resets and handoffs | Agent, in a human-defined structure | At handoffs, compaction, agent swaps, end of day | Required sections fixed, content free | Table 1A |
| Mentorship Pack | Team norms and "what good looks like" | Human (agent proposes) | Long-lived, updated from resolutions | Non-deterministic conditioning | Table 2 |
| Workflow Runbook | Execution protocol with gates | Human (agent proposes) | Long-lived or per task type | Deterministic triggers and gates | Table 3 |
| Consultation Request Pack | Structured decision request | Agent | At a decision boundary | Deterministic trigger, free content | Table 4 |
| Resolution Record | Durable decision and its follow-on rules | Risk-tiered; human approval for higher tiers | After a consult or merge review | Structured fields fixed | Table 4A |
| Merge-Readiness Pack | Evidence that delivery is merge-ready | Agent, under human-defined requirements | On delivery | Evidence requirements fixed, narrative free | Table 5 |

&#91;embedded content: artifact lifecycle · 4 governance artifacts, 3 evidence artifacts\]

The brief pulls in mentorship and runbooks, the agent works inside those gates, and every consult or merge review closes in a Resolution Record that feeds back into the governance artifacts.

### Mission Brief (Table 1)

The Mission Brief is a version-controlled specification for action, not a better prompt. It must link to the Mentorship Pack and Workflow Runbook versions it relies on.

| Section | Must contain | Tier | Baseline check |
| --- | --- | --- | --- |
| Goal and intent | Objective, user value, explicit non-goals, non-negotiable constraints | MVP | Front-matter fields `goal` and `non_goals` are non-empty |
| Success criteria | Scope and non-scope; acceptance as properties or invariants; pre/postconditions; latency or cost budgets | MVP | At least one acceptance property with an ID |
| Validation plan and evidence obligation | Required checks and test tiers; each acceptance property mapped to a check and the evidence it produces | MVP | Every property ID maps to a check |
| Escalation and permissions | What the agent may decide, must escalate (and to which role), must never do; stop conditions; risk thresholds | MVP | `autonomy.escalate` and `autonomy.forbidden` present |
| Curated context | Pointers to key modules, boundaries, gotchas, relevant mentorship and runbook entry points, data model notes | MVP | Paths in `context` exist in the repo |
| Implementation guidance | Preferred patterns, areas to avoid, do-not-touch APIs, compatibility and security constraints | Hardened | Do-not-touch paths feed the path guard (see stack section) |
| Conceptual plan | Strategy, checkpoints, slow-mode triggers, decision boundaries; not a brittle step list | Hardened | Checkpoints referenced in the plan ledger |
| Version management and roll-up | How clarifications become brief deltas; how Resolution Records link in; revision history | Hardened | Brief lives in git; RR IDs listed in `resolutions` |

If the goal is missing, the agent delivers correct work for the wrong problem. If success criteria are missing, "looks right" replaces "is right". Keep briefs short: the book stresses precision over volume, and overloading context raises cost and lowers quality.

### Continuity Pack (Table 1A)

The Continuity Pack preserves what matters for resuming work and lists dead ends so they are not re-explored. Keep it as its own file, or fold it into the brief as an aggressively compacted "Current state" section.

| Section | Must contain | Tier |
| --- | --- | --- |
| Current state snapshot | What is true now, in flight, done and pending | MVP |
| Open questions and next steps | Smallest next actions, known unknowns, where to gather evidence next | MVP |
| Dead ends and rejected approaches | What was tried, why it failed, pointers to logs, benchmarks or branches | MVP |
| Decisions and constraints | Decisions made, constraints discovered, links to Resolution Records | Hardened |
| Evidence pointers | Links to tests, traces, benchmarks and scans so far | Hardened |
| Handoff metadata | Timestamp, reset reason, owner, risk notes, routing hints for pending consults | Hardened |

### Mentorship Pack (Table 2)

The Mentorship Pack primes the agent's judgment; it does not enforce anything. Agents propose updates, humans approve them, and changes are versioned. Many teams already approximate this with `CLAUDE.md`, `AGENTS.md` and skill files; the baseline formalizes that.

| Section | Captures | Typical contents | Tier |
| --- | --- | --- | --- |
| Map | What the system is and where changes belong | Architecture map, key modules and boundaries, UI surfaces, tech stack, the agent's role | MVP |
| Operating playbook | How work is done here | Setup, build and run, testing expectations, CI and release flow, conventions, examples | MVP |
| Engineering intent | The why and the non-negotiables | Goals, non-functional priorities, design rationale, do-not-touch boundaries | MVP |
| Governance and safety | Decision rights and escalation | Owners, approval thresholds, consult protocol, secrets handling, risky operations | Hardened |
| Context engineering rules | How context is loaded and kept coherent | Budgets, invariant vs preference priority, retrieval rules, contradiction handling | Hardened |
| Lifecycle and references | How the pack stays true | Status, version history, deprecations, precedence across org levels, retirement rules | Hardened |

Write preferences and boundaries, not thinking procedures. When a reviewer corrects the agent, the agent should propose a general rule for the human to approve into this pack.

### Workflow Runbook (Table 3)

The Workflow Runbook is the SOP for the development loop and the place where enforcement lives. Gates can be automated, human or agentic; triggers must be deterministic and traceable.

| Section | Must contain | Tier |
| --- | --- | --- |
| Entry gate | Required context loaded; environment and baseline checks pass before work starts | MVP |
| Stages and gates | Named stages with checkpoints and stop rules | MVP |
| Deterministic checks | Build, tests, lint, static analysis, security scans | MVP |
| Escalation triggers | What requires a consult, where to stop, required routing metadata | MVP |
| Required outputs | Which packs must be produced and linked | MVP |
| Exploration policy | Number of alternatives, comparison criteria, decision rule | Hardened |
| Commands and triggers | Always-on enforcement spines, tagged human-only, agent-only or both | Hardened |
| Plan ledger and divergence rules | Capture of the executed plan and divergences; when a divergence needs approval | Hardened |
| Traceability and explainability | Evidence of whether a trigger ran, why, and what executed | Hardened |
| Decomposition and ownership | How work is split, ownership boundaries, interface expectations, handoff rules | Team scale |
| Layered readiness and integration scheduling | Code complete vs merge-ready vs integration-ready; sequencing and timing rules | Team scale |

### Consultation Request Pack (Table 4)

When the agent hits a decision boundary, it stops and produces a consult instead of acting silently or asking a vague question. The pack must name the runbook trigger that caused it.

| Section | Must contain | Tier |
| --- | --- | --- |
| Decision statement | One sentence naming the decision | MVP |
| Options and trade-offs | Two or three options with pros, cons, risk, cost and timeline | MVP |
| Recommendation and question | Recommended option, why, and a crisp question | MVP |
| Blast radius and rollback | What can break, who is affected, rollback plan | MVP |
| Minimal context | Relevant brief excerpts, constraints, impacted modules | MVP |
| Evidence bundle | Benchmarks, logs, tests, repro, data volume estimates | Hardened |
| Routing metadata | Stable ID, owner, target role, risk tier, urgency, lifecycle state, links to brief and continuity pack | Hardened |

### Resolution Record (Table 4A)

Every consult and every merge review ends in a Resolution Record. Without it, the team re-litigates the same decisions. Its most important section is the roll-back: each record says which system artifact it changes.

| Section | Must contain | Tier |
| --- | --- | --- |
| Decision and outcome | Decision in one sentence, approved option, scope of approval | MVP |
| Rationale and trade-offs | Why chosen, what was rejected and why | MVP |
| Approvals and authority | Who approved, their role, risk tier, timestamp | MVP |
| Linked artifacts and evidence | Links to the consult or merge pack and the evidence relied on | MVP |
| Constraints and follow-on rules | New invariants and must/should rules for future work | Hardened |
| Roll-back into the system | Required updates to the Mentorship Pack, runbook gates or brief template | Hardened |
| Supersession and history | Active or superseded, link to the superseding record | Hardened |

### Merge-Readiness Pack (Table 5)

The Merge-Readiness Pack replaces diff archaeology with an evidence audit. It is not a PR with a nice description; it proves each success criterion was checked.

| Section | Must contain | Tier |
| --- | --- | --- |
| Scope-to-proof map | Each brief success criterion mapped to a check and its evidence | MVP |
| Verification bundle | Tests run, new tests, edge and failure cases, property checks, logs | MVP |
| Engineering hygiene evidence | Lint and static analysis output, complexity signals, coherent change set | MVP |
| Rationale and trade-offs | What was done, why, alternatives considered and rejected | MVP |
| Risk and rollout plan | Blast radius, backward compatibility, rollout steps, rollback, open risks | MVP |
| Plan ledger | Conceptual plan vs executed plan, divergences with rationale and approvals | Hardened |
| Audit trail | Summary first, then links to exact brief, runbook, mentorship and RR versions and tool output | Hardened |
| Exploration archive | Experiments, rejected approaches, key measurements, pointers to full archives | Hardened |
| Machine-readable manifest | Every evidence block with stable ID, pointer, optional checksum, access classification | Hardened |
| Compliance, access and retention | Redaction, data handling, retention per policy | Hardened |
| Integration readiness and timing | Dependencies, sequencing, "not the right time to land" flags | Team scale |

## Applying it to a Java + Angular stack

All artifacts live in the repository next to the code, are reviewed through pull requests, and are validated in CI. This is the book's "artifacts are a platform layer" principle made concrete. Tool names below are suggestions; swap in what your build already uses.

### Repository layout

```text
AGENTS.md                      # root Mentorship Pack (harness-specific rule files point here)
.agentic/
  mentorship/                  # task-class packs: angular-ui.md, persistence.md, api.md
  runbooks/                    # feature.md, bugfix.md, migration.md, dependency-bump.md
  missions/MB-0042-*.md        # Mission Briefs, YAML front matter + prose
  continuity/MB-0042.md        # Continuity Packs, one per active mission
  consults/CRP-0107-*.md       # Consultation Request Packs
  resolutions/RR-0107-*.md     # Resolution Records
  schemas/*.schema.json        # JSON Schemas for every artifact's front matter
.github/pull_request_template.md   # Merge-Readiness Pack skeleton
.github/CODEOWNERS             # humans own mentorship, runbooks, resolutions
```

### Mission Brief front matter

Structured fields are what CI validates; the prose body carries the conceptual plan and context notes.

```yaml
id: MB-0042
owner: "@team-search"
goal: Add sort-by-relevance to the product search API and Angular results page
non_goals: [Personalized ranking, Changes to the indexing pipeline]
acceptance:
  - id: P1
    property: Results are ordered by non-increasing relevance score
    check: jqwik property test SearchServiceRelevanceProperties
  - id: P2
    property: p95 latency of /search stays under 250 ms
    check: Gatling scenario search-relevance
context: [backend/search/, frontend/src/app/search/, .agentic/mentorship/api.md]
autonomy:
  may_decide: [internal refactors inside backend/search]
  escalate: [{when: schema change, to: data-architect}, {when: public API change, to: api-owner}]
  forbidden: [edit db/migration without an approved RR, add dependencies]
runbook: .agentic/runbooks/feature.md@v3
mentorship: [AGENTS.md@v7, .agentic/mentorship/api.md@v2]
resolutions: []
```

### Deterministic gates per stack

The book calls the language and toolchain an extra layer of the safety system. Strict compilers and expressive feedback catch hallucinations early, so turn the strictness up.

| Gate | Java (Maven/Gradle) | Angular / TypeScript |
| --- | --- | --- |
| Language strictness | Compiler warnings as errors; nullness annotations with a nullness checker | `strict: true`, `strictTemplates`, `noUncheckedIndexedAccess` |
| Build and unit tests | `./mvnw verify` or `./gradlew check` | `ng build` and `ng test` with the workspace's test runner |
| Property-based acceptance | jqwik | fast-check |
| Lint and static analysis | Checkstyle or Spotless, PMD, SpotBugs or Error Prone | angular-eslint, Prettier check |
| Architecture boundaries | ArchUnit rules for layers and do-not-touch packages | ESLint module boundary rules or Nx tags |
| Coverage on changed code | JaCoCo threshold on the diff | Coverage threshold in the test runner config |
| Security and dependencies | OWASP Dependency-Check or the org scanner | `npm audit` or the org scanner |
| Performance budgets | Gatling or JMH where the brief sets a budget | Bundle budgets in `angular.json`; Lighthouse CI for key pages |
| End-to-end | Testcontainers integration tests | Playwright |

### Escalation triggers (MVP set)

Any change touching these paths without a linked, approved Resolution Record fails CI and requires a Consultation Request Pack:

- `db/migration/**` (Flyway or Liquibase): schema changes, routed to the data owner
- Security configuration, auth filters and Angular route guards or HTTP interceptors: routed to security
- Public REST contracts (OpenAPI specs, `@RestController` signatures): routed to the API owner
- `pom.xml`, `build.gradle`, `package.json` dependency changes: routed to the platform owner
- `angular.json` budgets or tsconfig strictness lowered: routed to the frontend lead

### CI enforcement

Add one `agentic-gate` job that runs on PRs labeled `agent-authored`:

1. Validate artifact front matter against `.agentic/schemas` (JSON Schema).
2. Require the PR body to reference a Mission Brief ID and contain every MVP Merge-Readiness section.
3. Check that each acceptance property ID in the brief appears in the scope-to-proof map.
4. Check escalation-trigger paths against linked RR IDs with status `approved`.
5. Require human CODEOWNERS approval for any change under `AGENTS.md`, `.agentic/mentorship`, `.agentic/runbooks` or `.agentic/resolutions`.
6. Publish test, lint and scan reports as build artifacts so the merge pack can link them.

### Inside the agent harness

Use harness commands and hooks in whichever agent harness each developer uses to make runbooks executable rather than advisory:

- A pre-edit hook blocks writes to the brief's `forbidden` and do-not-touch paths.
- A stop or pre-submit hook runs the fast gate set (compile, unit tests, lint) and refuses to finish on failure.
- A `/consult` command scaffolds a Consultation Request Pack from the template; a `/ready` command assembles the Merge-Readiness Pack from report files.
- Store commands and hooks in the repo, not in personal config, so they are shared and reviewed.

With mixed harnesses, keep one canonical source and adapt per tool:

- `AGENTS.md` and `.agentic/` are the single source of truth. Each harness's own rule file (`CLAUDE.md`, Copilot instructions, Cursor rules and similar) only points to or is generated from them, never diverges.
- Command and hook definitions describe the gate, not the tool: one script per gate in `scripts/agentic/` (for example `fast-gate.sh`, `scaffold-consult.sh`), called by each harness's hook or command mechanism.
- Harnesses differ in what they can enforce locally. CI's `agentic-gate` job is the backstop that holds for every harness, so no rule may depend on a local hook alone.

## Rollout plan and success measures

Adopt in four phases, each closed by an exit gate, so the team never carries more governance than its agent volume justifies. Pilot on one repository and one team before widening.

1. **Phase 1, Foundations.** Write the root Mentorship Pack (Map, Operating playbook, Engineering intent). Turn on the deterministic gates the stack already supports and raise strictness. Introduce the MVP Mission Brief template.
   - Exit gate: every agent task starts from a brief in the repo, and CI blocks merges on red gates.
2. **Phase 2, Evidence.** Ship the Merge-Readiness Pack as the PR template and add the `agentic-gate` CI job. Record a Resolution Record for every merge review of a higher-risk agent PR.
   - Exit gate: agent PRs carry a complete MVP merge pack, and reviewers audit the scope-to-proof map instead of reading the full diff first.
3. **Phase 3, Escalation and continuity.** Define escalation trigger paths and CODEOWNERS routing. Adopt Consultation Request Packs and Continuity Packs. Make every Resolution Record name its roll-back target.
   - Exit gate: no trigger-path change merges without an approved RR, and review corrections that repeat become Mentorship Pack rules.
4. **Phase 4, Process as code.** Move runbooks into harness commands and hooks, add plan ledgers and machine-readable manifests, and log trigger execution for traceability. For high-risk changes, run independent solutions from two agents and treat disagreement as a risk signal.
   - Exit gate: the team can show, for any merged agent change, which gates ran and which brief, runbook and mentorship versions applied.

### Suggested measures

These indicators are this baseline's suggestions; the book does not prescribe metrics. Track them from Phase 2 onward.

| Measure | What it tells you | Direction |
| --- | --- | --- |
| Agent PRs with a complete MVP merge pack | Evidence discipline | Up, toward 100% |
| Trigger-path changes with an approved RR | Escalation compliance | Must be 100% |
| Repeated review corrections | Whether mentorship is learning | Down |
| Review time per agent PR | Whether review became an audit | Down |
| Rework or revert rate on agent PRs | Real quality of output | Down |
| Escaped defects traced to agent changes | Trust at production level | Down |
| Stale artifacts (no owner or superseded but still referenced) | Artifact debt | Near zero |

## Sources

- Ahmed E. Hassan, [*Agentic Software Engineering: Building Trustworthy Software with Stochastic Teammates at Unprecedented Scale*](https://agenticse-book.github.io/pdf/AgenticSE_Book.pdf), 2026, version 0.5a.
  - Chapter 1, "Agentic SE: Turning Vibes into Trustworthy Engineering": book pages 3–33 (PDF pages 34–64).
  - Appendix A, Reference Tables 1, 1A, 2, 3, 4, 4A and 5: book pages 352–379 (PDF pages 383–410).
  - Appendix B, Glossary: book page 380 onward.
  - Preface and "How to Read This Book": book pages viii–xvi.

The tables in this document are condensed paraphrases of the book's schemas, with MVP and Hardened tiers added by this baseline. The stack mappings, CI design and metrics are this baseline's own proposals.
