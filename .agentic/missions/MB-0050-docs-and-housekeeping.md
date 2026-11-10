# Mission Brief — Documentation matches the system; packs, scaffolds and branches are tidy

```yaml
---
id: MB-0050
owner: "@juuso-kalliomaki"
goal: "A newcomer can clone the repo, create the first account and find their way from documents that describe the system as it is, and the project's own records and working copies carry no stale state."
non_goals:
  - "Changing any rule's substance: Mentorship Pack and AGENTS.md edits here only correct descriptions (an RR approves each)."
  - "Rewriting `deploy/README.md`'s procedures."
  - "Deleting anything unmerged without the owner confirming it."
acceptance:
  - id: P1
    property: "`README.md` describes the current app: status, invite-only registration with the `kauppa-web admin` steps to create the first account locally, languages, deployment pointer, and a layout that includes `locales/`, `deploy/` and `scripts/deploy/`."
    check: "manual: a fresh clone following only README.md reaches a signed-in session; steps recorded in MB-0050's Continuity Pack"
  - id: P2
    property: "`AGENTS.md` is corrected under an RR: sessions are cookie-stored, the repo trait list is complete, the Map names `locales/`, `deploy/` and the `i18n` module, the review date is current, and the baseline link resolves for someone who has only this repository."
    check: "manual: the RR roll-back applied, recorded in MB-0050's Continuity Pack"
  - id: P3
    property: "The task-class packs are corrected under the same RR: `templating.md` no longer points custom JS at `assets/js/` against AGENTS.md; `actix-web.md` describes `AppState`, the `Ui` extractor, `ui.render` and the `Vary` header as they are; `Cargo.toml`'s comments on disabled actix features cite advisories that still apply."
    check: "manual: the RR roll-back applied, recorded in MB-0050's Continuity Pack"
  - id: P4
    property: "`deploy/README.md` states current requirements instead of history (no \"after MB-0027\u2026 until then browsers load htmx from unpkg\", no gates on briefs that merged), keeping every procedure and RR reference that still applies."
    check: "scripts/deploy/check_no_secrets.sh"
  - id: P5
    property: "The Continuity Packs of MB-0028, MB-0033, MB-0037, MB-0038, MB-0039 and MB-0040 record their merged PRs and end state; MB-0028 and MB-0040 list the owner's manual steps still open; MB-0035 is closed out once #63 merges."
    check: "python scripts/agentic/validate_artifacts.py"
  - id: P6
    property: "`scripts/agentic/scaffold-mission.sh` creates the brief's Continuity Pack from the template with state \"not started\", as AGENTS.md requires."
    check: "scripts/agentic/test_check_triggers.sh"
  - id: P7
    property: "Merged remote and local branches, finished worktrees and obsolete stashes are removed after the owner confirms a list, and `.gitignore` covers the agent's local directory (`.claude/`)."
    check: "manual: before and after counts recorded in MB-0050's Continuity Pack"
context:
  - README.md
  - AGENTS.md
  - .agentic/README.md
  - .agentic/mentorship/templating.md
  - .agentic/mentorship/actix-web.md
  - deploy/README.md
  - Cargo.toml
  - .agentic/continuity/
  - scripts/agentic/scaffold-mission.sh
  - .gitignore
autonomy:
  may_decide:
    - "wording, inside what each document already decides"
    - "where the scaffold test lives (P6's check may name a new script; record the rename here)"
    - "closing out packs (agent-owned)"
  escalate:
    - { when: "every edit to `AGENTS.md` or a Mentorship Pack, as one CRP with the exact text", to: owner }
    - { when: "where the baseline document lives (copied into the repo, linked publicly, or the reference dropped)", to: owner }
    - { when: "which branches, worktrees and stashes may be deleted (one stash predates this work and is not the agent's)", to: owner }
    - { when: "`Cargo.toml` comment edits (trigger path)", to: platform-owner }
    - { when: "`.gitignore` and anything under `scripts/agentic/` (CODEOWNERS)", to: platform-owner }
  forbidden:
    - "change a rule's meaning while correcting its description"
    - "delete an unmerged branch, a worktree with uncommitted work, or a stash without the owner's confirmation"
    - "edit RRs"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
  - .agentic/mentorship/actix-web.md@v1
resolutions: []
---
```

## Context

Found in the audit of 2026-11-10 (findings 8, 11, 13 and 21–26).

- `README.md` says "Scaffold … empty list and recipe pages" and gives no
  way to create a first account (registration is invite-only since MB-0023).
- `AGENTS.md` says "server-side sessions" (they are in a cookie), lists
  three repo traits of seven, lacks `locales/`, `deploy/` and `i18n` in its
  Map, was last reviewed 2026-10-28, and links the baseline outside the
  repo (`../Agentic SE Implementation Baseline.md`).
- `templating.md` sends custom JS to `assets/js/`, which AGENTS.md forbids.
  `actix-web.md` says repo trait objects live in `AppState`, reserves
  Playwright tests that don't exist (MB-0047), and predates `Ui`.
- `deploy/README.md` is phrased as history in places.
- `Cargo.toml` justifies disabled actix features with advisories whose
  current status wasn't checked.
- Six packs say "not pushed" or "PR not opened" though their PRs (#47, #48,
  #50–#53) merged on 2026-11-10.
- `scaffold-mission.sh` doesn't create a Continuity Pack.
- 64 remote branches (57 merged), 97 local branches, 37 worktrees, 3
  stashes; `.claude/` is untracked and not ignored.

## Conceptual plan

1. Close out the packs and fix the scaffold (no consult needed for packs).
2. **Clarify (S1).** One CRP with the proposed text for AGENTS.md and the
   packs, the baseline question and the deletion list.
3. README and deploy README; then the roll-back and the cleanup.

## Validation plan

- A fresh-clone walkthrough of README.md.
- The artifact validator; `git branch -r | wc -l` before and after.
