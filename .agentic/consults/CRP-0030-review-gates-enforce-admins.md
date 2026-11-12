# Consultation Request Pack — Leave `enforce_admins` off while code-owner reviews are on (amend RR-0027)

```yaml
---
id: CRP-0030
mission: MB-0043
owner: "@juuso-kalliomaki"
target_role: security
risk_tier: low
urgency: medium
state: open
runbook_trigger: "RR-0027 constraint 4 (`enforce_admins` stays `true`; turning it off needs an RR) conflicts with the owner's choice of 2026-11-12"
---
```

## Decision statement

Record that `enforce_admins` stays `false` on `main`, against RR-0027
sub-answer 4 and constraint 4, because with code-owner reviews required it
would block the owner's own PRs.

## Current state

- RR-0027 (signed 2026-11-12) set CODEOWNERS to `@Jurzo` and, in
  sub-answer 4, `enforce_admins: true`; constraint 4 says it stays `true`.
- Branch protection keeps `require_code_owner_reviews: true` with
  `required_approving_review_count: 0`.
- While CODEOWNERS named a non-existent account, GitHub found no valid code
  owner and the review requirement asked for nothing. With `@Jurzo` as
  code owner, a PR touching an owned path needs `@Jurzo`'s approval, and
  GitHub doesn't let a PR's author approve it. The agent's PRs are opened
  under the owner's account, so only the admin bypass lets them merge.
  Turning on `enforce_admins` would remove that bypass (inferred from
  GitHub's rules, not tested).
- The owner chose option (ii) on 2026-11-12: keep code-owner reviews on,
  leave `enforce_admins` off. The two extra required checks (RR-0027
  sub-answer 3) are applied and verified.

## Options and trade-offs

### Option A — (i) code-owner reviews off, `enforce_admins` on

- Pros: the required checks bind every account, as RR-0027 intended.
- Cons: CODEOWNERS only routes review requests; a branch-protection change
  RR-0027 didn't cover.
- Risk: low.

### Option B — (ii) keep reviews on, `enforce_admins` off (chosen)

- Pros: no further setting change; the owner can always merge.
- Cons: the owner's admin account can merge past failing or missing
  checks. The rule against it is the feature runbook's merges paragraph,
  followed by the agent, not enforced by GitHub.
- Risk: low with one owner who follows the runbook.

### Option C — (iii) a second identity for the agent's PRs

- Pros: real reviews, and `enforce_admins` could be on.
- Cons: a second account or bot and its tokens to manage.

## Recommendation and question

Recommended: B, as the owner chose.

Crisp question to answer: amend RR-0027 so that `enforce_admins` stays
`false` while `require_code_owner_reviews` is `true`, and replace
constraint 4 with a rule the agent and owner follow by practice?

## Blast radius and rollback

What can break: nothing changes in GitHub; the record changes to match it.
Who is affected: the owner and the agent.
Rollback plan: a later RR choosing A or C.

## Minimal context

- RR-0027 sub-answer 4 and constraint 4; MB-0043 P4.
- `.agentic/runbooks/feature.md` merges paragraph (RR-0027 roll-back).

## Evidence bundle (Hardened)

- `gh api repos/Jurzo/kauppa-rs/branches/main/protection` (2026-11-12):
  seven required checks, `strict: true`, `enforce_admins: false`,
  `require_code_owner_reviews: true`, `required_approving_review_count: 0`.
- MB-0043's Continuity Pack: the before-snapshot and the blocker analysis.
