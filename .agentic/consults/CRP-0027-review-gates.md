# Consultation Request Pack — Making the review gates real: CODEOWNERS, trigger paths, required checks, merge approval

```yaml
---
id: CRP-0027
mission: MB-0043
owner: "@juuso-kalliomaki"
target_role: security
risk_tier: medium
urgency: medium
state: open
runbook_trigger: "`.agentic/README.md` rule 1 and CODEOWNERS: edits to Runbooks, Mentorship Packs, `.github/` and `scripts/agentic/`; MB-0043 autonomy.escalate (account, trigger paths, approval rule → owner, security)"
---
```

## Decision statement

Decide which GitHub account owns the protected paths, which paths require
a Resolution Record, which checks block a merge, and how a merge is
approved.

## Current state

- `.github/CODEOWNERS` names `@juuso-kalliomaki` on every line.
  `gh api users/juuso-kalliomaki` answers 404. The repository belongs to
  `Jurzo`. RRs are signed `@juuso.kalliomaki`; briefs and runbooks say
  `@juuso-kalliomaki`.
- Branch protection on `main`: `require_code_owner_reviews: true`,
  `required_approving_review_count: 0`, `enforce_admins: false`, strict
  up-to-date branches, five required checks (`cargo deny`,
  `fmt / clippy / test`, `validate .agentic/ artifacts`,
  `Merge-Readiness evidence`, `Escalation trigger paths`).
- In practice the agent runs `gh pr merge` with the owner's credentials
  after the owner approves in chat. PRs are authored by the same account,
  which GitHub doesn't let approve its own PR.
- `check_triggers.py` requires an approved RR for: `migrations/`,
  `password.rs`, `routes/auth`, `routes/admin/`, `admin/`, `main.rs`,
  `Cargo.toml` (root and crates), `rust-toolchain.toml`, `rustfmt.toml`,
  `deny.toml`, `.agentic/{mentorship,runbooks,resolutions}`.
- Not covered by triggers or CODEOWNERS: `csrf/` (RR-0010), `csp.rs`
  (RR-0019), `config.rs` (named in `security.md`'s escalation list),
  `i18n/request.rs` (the `kauppa_lang` cookie, RR-0023),
  `routes/language.rs`, and everything under `deploy/`.

## Options and trade-offs

### Q1 — The account

- **Recommended:** CODEOWNERS names `@Jurzo`, the account that exists and
  owns the repo. Artifacts keep `@juuso-kalliomaki` as the owner label
  (42 briefs, runbooks and packs use it; the schema only needs an
  `@handle`), and `.agentic/README.md` gains one line saying the label
  maps to the GitHub account `Jurzo`.
- Alternative: rename every artifact to `@Jurzo`. Consistent, but a large
  mechanical edit of human-owned files, and RRs already signed differ.

### Q2 — Trigger paths to add

**Recommended (security):** `crates/kauppa-web/src/csrf/`,
`crates/kauppa-web/src/csp.rs`, `crates/kauppa-web/src/config.rs`,
`crates/kauppa-web/src/i18n/request.rs`,
`crates/kauppa-web/src/routes/language.rs`.

**Recommended (platform, deployment):** `deploy/Dockerfile`,
`deploy/compose.yaml`, `deploy/Caddyfile`, `deploy/docker-entrypoint.sh`,
`deploy/terraform/`, `deploy/backup/`. Not `deploy/README.md`: a doc fix
shouldn't need an RR.

- Cost: every future PR touching these needs an approved RR in its body.
  `config.rs` and `main.rs` are touched often for wiring; that friction is
  the point, but it is real.
- Tests under those paths (`csrf/tests/`) would also trigger, as
  `routes/auth/tests/` does today.

The same paths go into CODEOWNERS and `security.md`'s escalation list, and
a test keeps the three lists equal (MB-0043 P2).

### Q3 — Required checks

- **Recommended:** add `image build, smoke tests, push (main)` and
  `no secrets committed` to the required checks. Both already run on every
  PR; the image job reports "skipped" when no image input changed, which
  GitHub counts as passing.
- Add the brief-test check when MB-0045 puts it in CI.

### Q4 — How a merge is approved

- **(a) (recommended): write down today's practice.** The owner approves
  in chat or in GitHub; the agent may then merge with the owner's account;
  the enforced gates are the required checks and the RR check. The feature
  runbook gets one paragraph saying so. `enforce_admins` becomes `true`,
  so the checks can't be bypassed even by the owner's account.
- (b) The agent never merges; the owner merges in GitHub. A clearer
  separation, at the cost of the owner doing every merge by hand.
- (c) Require one approving review. Not possible with one account: the PR
  author can't approve. It would need a second GitHub account or a bot
  identity for the agent's PRs.

## Recommendation and question

Recommended: Q1 `@Jurzo` in CODEOWNERS with a mapping line; Q2 the five
security paths and the six deployment paths; Q3 two more required checks;
Q4 (a) with `enforce_admins: true`.

Crisp questions:

1. CODEOWNERS → `@Jurzo`, artifacts keep `@juuso-kalliomaki` with a
   mapping line?
2. Add the five security paths and the six deployment paths as trigger
   paths (and to CODEOWNERS and `security.md`)?
3. Make the image job and the secrets check required?
4. Merge approval: (a) the agent merges after your approval, with
   `enforce_admins` on; (b) only you merge; or (c) a second identity?

Only you can apply the GitHub settings (Q3, Q4); I will give the exact
`gh api` commands and record the result in the pack.

## Blast radius and rollback

What can break:
- A wrong CODEOWNERS entry or a required check that never reports blocks
  every PR until fixed in the GitHub settings.
- More PRs need an RR.

Who is affected: the owner and the agent; no user-facing change.

Rollback plan: revert the PR; restore the previous protection settings
(recorded in the pack before the change).

## Minimal context

- MB-0043 P1–P4.
- Files: `.github/CODEOWNERS`, `scripts/agentic/check_triggers.py`,
  `scripts/agentic/test_check_triggers.sh`,
  `.agentic/runbooks/feature.md`, `.agentic/mentorship/security.md`,
  `.agentic/README.md`.

## Evidence bundle (Hardened)

- `gh api users/juuso-kalliomaki` → 404 (2026-11-10).
- `gh api repos/Jurzo/kauppa-rs/branches/main/protection` (2026-11-10):
  the values quoted under Current state.
- `scripts/agentic/check_triggers.py:24-39` (the trigger list);
  `.github/CODEOWNERS`; `security.md` → Escalation.
- PRs #47–#67 were merged by `gh pr merge` under the owner's account with
  no GitHub review.
