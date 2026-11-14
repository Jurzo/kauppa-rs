# Resolution Record — Amend RR-0018: declare the response-targets extension on `<body>`

```yaml
---
id: RR-0025
closes: CRP-0025
mission: MB-0029
decided_by: "@juuso.kalliomaki"
role: platform
risk_tier: low
decided_at: 2026-11-10T09:20:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/resolutions/RR-0018-boosted-form-errors.md
  - .agentic/missions/MB-0029-form-errors-under-htmx.md
  - .agentic/consults/CRP-0025-response-targets-on-body.md
---
```

## Decision and outcome

Approve CRP-0025 **Option A**: the layout's `<body>` declares
`hx-ext="response-targets"`; the login, register and create-invitation
forms keep `hx-target-4xx="body"` and drop their own `hx-ext`.

Sub-answers:

1. **Placement.**
   - `templates/layout.hbs`: `<body hx-boost="true" hx-ext="response-targets">`.
     It is the only element that declares `hx-ext`.
   - `templates/auth/login.hbs`, `templates/auth/register.hbs` and
     `templates/admin/invitations.hbs`: the three forms carry
     `hx-target-4xx="body"` and no `hx-ext`.
   - The template-only changes to `templates/auth/` and `templates/admin/`
     are approved under this RR, as RR-0018 sub-answer 2 approved them. No
     handler changes.
2. **Tests.**
   - `templates::tests::boosted_error_pages_swap` (P1) asserts that the
     layout's `<body>` declares the extension and that the three forms carry
     `hx-target-4xx="body"`.
   - `templates::tests::fragment_errors_not_swapped` (P2) asserts that
     `hx-ext` appears only on the layout's `<body>`, and that
     `hx-target-<code>` appears only on the three forms, none of which has
     or contains `hx-post`/`hx-get`. It also asserts that no template sets
     `responseHandling`.
   - The two route-test assertions that match the exact form tag
     (`routes/auth/tests/register_requires_invite.rs`,
     `routes/admin/tests/invites.rs`) follow the new tag, as in #58.
3. **No `HX-Retarget`.** The app sends no `HX-Retarget` header. A handler
   that adds one on a non-200 response needs an RR, because with the
   extension active page-wide that response would be swapped.
4. **P4.** The owner reruns MB-0029's P4 error-message lines on the release
   binary. The lines that already passed (history, success redirects,
   fragments, console) need no rerun beyond a quick look.

Covers:

- MB-0029's follow-up PR on `agentic/mb-0029-boosted-errors-fix`: the
  template, test and roll-back changes above.

Does not cover:

- options B (global `responseHandling`) and C (revert);
- any other `hx-ext` value, any `hx-target-<code>` on another element, or
  any handler change;
- a browser-level test in CI (a possible follow-up; it would need a JS
  runtime in CI, a platform decision).

## Rationale and trade-offs

- **A, chosen.**
  - htmx 2.0.11 dispatches `htmx:beforeSwap` on the swap target and runs
    the extensions declared on it or its ancestors. A boosted form's
    target is `<body>`, so `hx-ext` must be on `<body>` or above it.
  - The extension acts only where `hx-target-<code>` is found from the
    requesting element upward, so the scope rule moves to that attribute.
  - Verified in jsdom against the release binary (CRP-0025 Evidence).
- **B, not chosen:** it changes every request and was already rejected in
  RR-0018.
- **C, not chosen:** it leaves MB-0029's goal unmet.

## Approvals and authority

Approved by platform-owner @juuso.kalliomaki (placement, constraint 5 wording) and
security-owner @juuso.kalliomaki (auth and admin templates and their route tests).

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0025-response-targets-on-body.md`
- Mission Brief: `.agentic/missions/MB-0029-form-errors-under-htmx.md`
- Related decisions:
  - RR-0018, which this amends: sub-answer 1's placement and constraint 5;
  - RR-0019 (CSP; `script-src 'self'` covers the extension).
- Evidence links:
  - CRP-0025 Evidence bundle: the htmx source excerpts, the jsdom repro
    before and after, and the `HX-Retarget` grep;
  - the owner's P4 run on 2026-11-10.

## Constraints and follow-on rules (Hardened)

1. **One declaration.** `hx-ext="response-targets"` appears only on the
   layout's `<body>`. P2's test enforces this.
2. **Replaces RR-0018 constraint 5.** `hx-target-<code>` appears only on
   full-page boosted forms whose handler renders a 4xx page, never on
   `<body>`, a section, or an element that has or contains
   `hx-post`/`hx-get`. A new form of that kind adds the attribute and joins
   P1's list.
3. **No `HX-Retarget`** on non-200 responses without an RR (sub-answer 3).
4. **Behaviour, not just markup.** A change to htmx, the extension or this
   wiring is not done until MB-0029's P4 error lines pass in a browser.

## Roll-back into the system (Hardened)

To be applied in the follow-up PR:

- `RR-0018`: under "Supersession and history", add "Amended by RR-0025
  (2026-11-10): `hx-ext=\"response-targets\"` is declared on the layout's
  `<body>`, not on the forms (sub-answer 1). RR-0025 constraint 2 replaces
  constraint 5." Nothing else in RR-0018 changes.
- `MB-0029`:
  - `resolutions: [RR-0018]` → `[RR-0018, RR-0025]`;
  - P1's "(RR-0018)" → "(RR-0018, RR-0025)".
- `CRP-0025`: `state: open` → `answered`.

No Mentorship Pack or Runbook change.

## Supersession and history

Status: active once approved. Amends RR-0018 (placement and constraint 5);
supersedes nothing.

Amended by RR-0038 (2026-11-14): the error-page middleware sends
`HX-Retarget: body` on its pages for htmx requests (constraint 3 and
sub-answer 3 allow it for that case only).
