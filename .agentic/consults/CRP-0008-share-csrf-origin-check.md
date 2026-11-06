# Consultation Request Pack — CSRF protection for list sharing (MB-0008 P8)

```yaml
---
id: CRP-0008
mission: MB-0008
owner: "@juuso-kalliomaki"
target_role: security
risk_tier: high
urgency: medium
state: open
runbook_trigger: "MB-0008 autonomy.escalate (\"the app has no CSRF protection to ride and P8 forces a security posture change\"); RR-0002 constraint 6 (resolver to confirm whether sharing promotes the security.md CSRF \"should\" to a merge-blocking \"must\"); feature.md escalation paths \"Session middleware in crates/kauppa-web/src/main.rs\" and \"Shared-list authorization in crates/kauppa-web/src/routes/lists.rs\""
---
```

## Decision statement

Decide what CSRF protection the share and revoke endpoints (and, if you
choose, every state-changing POST) ride, now that MB-0008 lets one user's
session change data another user can see, given the app has none today.

## Current state

- There is no CSRF or origin check anywhere in `kauppa-web` (checked on
  2026-11-06: no token, no `Origin` / `Referer` handling, no middleware beyond
  `TracingLogger`, `IdentityMiddleware` and `SessionMiddleware` in `main.rs`).
- The only mitigation is the session cookie's `SameSite=Lax` (actix-session's
  default, required by `security.md`). Modern browsers then leave the cookie off
  cross-site POSTs, so a classic cross-site form post arrives signed out. It
  does not cover *same-site* attackers (another app on a sibling subdomain of
  a self-hosted install), browsers without `SameSite` support, or a future
  move to `SameSite=None`.
- `security.md` → Rules (should): "Add CSRF protection (double-submit cookie
  or origin check) before any deployment that is reachable outside localhost."
- RR-0002 constraint 6: grant / revoke / edit must ride the app's CSRF
  protection; the resolver is to confirm whether this feature makes it a
  merge-blocking "must". MB-0008 P8 asks for an origin check on share and
  revoke, "pending the app-wide CSRF decision".
- MB-0008 P1–P7 are implemented on `agentic/mb-0008-share-list` without any
  CSRF mechanism; P8 is stopped here, as the brief's slow-mode trigger asks.

## Options and trade-offs

### Option A — Origin check on the share and revoke endpoints only (P8 as written)

A small helper in `routes/lists.rs` (no middleware, no dependency) rejects
`POST /lists/{id}/shares` and `POST /lists/{id}/shares/{grantee}/revoke` with
403 unless the request's `Origin` header (or, when absent, `Referer`) names the
same scheme + host + port the request was addressed to. A request with
neither header is rejected.

- Pros: smallest change; no template or session change; no dependency; P8
  closes inside the brief's scope; easy to test (`share_rejects_cross_origin`).
- Cons: protects two endpoints while every other POST (add / check / remove
  items, which edit grantees can now perform on someone else's list; delete
  list; recipes, favorites) stays unprotected, so the app's posture is uneven.
  "Same host as the request" trusts the `Host` header; behind a reverse
  proxy that rewrites `Host` it needs the proxy to forward it (or a configured
  public origin, a config change).
- Risk: low for the two endpoints, but leaves RR-0002's "grant / revoke /
  **edit**" partially uncovered.
- Cost / timeline: about an hour inside MB-0008.

### Option B — App-wide origin check middleware (promote the "should" to "must")

The same check as a `wrap_fn` middleware in `main.rs` on every non-safe
method (`POST`, and any future `PUT` / `PATCH` / `DELETE`), with the same
missing-header rule, and `security.md` amended to make CSRF protection a
"must".

- Pros: one control for the whole app, covering edit-grantee item writes as
  RR-0002 constraint 6 asks; no template changes; htmx requests carry `Origin`
  like form posts do; matches the "origin check" the `security.md` rule
  already names.
- Cons: touches session middleware wiring in `main.rs` (a security
  escalation path) and the security pack (an RR roll-back); scripted clients
  (curl smoke tests) must send an `Origin` header; same `Host`-header caveat
  as A.
- Risk: medium — a wrong comparison locks every user out of every write, but
  that fails loudly and is caught by the existing route tests once they send
  `Origin`.
- Cost / timeline: half a day, best as its own small brief (or folded into
  MB-0008 if you prefer one PR).

### Option C — Double-submit token (synchronizer token in the session)

A per-session random token in a hidden field on every form and in an
`hx-headers` attribute on `<body>`, checked on every POST.

- Pros: does not depend on `Origin` / `Referer` being present or on the
  `Host` header; the textbook defence.
- Cons: touches every form template and the layout, the session contents,
  and needs a CSPRNG source (`uuid` v4 is available, but a dedicated token
  type is cleaner); much larger diff; every route test must fetch a token.
- Risk: medium — easy to miss a form, which then fails closed.
- Cost / timeline: one to two days, its own brief.

### Option D — Keep `SameSite=Lax` as the only mitigation; P8 waived

- Pros: no work.
- Cons: leaves RR-0002 constraint 6 unmet; sharing raises the stakes of a
  same-site CSRF (it can now grant another account access to a list).
- Risk: medium for an internet-reachable install.
- Cost / timeline: none; drop P8 from MB-0008 by brief amendment.

## Recommendation and question

Recommended option: **A now, B as a follow-up brief.** Because: A closes P8
inside MB-0008's approved scope without touching `main.rs` or the security
pack, and grant / revoke are the endpoints whose forgery is worst (a forged
share hands a list to the attacker's account). B is the right end state, but
it changes app-wide security posture and the session middleware wiring, which
deserves its own brief, tests and RR roll-back rather than riding a feature PR.

Crisp question to answer: **Does MB-0008 merge with (A) an origin check on
share and revoke only, (B) an app-wide origin check, (C) a token, or (D) no
CSRF control — and does sharing promote `security.md`'s CSRF rule from
"should" to "must"?** Secondary: for the origin comparison, is the request's
`Host` header acceptable, or do you want a configured public origin
(`KAUPPA_PUBLIC_ORIGIN`, a config change)?

## Blast radius and rollback

What can break: with A, only share and revoke (a false rejection blocks
sharing, nothing else). With B, every write in the app. With C, every form.
Who is affected: list owners sharing lists (A); every signed-in user (B, C).
Rollback plan: A and B are a helper / middleware removal in one commit; C
also reverts template changes. No schema change in any option.

## Minimal context

- `.agentic/missions/MB-0008-share-list.md` → P8, `autonomy.escalate`.
- `.agentic/resolutions/RR-0002-list-sharing-authz.md` → constraint 6.
- `.agentic/mentorship/security.md` → threat #4 (CSRF), Rules (should).
- `crates/kauppa-web/src/main.rs` (middleware stack),
  `crates/kauppa-web/src/routes/lists.rs` (share / revoke handlers).

## Evidence bundle (Hardened)

- `grep -rni "csrf\|origin" crates/` on `origin/main` (ad22bd9) finds no CSRF
  mechanism (only unrelated doc-comment matches).
- MB-0008 P1–P7 tests pass without any CSRF control; P8's check
  (`cargo test -p kauppa-web routes::lists::share_rejects_cross_origin`) is not
  written yet, pending this answer.
