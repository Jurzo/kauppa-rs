# Mission Brief — Make the review gates real: CODEOWNERS, trigger paths, required checks

```yaml
---
id: MB-0043
owner: "@juuso-kalliomaki"
goal: "The governance rules the repo documents are enforced by GitHub: CODEOWNERS names an account that exists, the escalation trigger paths cover all security-relevant code, and every check that guards a merge is a required status check."
non_goals:
  - "Changing who owns decisions (one owner stays)."
  - "Rewriting the runbooks' workflow."
  - "Adding new kinds of checks (MB-0045 covers CI content)."
acceptance:
  - id: P1
    property: "Every owner in `.github/CODEOWNERS` is a GitHub account or team with write access to the repo, and a test fails if the file names an owner the configured list doesn't contain."
    check: "scripts/agentic/test_check_triggers.sh"
  - id: P2
    property: "The trigger list in `check_triggers.py`, the escalation list in `security.md` and the runbooks, and CODEOWNERS name the same paths, including `crates/kauppa-web/src/csrf/`, `csp.rs`, `config.rs`, `i18n/request.rs`, `routes/language.rs` and `deploy/`, or the RR records why one is left out; a test compares the three lists."
    check: "scripts/agentic/test_check_triggers.sh"
  - id: P3
    property: "Branch protection on `main` requires the image job, the secrets check and the brief-test check alongside today's five, and the settings are recorded in the repo so drift is visible."
    check: "manual: `gh api repos/Jurzo/kauppa-rs/branches/main/protection` output recorded in MB-0043's Continuity Pack"
  - id: P4
    property: "How merges are approved is written down and matches practice: whether the agent may merge with the owner's account after a chat approval, or a GitHub review is required (`required_approving_review_count`, `enforce_admins`)."
    check: "manual: the RR's decision applied and recorded in MB-0043's Continuity Pack"
context:
  - .github/CODEOWNERS
  - scripts/agentic/check_triggers.py
  - scripts/agentic/test_check_triggers.sh
  - .agentic/runbooks/feature.md
  - .agentic/mentorship/security.md
  - .github/workflows/agentic-gate.yml
  - .github/workflows/deploy.yml
autonomy:
  may_decide:
    - "how the three path lists are compared in the test"
    - "the format of the recorded branch-protection settings"
  escalate:
    - { when: "which account or team CODEOWNERS names, and the owner handle used in briefs, runbooks and RRs (`@juuso-kalliomaki` vs `@juuso.kalliomaki` vs the GitHub login)", to: owner }
    - { when: "which paths become trigger paths (each one adds an RR requirement to future PRs)", to: security }
    - { when: "the merge-approval rule and any branch-protection change (only the owner can apply it)", to: owner }
    - { when: "any edit to a Runbook, Mentorship Pack or CODEOWNERS", to: owner }
  forbidden:
    - "change branch protection or repository settings without the owner doing or approving each change"
    - "weaken an existing trigger path"
    - "edit RRs"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/security.md@v1
resolutions: []
---
```

## Context

Found in the audit of 2026-11-10 (findings 1, 4 and part of 6).

- `.github/CODEOWNERS` names `@juuso-kalliomaki` on every line. The GitHub
  API answers 404 for that user; the repository owner is `Jurzo`, and RRs
  are signed `@juuso.kalliomaki`. Branch protection has
  `require_code_owner_reviews: true` with
  `required_approving_review_count: 0` and `enforce_admins: false`. In
  practice the agent merges with `gh pr merge` under the owner's account
  after approval in chat, so the RR check is the only enforced gate.
- The trigger list (`check_triggers.py`) and CODEOWNERS predate `csrf/`
  (RR-0010), `csp.rs` (RR-0019), the `kauppa_lang` cookie
  (`i18n/request.rs`, RR-0023), `routes/language.rs` and `deploy/`.
  `security.md` names "cookie flags in `config.rs`", which neither list
  covers.
- Required status checks are `cargo deny`, `fmt / clippy / test`, the
  artifact validator, Merge-Readiness evidence and trigger paths. The image
  build and smoke tests, and the secrets check, are not required.

## Conceptual plan

1. **Clarify (S1).** CRP to the owner (account, approval rule, protection)
   and security (trigger paths).
2. Extend `test_check_triggers.sh` with the list comparison; update the
   lists under the RR.
3. The owner applies the GitHub settings; record them in the pack.

## Validation plan

- P1, P2: the script's new cases, run in CI once MB-0045 hooks it in.
- P3, P4: API output and the RR.
