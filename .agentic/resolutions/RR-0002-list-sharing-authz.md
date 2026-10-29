# Resolution Record — Shared-list access model & authorization

> **DRAFT SKELETON — resolver to complete.** The mechanical/linking fields are
> filled and the recommendation from CRP-0002 is pre-drafted below for quick
> sign-off. This RR intentionally does **not** validate yet: `status`,
> `decided_by`, and `decided_at` are placeholders because authoring the decision
> is the resolver's role, not the agent's. Set those three (and edit the body)
> to finalize; until then `validate_artifacts.py` will flag exactly those fields.

```yaml
---
id: RR-0002
closes: CRP-0002
mission: MB-0008
decided_by: "@juuso.kalliomaki"
role: security
risk_tier: high
decided_at: 2026-10-29T00:00:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/mentorship/security.md
  - .agentic/runbooks/feature.md
---
```

## Decision and outcome

Approve Option A from CRP-0002 — sharing a
list with an existing user by email, with `view` / `edit` roles and owner-only
share / revoke / delete. Scope of approval: the `list_shares` data model and the
relaxation of the `owner_id = $current_user` authorization invariant to
"owner OR grantee with sufficient role" for shopping-list and list-item
resources only. Does **not** cover recipe sharing, capability share-links, or a
household/group model.

## Rationale and trade-offs

Option A extends the existing session-based authz with the smallest new attack
surface, gives clean immediate revocation, and matches the brief's "another
user" wording. Rejected: Option B (capability link) — the token becomes a
leakable credential and breaks the session/CSRF model; Option C (household/group)
— far larger scope than the nice-to-have warrants.

## Approvals and authority

Resolver: @juuso.kalliomaki, role: security (per the AGENTS.md decision-rights
table, Auth/sessions → @juuso-kalliomaki). Risk tier is **high** — if a
co-approver is required for high-risk security changes, record them here.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0002-list-sharing-authz.md`
- Mission Brief: `.agentic/missions/MB-0008-*.md` (to be scaffolded on approval)
- Evidence links: per-route authz tests proving a `view` grantee can read but
  not mutate, an `edit` grantee can mutate but not re-share, and a revoked
  grantee is denied on the next request.

## Constraints and follow-on rules (Hardened)

These carry over from CRP-0002 regardless of wording and bind MB-0008:

1. **Centralized predicate.** One authz helper computes effective access
   (owner, or share row with role ≥ required); every list / list-item read and
   write goes through it. A query filtering on `owner_id` alone is a review
   failure.
2. **Role semantics.** `view` = read-only; `edit` = add / check / remove but not
   re-share and not delete the list; only the owner shares, revokes, deletes.
3. **Enumeration-safe invites.** Sharing to an unknown email returns the same
   generic result as success — never reveal whether an account exists.
4. **Revocation is immediate.** Deleting a share row removes access on the next
   request; no cached grants.
5. **Audit & logging.** Log grant / revoke with grantor and list id; never the
   grantee's full email at `info` or higher.
6. **CSRF.** Grant / revoke / edit are state-changing POSTs and must ride the
   app's CSRF protection — resolver to confirm whether this feature promotes the
   `security.md` CSRF "should" to a merge-blocking "must".

## Roll-back into the system (Hardened)

On approval, update:

- `.agentic/mentorship/security.md` — amend the Authorization rule to the
  "owner OR grantee" predicate and record the centralized-helper requirement.
- `.agentic/runbooks/feature.md` — add shared-list authorization in
  `crates/kauppa-web/src/routes/lists.rs` to the security escalation-trigger
  paths.
- Link the PR that applies these roll-backs here.

## Supersession and history

Status: active. Superseded by: none.
