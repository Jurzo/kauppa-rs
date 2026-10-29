# Mission Brief — Share a shopping list with another user

```yaml
---
id: MB-0008
owner: "@juuso-kalliomaki"
goal: Let a list owner share a shopping list with another existing user as a viewer or an editor, enforced through one central authorization predicate, with immediate revocation.
non_goals:
  - Sharing recipes or the product catalog — lists and list items only, per RR-0002 scope.
  - Capability share-links or sharing to non-users (Option B, rejected in RR-0002).
  - A household / group model (Option C, rejected in RR-0002).
  - Real-time collaboration or edit-conflict resolution.
acceptance:
  - id: P1
    property: A single authorization helper decides access (owner, or a share row with role at least the required level); every list and list-item read and write routes through it, and a non-owner with no share is denied on reads and all mutations.
    check: cargo test -p kauppa-core repo::share::central_predicate_denies_non_grantee
  - id: P2
    property: A view-role grantee can read the list but every mutation (add, check, remove, share, delete) is rejected.
    check: cargo test -p kauppa-web routes::lists::view_role_is_read_only
  - id: P3
    property: An edit-role grantee can add, check, and remove items but cannot re-share, revoke, or delete the list.
    check: cargo test -p kauppa-web routes::lists::edit_role_cannot_administer
  - id: P4
    property: Only the owner can share, revoke, and delete the list.
    check: cargo test -p kauppa-web routes::lists::owner_only_administration
  - id: P5
    property: Sharing to an unknown or non-existent email returns the same generic response as a successful share, disclosing nothing about account existence.
    check: cargo test -p kauppa-web routes::lists::share_is_enumeration_safe
  - id: P6
    property: After the owner revokes a share, the former grantee is denied on the very next request with no cached grant.
    check: cargo test -p kauppa-core repo::share::revocation_is_immediate
  - id: P7
    property: Grant and revoke are logged with grantor and list id and never the grantee's full email at info level or higher.
    check: cargo test -p kauppa-web routes::lists::share_audit_logging
  - id: P8
    property: State-changing share and revoke endpoints reject cross-origin form posts (origin check), pending the app-wide CSRF decision.
    check: cargo test -p kauppa-web routes::lists::share_rejects_cross_origin
context:
  - crates/kauppa-web/src/routes/lists.rs
  - crates/kauppa-web/src/routes/auth.rs
  - crates/kauppa-core/src/repo/mod.rs
  - migrations/0001_init.sql
  - .agentic/missions/MB-0004-assemble-list.md
  - .agentic/mentorship/security.md
  - .agentic/consults/CRP-0002-list-sharing-authz.md
  - .agentic/resolutions/RR-0002-list-sharing-authz.md
autonomy:
  may_decide:
    - internal shape of the share repo methods and the authz helper, provided the predicate stays centralized
    - the share-management UI (invite form, grantee list, role selector) and its fragment markup
    - whether role is an enum column with a CHECK constraint or a lookup table
  escalate:
    - { when: schema change beyond the list_shares table approved in RR-0002, to: data-owner }
    - { when: the authorization predicate's scope is extended beyond lists and list-items, to: security }
    - { when: the app has no CSRF protection to ride and P8 forces a security posture change, to: security }
    - { when: public route added or removed, to: api-owner }
  forbidden:
    - bypass or duplicate the authorization predicate outside the central helper
    - edit a committed migration (migrations/0001_init.sql and earlier 000N files) — add a new numbered file
    - add dependencies
    - lower lint strictness
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/security.md@v1
  - .agentic/mentorship/persistence.md@v1
  - .agentic/mentorship/actix-web.md@v1
  - .agentic/mentorship/templating.md@v1
resolutions:
  - RR-0002
---
```

## Context

RR-0002 approved Option A from CRP-0002: share a list with an existing user by
email, with `view` / `edit` roles and owner-only share / revoke / delete. This
is the highest-consequence change in the app so far — it deliberately relaxes
the `security.md` authorization invariant (*"every per-user resource query
filters on `owner_id = $current_user`"*, threat #3, horizontal privilege
escalation) to "owner OR grantee with sufficient role" for lists and list items.

The brief's acceptance properties map one-to-one onto RR-0002's constraints 1–6
(P1 ↔ centralized predicate, P2/P3/P4 ↔ role semantics, P5 ↔ enumeration-safe
invites, P6 ↔ immediate revocation, P7 ↔ audit logging, P8 ↔ CSRF/origin). The
`list_shares` schema and the authz relaxation are both covered by RR-0002, whose
resolver (@juuso.kalliomaki) holds the data and security roles alike.

Depends on MB-0004 (the list / list-item model). Session identity comes from
`actix-identity` via `routes/lists.rs::current_user`.

## Conceptual plan

Add migration `0007_list_shares.sql`: a `STRICT` `list_shares(id, list_id
REFERENCES shopping_lists(id) ON DELETE CASCADE, grantee_id REFERENCES users(id)
ON DELETE CASCADE, role TEXT with a CHECK in ('view','edit'), created_at)` with
`UNIQUE(list_id, grantee_id)`. Put the access decision in the repo layer as one
helper the constraint calls the "central predicate": every list and list-item
method takes the acting user and resolves effective access before touching data,
so no query can filter on `owner_id` alone (forbidden). Add owner-only
share/revoke handlers and a share-management fragment in `templates/lists/`;
resolve the invite by email with a generic response regardless of match (P5).

Checkpoints: the predicate and its denial paths (P1, P6) are unit-tested at the
repo layer before any UI. Slow-mode triggers: if any list query cannot be routed
through the central helper, stop — that is the security crux; if the app has no
CSRF mechanism for P8 to ride, raise a consult (RR-0002 constraint 6 left the
"should → must" promotion to the resolver) rather than inventing one here.

## Validation plan

P1 and P6 are `kauppa-core` tests over a temp SQLite pool (non-grantee denial on
read + every mutation; denial on the request immediately after revoke). P2–P5,
P7, P8 are `kauppa-web` tests: role-scoped read/mutate matrices, owner-only
administration, enumeration-safe invite responses, audit-log field assertions,
and cross-origin rejection on the share/revoke POSTs. Evidence: named test
outputs in the scope-to-proof map, with an explicit test that fails if a query
bypasses the central predicate.
