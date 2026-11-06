# Resolution Record — Defer CSRF protection for list sharing to MB-0018

```yaml
---
id: RR-0008
closes: CRP-0008
mission: MB-0008
decided_by: "@juuso.kalliomaki"
role: security
risk_tier: high
decided_at: 2026-11-06T21:16:42Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0008-share-list.md
  - .agentic/missions/MB-0018-cross-site-request-protection.md
  - .agentic/consults/CRP-0008-share-csrf-origin-check.md
---
```

## Decision and outcome

Close CRP-0008 with **Option D for MB-0008**:

- MB-0008 merges with no CSRF control, relying on `SameSite=Lax` as today.
- MB-0008 P8 (origin check on share and revoke) moves to MB-0018 as its P5.
- MB-0018 sets up production-ready, app-wide protection. It picks the
  mechanism in its own CRP; the options are CRP-0008's B, C or both.

Covers:

- amending MB-0008's acceptance to drop P8;
- RR-0002 constraint 6 ("grant / revoke / edit must ride the app's CSRF
  protection") being met by MB-0018, not by MB-0008.

Does not cover:

- choosing the CSRF mechanism (MB-0018's CRP);
- promoting `security.md`'s CSRF rule to a "must" (MB-0018's RR);
- any cookie or session change.

## Rationale and trade-offs

- **D now, app-wide later — chosen.** A two-endpoint origin check (Option A)
  would leave every other write unprotected. That includes item edits by edit
  grantees on someone else's list, which constraint 6 names too. One app-wide
  control, decided once with tests and a route-coverage guard, is a better end
  state than a partial one inside a feature PR.
- **A — not taken.** It's cheap, but it protects two of roughly thirty
  writes.
- **B and C now — not taken.** Each changes app-wide security posture and
  middleware wiring, which belongs in its own brief and RR rather than in
  MB-0008.

## Approvals and authority

Approved by security-owner @juuso.kalliomaki

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0008-share-csrf-origin-check.md`
- Mission Brief: `.agentic/missions/MB-0008-share-list.md`
- Follow-up brief: `.agentic/missions/MB-0018-cross-site-request-protection.md`
- Evidence links:
  - CRP-0008 → Current state (no CSRF or origin handling in kauppa-web on
    ad22bd9);
  - `.agentic/resolutions/RR-0002-list-sharing-authz.md` → constraint 6.

## Constraints and follow-on rules (Hardened)

1. **The gap is tracked, not forgotten.** MB-0018 carries the deferred P8
   as its own P5 (`routes::lists::share_rejects_cross_origin`). RR-0002
   constraint 6 counts as met only when MB-0018 merges.
2. **No partial controls in the meantime.** Feature PRs don't add one-off
   origin checks; protection arrives app-wide through MB-0018.
3. **Deployment caution.** Until MB-0018 merges, `security.md`'s rule
   applies as written: CSRF protection comes before any deployment reachable
   outside localhost.

## Roll-back into the system (Hardened)

To be applied in MB-0008's PR after approval:

- `MB-0008`:
  - remove acceptance P8 and its escalation entry ("the app has no CSRF
    protection to ride…");
  - add `RR-0008` to `resolutions`;
  - add a Context note that P8 moved to MB-0018 (RR-0008).
- `MB-0018`: already lists RR-0008 and carries P8 as P5. No change.
- `CRP-0008`: `state: open` → `answered`.

No Mentorship Pack or Runbook change; that happens in MB-0018's RR.

## Supersession and history

Status: active. Supersedes nothing. Narrows RR-0002 constraint 6's timing
(MB-0018 rather than MB-0008); it doesn't change the constraint.
