# Resolution Record — Review gates: CODEOWNERS account, trigger paths, required checks, merge approval

```yaml
---
id: RR-0027
closes: CRP-0027
mission: MB-0043
decided_by: "@Jurzo"
role: security
risk_tier: medium
decided_at: 2026-11-12T08:00:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/runbooks/feature.md
  - .agentic/mentorship/security.md
  - .agentic/README.md
  - .agentic/missions/MB-0043-enforce-review-gates.md
  - .agentic/consults/CRP-0027-review-gates.md
---
```

## Decision and outcome

Approve CRP-0027's **recommended set, Q1–Q4**, with merge approval
option **(a)**.

Sub-answers:

1. **Account (Q1).** `.github/CODEOWNERS` names `@Jurzo` on every line.
   Artifacts keep `@juuso-kalliomaki` as the owner label; `.agentic/README.md`
   states that the label means the GitHub account `Jurzo`.
2. **Trigger paths (Q2).** These join the escalation trigger paths, in
   `check_triggers.py`, the feature runbook's table, CODEOWNERS and (for
   the security ones) `security.md`'s escalation list:
   - security: `crates/kauppa-web/src/csrf/`,
     `crates/kauppa-web/src/csp.rs`, `crates/kauppa-web/src/config.rs`,
     `crates/kauppa-web/src/i18n/request.rs`,
     `crates/kauppa-web/src/routes/language.rs`;
   - platform (deployment): `deploy/Dockerfile`, `deploy/compose.yaml`,
     `deploy/Caddyfile`, `deploy/docker-entrypoint.sh`,
     `deploy/terraform/`, `deploy/backup/`.
   `deploy/README.md` is not a trigger path. A test keeps the lists equal.
3. **Required checks (Q3).** `image build, smoke tests, push (main)` and
   `no secrets committed` become required status checks on `main`. The
   brief-test check joins them when MB-0045 adds it to CI.
4. **Merge approval (Q4 (a)).** The owner approves a merge in chat or in
   GitHub. The agent may then merge with the owner's account. The enforced
   gates are the required checks and the RR check. `enforce_admins` is set
   to `true`, so no account bypasses the checks. The feature runbook says
   so.

The owner applies the GitHub settings in sub-answers 3 and 4; the agent
supplies the commands and records the before and after in MB-0043's pack.

Covers:

- MB-0043's implementation PR, P1–P4;
- the edits to CODEOWNERS, `check_triggers.py`, `test_check_triggers.sh`,
  the feature runbook, `security.md` and `.agentic/README.md` listed
  below.

Does not cover:

- renaming the owner label in existing artifacts or RRs;
- a second GitHub identity or required approving reviews (option (c));
- any other branch-protection change;
- removing or narrowing an existing trigger path.

## Rationale and trade-offs

- **`@Jurzo`.** It is the account that exists. Renaming 40-odd artifacts
  buys nothing; one mapping line removes the ambiguity.
- **Trigger paths.** The list predates the cross-site check, the CSP, the
  language cookie and the deployment. The cost is an RR for more PRs,
  which is the purpose of the list.
- **Required checks.** Both already run on every PR; making them required
  costs nothing and closes the gap where a broken image could merge.
- **(a) with `enforce_admins`.** It matches how the project works with one
  owner, and makes the checks binding for every account.
  - **(b) owner merges by hand, not chosen:** clearer separation, more
    manual work.
  - **(c) second identity, not chosen:** the only way to require a review,
    but new accounts and tokens to manage.

## Approvals and authority

Approved by the owner (Q1, Q3, Q4, and the Runbook and README edits) and
security-owner (Q2 and the `security.md` edit); platform-owner for the
deployment trigger paths.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0027-review-gates.md`
- Mission Brief: `.agentic/missions/MB-0043-enforce-review-gates.md`
- Related decisions: RR-0010, RR-0019, RR-0023 (the code the new security
  paths hold); RR-0014 (deployment); RR-0012 (how `routes/auth` is
  matched).
- Evidence links: CRP-0027 → Evidence bundle (the 404 for the named
  account; the protection settings of 2026-11-10).

## Constraints and follow-on rules (Hardened)

1. **Three lists, one content.** `check_triggers.py`, the feature
   runbook's table and CODEOWNERS name the same trigger paths; the test
   fails when they differ.
2. **CODEOWNERS names real accounts.** The test fails on an owner outside
   the configured list.
3. **New security-relevant code joins the list** in the PR that adds it:
   anything that sets cookies or headers, checks origin or identity, or
   reads security settings.
4. **Checks bind everyone.** `enforce_admins` stays `true`; turning it off
   needs an RR.
5. **The agent merges only after the owner's approval of that PR**, never
   past a failing or missing required check.

## Roll-back into the system (Hardened)

To be applied in MB-0043's implementation PR:

- `.agentic/runbooks/feature.md` → Escalation triggers table: add rows
  - "Cross-site check, security headers, settings and the language cookie
    in `crates/kauppa-web/src/csrf/`, `csp.rs`, `config.rs`,
    `i18n/request.rs`, `routes/language.rs` | @juuso-kalliomaki (security)"
  - "Deployment: `deploy/Dockerfile`, `deploy/compose.yaml`,
    `deploy/Caddyfile`, `deploy/docker-entrypoint.sh`,
    `deploy/terraform/**`, `deploy/backup/**` | @juuso-kalliomaki (platform)"

  and add after the table: "Merges: the owner approves each merge, in chat
  or in GitHub. The agent may then merge with the owner's account, never
  past a failing or missing required check (RR-0027)."
- `.agentic/mentorship/security.md` → Escalation list: add
  "`crates/kauppa-web/src/csrf/`, `csp.rs`, `i18n/request.rs` and
  `routes/language.rs` (cross-site check, security headers, the language
  cookie; RR-0027)" and change "Cookie flags in `config.rs`" to
  "`crates/kauppa-web/src/config.rs`".
- `.agentic/README.md` → after the table: "The owner label
  `@juuso-kalliomaki` in these artifacts is the GitHub account `Jurzo`
  (RR-0027)."
- `MB-0043`: `resolutions: []` → `[RR-0027]`; plan step 1 gains "Answered
  by RR-0027."
- `CRP-0027`: `state: open` → `answered`.

## Supersession and history

Status: active once approved. Supersedes nothing; extends the trigger list
first set in the feature runbook.

Amended by RR-0030 (2026-11-12): `enforce_admins` stays `false` while
code-owner reviews are required (sub-answer 4); constraint 4 is replaced by
RR-0030's "No bypass by practice".
