# Consultation Request Pack — Shared-list access model & authorization

```yaml
---
id: CRP-0002
mission: MB-0008
owner: "@juuso-kalliomaki"
target_role: security
risk_tier: high
urgency: low
state:  answered
runbook_trigger: ".agentic/mentorship/security.md Authorization rule (`owner_id = $current_user`) and threat #3 (horizontal privilege escalation); feature.md escalation trigger `migrations/**` → data-owner for the new share table"
---
```

## Decision statement

Decide how a shopping list is shared with another user — the access-grant data
model and, critically, how the per-resource authorization predicate changes from
"owner only" to "owner or grantee" without opening a horizontal
privilege-escalation hole.

## Options and trade-offs

Today authorization is a single invariant (`security.md`): *every per-user
resource query filters on `owner_id = $current_user`*, and threat #3 is exactly
"reading another user's lists." Sharing deliberately relaxes that invariant, so
the question is security-tier, not a feature detail. Identity is an
`actix-identity` session carrying the user's UUID (`routes/lists.rs::current_user`).

### Option A — Share to an existing user by email, role-based (recommended)

A `list_shares(list_id, grantee_id, role, created_at)` table; the owner grants a
known account `view` or `edit`; the authz predicate becomes "owner OR a share
row for the current user with sufficient role," applied in the repo layer for
every list and list-item query.

- Pros: smallest new attack surface — access requires an authenticated session
  of a named account; natural revocation (delete the row); maps directly to
  "share with another user"; keeps authz in one checked predicate.
- Cons: requires an enumeration-safe user lookup by email; needs the predicate
  threaded through every list query, not just the entry handler.
- Risk: medium — the predicate must be centralized so no query forgets it; a
  missed query re-opens threat #3.
- Cost / timeline: one migration + a shared authz helper + per-route tests.

### Option B — Capability share link (unguessable token)

A tokenized URL grants access, optionally without login.

- Pros: frictionless; works for people without an account.
- Cons: the token *is* the credential — leakage (history, logs, referrer) =
  silent access; revocation and expiry must be built; "without login" breaks the
  session-based authz model and CSRF posture; contradicts "another user" (implies
  an account).
- Risk: high — larger surface, easy to get wrong, hard to audit who has access.
- Cost / timeline: token issuance + revocation + expiry + abuse handling.

### Option C — Household / group model

Users join a shared group; lists are shared to the group.

- Pros: scales to families; one grant covers many lists.
- Cons: much larger scope than the nice-to-have asks; new membership/invite
  lifecycle; premature for a single feature.
- Risk: medium — scope risk more than security risk.
- Cost / timeline: significantly more than A; a mini-subsystem.

## Recommendation and question

Recommended option: **A — share to an existing user by email, role-based.**
Because: it extends the existing session-based authz with the smallest new
surface, gives clean revocation, and matches the brief's wording ("another
user"). The real work is disciplined, not large: the owner-or-grantee check must
be a single shared predicate in the repo layer so no query can forget it.

Security constraints I propose to bind into the implementing brief (MB-0008):

1. **Centralized predicate.** One authz helper computes effective access
   (owner, or share row with role ≥ required); every list/list-item read and
   write goes through it. A query that filters on `owner_id` alone is a review
   failure.
2. **Role semantics.** `view` = read-only (no add/check/remove/share); `edit` =
   add/check/remove but **not** re-share and **not** delete the list. Only the
   owner shares, revokes, and deletes.
3. **Enumeration-safe invites.** Sharing to a non-existent / wrong email returns
   the same generic result as success — never reveal whether an account exists
   (consistent with the login error in `routes/auth.rs`).
4. **Revocation is immediate.** Deleting a share row removes access on the next
   request; no cached grants.
5. **Audit & logging.** Log share grant/revoke with grantor and list id, never
   the grantee's full email at `info`+ (per `security.md` logging rule).
6. **CSRF.** Grant/revoke/edit are state-changing POSTs and must ride whatever
   CSRF protection the app adopts (currently a `security.md` "should" — flag if
   this feature forces it to "must").

Crisp question to answer: **Do you approve Option A and constraints 1–6 as the
basis for MB-0008, with `view`/`edit` as the two roles and owner-only
share/revoke/delete?** If not, which option, and do any constraints change?
Secondary: does enabling cross-user data access make CSRF protection a
merge-blocking requirement for this feature (constraint 6)?

## Blast radius and rollback

What can break: the core authorization invariant — the highest-consequence
surface in the app. A missed query path would let a non-owner read or mutate a
list (threat #3). No production data today (pre-release), so the risk is
code-correctness, not live exposure.
Who is affected: every current and future list / list-item route.
Rollback plan: feature is additive and gated by the presence of share rows; a
compensating migration drops `list_shares` and the authz helper falls back to
owner-only. Because the predicate is centralized, removal is one helper + one
migration, not a scatter of reverts.

## Minimal context

- Brief: `.drafts/kauppalistasovellus.docx` — nice-to-have "the shopping list can be
  shared with another user."
- Mission: MB-0008 (sharing) — not yet scaffolded; depends on the list model
  from MB-0004 and will be scaffolded after this consult is resolved.
- Current authz: `crates/kauppa-web/src/routes/lists.rs` (`current_user`,
  `owner_id` filtering via `ShoppingListRepo`), `actix-identity` session set in
  `routes/auth.rs`.
- Governance: `.agentic/mentorship/security.md` (authorization rule, threat
  model #3, escalation requires CRP + approved RR); `feature.md` routes
  `migrations/**` to data-owner and auth/session changes to security — this
  consult is primarily **security**, with a **data-owner** co-sign on the
  `list_shares` migration.

## Evidence bundle (Hardened)

- The authz change is to a named invariant in `security.md`; the proof surface
  is per-route tests asserting a non-owner `view` grantee can read but not
  mutate, an `edit` grantee can mutate but not share, and a revoked grantee is
  denied on the next request.
- No benchmarks required; `list_shares(grantee_id, list_id)` is a small indexed
  lookup folded into existing queries, well within the AGENTS.md p95 budget.
- Repro / proof of the centralized predicate (and a test that fails if a query
  bypasses it) will be delivered under MB-0008 once this is approved.
```
