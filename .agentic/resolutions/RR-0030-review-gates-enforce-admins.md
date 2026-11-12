# Resolution Record — Amend RR-0027: `enforce_admins` stays off while code-owner reviews are required

```yaml
---
id: RR-0030
closes: CRP-0030
mission: MB-0043
decided_by: "@Jurzo"
role: security
risk_tier: low
decided_at: 2026-11-12T08:40:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/resolutions/RR-0027-review-gates.md
  - .agentic/runbooks/feature.md
  - .agentic/missions/MB-0043-enforce-review-gates.md
  - .agentic/consults/CRP-0030-review-gates-enforce-admins.md
---
```

## Decision and outcome

Approve CRP-0030 **Option B** (the owner's choice of 2026-11-12, option
(ii)).

1. **Settings.** On `main`, `require_code_owner_reviews` stays `true` and
   `enforce_admins` stays `false`. RR-0027 sub-answer 4's
   "`enforce_admins` is set to `true`" no longer applies. The other
   settings are as RR-0027 sub-answer 3 left them: seven required checks,
   `strict: true`.
2. **Constraint 4 of RR-0027 is replaced** by: "**No bypass by practice.**
   The owner's admin account can merge past a failing or missing required
   check; neither the owner nor the agent does so. The agent merges only
   after the owner's approval of that PR and only when every required
   check has passed. Turning `enforce_admins` on, or code-owner reviews
   off, needs an RR."

Covers:

- the RR-0027 amendment note, the feature-runbook sentence and the brief
  changes below, in MB-0043's PR (#69);
- no GitHub setting change.

Does not cover:

- options A (code-owner reviews off, `enforce_admins` on) or C (a second
  identity);
- any other branch-protection change.

## Rationale and trade-offs

- With one account, required code-owner reviews and `enforce_admins`
  together would block the owner's own PRs: a PR's author can't approve
  it. Keeping the admin bypass is the least change.
- The cost is that the checks bind by practice, not by GitHub. The runbook
  already says the agent never merges past a failing or missing check.
- **A, not chosen:** binds the checks for every account, but drops
  code-owner reviews.
- **C, not chosen:** a second identity to manage.

## Approvals and authority

Approved by security-owner (the branch-protection rule) and the owner (the
runbook sentence).

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0030-review-gates-enforce-admins.md`
- Mission Brief: `.agentic/missions/MB-0043-enforce-review-gates.md`
- Amends: RR-0027 (sub-answer 4, constraint 4).
- Evidence: CRP-0030 → Evidence bundle (protection settings of
  2026-11-12).

## Constraints and follow-on rules (Hardened)

1. **No merge past a failing or missing required check**, by any account.
2. **Settings change only by RR.** Turning `enforce_admins` on or
   code-owner reviews off needs an RR.

## Roll-back into the system (Hardened)

To be applied in MB-0043's PR (#69):

- `RR-0027` → "Supersession and history": add "Amended by RR-0030
  (2026-11-12): `enforce_admins` stays `false` while code-owner reviews are
  required (sub-answer 4); constraint 4 is replaced by RR-0030's 'No bypass
  by practice'."
- `.agentic/runbooks/feature.md` → the merges paragraph: append "GitHub
  doesn't enforce this for the owner's admin account (`enforce_admins` is
  off, RR-0030); neither the owner nor the agent merges past a failing or
  missing required check."
- `MB-0043`: `resolutions: [RR-0027]` → `[RR-0027, RR-0030]`.
- `CRP-0030`: `state: open` → `answered`.

## Supersession and history

Status: active once approved. Amends RR-0027.
