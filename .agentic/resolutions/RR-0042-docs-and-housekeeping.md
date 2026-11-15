# Resolution Record — Documentation corrections, the baseline copied in, a scaffold that creates the pack, and the cleanup list

```yaml
---
id: RR-0042
closes: CRP-0042
mission: MB-0050
decided_by: "@Jurzo"
role: platform
risk_tier: low
decided_at: 2026-11-15T07:05:00Z
status: approved
superseded_by: null
rolls_back_into:
  - AGENTS.md
  - .agentic/mentorship/templating.md
  - .agentic/mentorship/actix-web.md
  - .agentic/missions/MB-0050-docs-and-housekeeping.md
  - .agentic/consults/CRP-0042-docs-and-housekeeping.md
---
```

## Decision and outcome

Approve CRP-0042 Q1, Q3, Q4, Q5 and Q6 as recommended, and Q2 **A**.

Sub-answers:

1. **`AGENTS.md` (Q1, owner).** Items 1–4 of CRP-0042 → Proposed text,
   verbatim: the review date, cookie-stored sessions, the Translations and
   Deployment rows in the Map, the seven repo traits.
2. **The baseline (Q2 A, owner).** The owner's "Agentic SE Implementation
   Baseline" is copied byte for byte into `docs/agentic-se-baseline.md`.
   `AGENTS.md` line 5-6 links it there and adds: "It was written for a
   Java and TypeScript team; where it and this repository's runbooks or
   packs differ, the repository wins."
3. **Mentorship Packs (Q3, owner).** `templating.md` and `actix-web.md` as
   CRP-0042 → Proposed text → Q3 says, verbatim. `actix-web.md`'s
   Playwright line is left to RR-0041.
4. **`Cargo.toml` (Q4, platform).** The comment as CRP-0042 → Q4 says;
   no feature changes.
5. **Scaffold and `.gitignore` (Q5, platform).** `scaffold-mission.sh`
   also creates the Continuity Pack as CRP-0042 → Q5 describes, and
   refuses to overwrite one; `scripts/agentic/test_scaffold_mission.sh` is
   P6's check and runs in the `agentic scripts` CI job; `.gitignore` gains
   `/.claude/`.
6. **Cleanup (Q6, owner).** Delete the remote branches, local branches and
   worktrees CRP-0042 → Q6 lists, plus `agentic/mb-0041-sqlite-fix` (#98,
   which carries its commits, is merged) and any other branch whose PR has
   merged by then, each re-checked with `git cherry` at deletion time and
   its tip recorded in MB-0050's pack. Drop all three stashes, `stash@{1}`
   and `stash@{2}` included. Never delete a branch with an open PR, a
   worktree with uncommitted changes, or anything unmerged not on the list.

Covers:

- MB-0050's remaining PRs (P1–P4, P6) and the P7 cleanup;
- the roll-backs below.

Does not cover:

- any change to a rule's meaning, or enabling `compress-zstd`;
- edits to the baseline's content once copied (a later decision).

## Rationale and trade-offs

- **Corrections only**: each line was checked against the code it
  describes (CRP-0042 → Current state).
- **Baseline in the repository**: anyone with a clone can read what
  `AGENTS.md` builds on; the owner chose to publish it with the
  repository.
- **Cleanup with tips recorded**: a deleted branch is recreated from its
  recorded tip if anything turns out to be missing.

## Approvals and authority

@Jurzo as owner for sub-answers 1, 2, 3 and 6; as platform for sub-answers 4 and 5.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0042-docs-and-housekeeping.md`
- Mission Brief: `.agentic/missions/MB-0050-docs-and-housekeeping.md`
- Evidence: CRP-0042 → Evidence bundle.

## Constraints and follow-on rules (Hardened)

1. A description in `AGENTS.md` or a Mentorship Pack that stops matching
   the code is corrected through a CRP with the exact text, as here.
2. New briefs come with their pack from the scaffold; the self-test keeps
   it so.

## Roll-back into the system (Hardened)

To be applied after this record is signed, with the owner's OK:

- `AGENTS.md`: sub-answers 1 and 2.
- `docs/agentic-se-baseline.md`: the copy (sub-answer 2).
- `.agentic/mentorship/templating.md`, `.agentic/mentorship/actix-web.md`:
  sub-answer 3.
- `MB-0050`: `resolutions: [RR-0042]`; P6's check →
  `scripts/agentic/test_scaffold_mission.sh`; plan step 2 marked done.
- `CRP-0042`: `state: open` → `answered`.

`Cargo.toml`, the scaffold and `.gitignore` change in MB-0050's
implementation PR (sub-answers 4 and 5).

## Supersession and history

Status: active once signed. Supersedes nothing.
