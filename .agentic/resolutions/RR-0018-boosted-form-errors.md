# Resolution Record — Boosted form errors via the response-targets extension, and no history snapshots of secret pages

```yaml
---
id: RR-0018
closes: CRP-0018
mission: MB-0029
decided_by: "@juuso.kalliomaki"
role: platform
risk_tier: low
decided_at: 2026-11-10T08:05:00Z
status: approved
superseded_by: null
rolls_back_into:
  - AGENTS.md
  - .agentic/missions/MB-0029-form-errors-under-htmx.md
  - .agentic/consults/CRP-0018-boosted-form-errors.md
---
```

## Decision and outcome

Approve CRP-0018's **recommended set**: vendor the htmx
**response-targets extension 2.0.4** and put `hx-ext="response-targets"
hx-target-4xx="body"` on the login, register and create-invitation forms
only (Q1 B), and add `hx-history="false"` to the register and admin
invitations pages in this mission as brief delta P5 (Q5 (a)).

Sub-answers (numbered as CRP-0018's crisp questions):

1. **Mechanism (Q1, B; platform).** Vendor `htmx-ext-response-targets`
   **2.0.4** (0BSD) from
   `https://registry.npmjs.org/htmx-ext-response-targets/-/htmx-ext-response-targets-2.0.4.tgz`,
   file `package/dist/response-targets.min.js`. Before vendoring, all of
   these must hold:
   - tarball sha512 equals npm `dist.integrity`
     `sha512-Q/yfH0N2A40j903mr6ldGV3qLWMQeufROylYIbYQLBGrCpmydflxXmQwDOEhEWdPnBTgiHofkAo0gQ3S1BmyxQ==`;
   - `response-targets.min.js` is **1,692 B** with sha256
     `bde3061dd9b0c7a0b3c7bae63784312bbb7a3a798411b750fe1a5c07a286e63a`;
   - its sha384 equals the SRI htmx's docs publish for
     `htmx-ext-response-targets@2.0.4` at tag `v2.0.11`
     (`www/content/docs.md` l.1168):
     `sha384-T41oglUPvXLGBVyRdZsVRxNWnOOqCynaPubjUVjxhsjFTKrFJGEMm3/0KGmNQ+Pg`.

   `bigskysoftware/htmx-extensions` publishes no releases or tags, so there
   is no GitHub digest; two checks suffice for the extension (RR-0015
   constraint 3 requires the third value for htmx itself). Any mismatch
   stops the mission; the commit records the tarball URL,
   `dist.integrity`, the sha256 and the docs SRI.
   - Vendored at `assets/vendor/response-targets-2.0.4.min.js`,
     byte-for-byte, loaded from `templates/layout.hbs` **after** htmx with
     `integrity="sha384-T41oglUPvXLGBVyRdZsVRxNWnOOqCynaPubjUVjxhsjFTKrFJGEMm3/0KGmNQ+Pg"`
     and `defer`.
     <!-- check: CRP-0018 doesn't name the vendored file; this draft uses the upstream basename plus version (RR-0015's versioned-name rule) and avoids an `htmx-` prefix so MB-0030 P1's "exactly one htmx file" stays unambiguous. -->
   - Checksum constants (sha256, SRI, 1,692 B length) and a provenance doc
     comment beside the htmx constants in
     `crates/kauppa-web/src/templates/tests.rs`, checked by `cargo test`;
     a `SERVED_AS` row `("vendor/response-targets-2.0.4.min.js",
     "text/javascript")` in `crates/kauppa-web/src/routes/assets/tests.rs`;
     `no_external_scripts` allows the two same-origin vendored scripts and
     asserts each `integrity` against its constant. Constant and test names
     are the agent's (brief `may_decide`).
2. **Auth templates (Q1 security sub-question; security).** Template-only
   attribute changes to `templates/auth/login.hbs`,
   `templates/auth/register.hbs` and `templates/admin/invitations.hbs` are
   approved under this RR. No change to any handler under
   `crates/kauppa-web/src/routes/auth/` or `routes/admin/`: status codes,
   headers (including RR-0012's `no-store` / `no-referrer`) and bodies stay
   as they are.
3. **Scope (Q2).** Exactly three forms: sign in (`auth/login.hbs`),
   register (`auth/register.hbs`) and create invitation
   (`admin/invitations.hbs`). Not sign out, not revoke invitation, not any
   `hx-post`/`hx-get` element. Tests as CRP-0018 Q2 names them:
   `templates::tests::boosted_error_pages_swap` (P1: the three forms carry
   the attributes; the layout loads the extension after htmx with its
   `integrity`) and `templates::tests::fragment_errors_not_swapped` (P2: no
   `hx-post`/`hx-get` element or ancestor carries `hx-ext` or any
   `hx-target-<code>`; no `htmx-config` sets `responseHandling`).
4. **URL after an error (Q3).** Accepted: the boosted swap pushes the
   form's `action` URL. After a register error the address bar shows
   `/auth/register`, with the invitation token gone from it; the hidden
   `invite` field still carries it, so resubmitting works. P4's "the URL
   stays the form's" reads as the form's `action`. No `HX-Push-Url` header
   and no `hx-push-url` attribute.
5. **Order (Q4).** Implement after MB-0030 merges (RR-0017), so P4's
   browser check runs on htmx 2.0.11. This mission adds **no**
   `htmx-config` meta; MB-0031's `script-src 'self'` covers the extension.
6. **History cache (Q5, (a); security).** Add `hx-history="false"` to the
   register page (`auth/register.hbs`) and the admin invitations page
   (`admin/invitations.hbs`) in this mission, as brief delta **P5** with
   the test `templates::tests::secret_pages_skip_history`. Back on those
   pages refetches instead of restoring a snapshot.

Covers:

- MB-0029's implementation PR, with P1–P4 as written and P5 below;
- the vendored `response-targets-2.0.4.min.js`, its constants, `SERVED_AS`
  row, `layout.hbs` script tag and the `no_external_scripts` change;
- the attribute changes in `login.hbs`, `register.hbs` and
  `invitations.hbs`;
- the AGENTS.md and brief changes below.

Does not cover:

- options A (global `responseHandling` in `htmx-config` plus an
  `HX-Reswap: none` middleware) and C (`hx-boost="false"`);
- any change to an auth or admin handler, `main.rs`, `test_support`, or
  the session wiring;
- any other htmx extension, any other extension version, or modifying the
  vendored bytes;
- `historyCacheSize` or any other `htmx-config` key (MB-0031);
- error display for boosted links (GET) or plain-text 4xx answers, which
  stay dropped as today.

## Rationale and trade-offs

- **Q1 B — chosen.** The only option meeting P1–P4 as written without
  touching a handler, the session wiring or fragment behaviour. Scoped by
  construction: fragments and boosted links never carry the attribute, so
  P2 needs no server change. The extension (read in full) uses no eval,
  inline style or script creation, so it is CSP-compatible, and supports
  htmx `^2.0.2`. Cost: a second vendored file (+1.7 KB) and one attribute
  pair per future full-page form.
  - **A not chosen:** changes every htmx request; P2 depends on a complete
    middleware in a security path; boosted links answering plain-text 4xx
    would replace the page with a bare word.
  - **C not chosen:** a full reload on submit, so P4 fails and the brief
    needs a delta; kept as the fallback.
- **Auth templates — approved.** Only attributes change; RR-0012's and
  RR-0013's response rules are untouched.
- **Q3 — accepted.** Dropping the token from the address bar is what
  RR-0012 sub-answer 5 favours. Keeping the old URL would need a header
  change in `routes/auth/` or would break the success redirect's URL.
- **Q5 (a) — chosen.** Template-only, testable, and the error swap adds
  one more path that would snapshot the register page.
  - **(b) `historyCacheSize: 0` not chosen:** disables Back restore app-wide
    and touches MB-0031's meta.
  - **(c) rely on `sessionStorage` not chosen:** narrows the exposure to
    one tab's lifetime but doesn't remove it.

## Approvals and authority

Approved by platform-owner @juuso.kalliomaki (Q1 mechanism and vendored file,
Q2–Q4); security-owner @juuso.kalliomaki (Q1 security sub-question: auth
templates; Q5 history cache).

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0018-boosted-form-errors.md`
- Mission Brief: `.agentic/missions/MB-0029-form-errors-under-htmx.md`
- Related decisions: RR-0015 (vendoring rules); RR-0017 (MB-0030, htmx
  2.0.11, merges first); RR-0012 sub-answer 5 (register responses and the
  token's placement); RR-0013 (429 pages and the 4xx count, unchanged);
  CRP-0019 (MB-0031, CSP and the one `htmx-config` meta).
- Evidence links: CRP-0018 → Current state (inventory of 4xx answers with
  file:line) and Evidence bundle:
  - htmx 2.0.3 and 2.0.11 `dist/htmx.js`: `responseHandling` defaults,
    `handleAjaxResponse`, `determineHistoryUpdates`,
    `saveCurrentPageToHistory` honouring `hx-history="false"`, history
    storage;
  - npm `htmx-ext-response-targets` (`latest: 2.0.4`, 2025-10-18,
    publisher `carsongross`, peer `htmx.org ^2.0.2`, 0BSD); tarball sha512
    = `dist.integrity`; sha256/sha384 above;
  - htmx docs SRI at `v2.0.11` l.1167–1168;
  - `htmx-extensions` releases and tags: empty; advisories: none.

## Constraints and follow-on rules (Hardened)

1. **Two vendored files, no more.** `assets/vendor/` holds htmx
   (RR-0017) and `response-targets-2.0.4.min.js`. Another vendored file
   needs an RR (RR-0015 constraint 1).
2. **Byte-for-byte, checksummed, never edited.** The extension file is the
   tarball's `package/dist/response-targets.min.js` unchanged. Its sha256,
   SRI and length are checked by `cargo test`; file name, constants,
   `integrity` and `SERVED_AS` change together and only under an RR
   (RR-0015 constraint 2).
3. **Verified before vendoring.** Every extension version is checked
   against npm `dist.integrity` and the docs SRI at the htmx tag that
   publishes it, recorded in the commit (RR-0015 constraint 3).
4. **No external scripts.** The extension loads from `/assets/vendor/`
   only, never a CDN (RR-0015 constraint 4).
5. **Scoped attributes.** `hx-ext="response-targets"` and
   `hx-target-<code>` appear only on full-page boosted forms whose handler
   renders a 4xx page; never on `<body>`, a section, or an element with
   `hx-post`/`hx-get` (P2's test). A new form of that kind adds the pair
   and joins P1's list.
6. **Secret pages skip history.** A page that renders a secret (a token or
   one-time link) in its HTML carries `hx-history="false"`; P5's test
   pins register and invitations.
7. **htmx upgrades check the extension.** A later htmx upgrade confirms
   the extension's `htmx.org` peer range still covers it, and P1/P4 still
   pass.

## Roll-back into the system (Hardened)

To be applied in MB-0029's implementation PR:

- `AGENTS.md` → Engineering intent → Do-not-touch boundaries: after
  RR-0017's roll-back the bullet ends "…with its checksum checked in the
  test suite (htmx 2.0.11 from the npm registry, RR-0017). Adding,
  upgrading or modifying a vendored file is a platform decision." Replace
  "(htmx 2.0.11 from the npm registry, RR-0017)" with "(htmx 2.0.11,
  RR-0017, and its response-targets extension 2.0.4, RR-0018, both from
  the npm registry)". The bullet then reads:
  "`assets/` is served directly; nothing executable may be committed there
  except under `assets/vendor/`: pinned third-party JS from a reviewed
  source, byte-for-byte, with its checksum checked in the test suite (htmx
  2.0.11, RR-0017, and its response-targets extension 2.0.4, RR-0018, both
  from the npm registry). Adding, upgrading or modifying a vendored file is
  a platform decision."
  <!-- check: written against the bullet as RR-0017 leaves it; if RR-0017 is rejected or not yet applied, the replaced text is "(htmx 2.0.3 from the npm registry, RR-0015)" and the version reads 2.0.3, RR-0015. The Map "Client interactivity" row names no file and stays unchanged. -->
- `MB-0029`:
  - `resolutions: []` → `resolutions: [RR-0018]`;
  - brief delta, new acceptance property:
    ```yaml
      - id: P5
        property: "Pages that render a secret in their HTML (the register page's hidden `invite` token, the admin invitations page's one-time link) carry `hx-history=\"false\"`, so htmx never snapshots them into the history cache (RR-0018)."
        check: cargo test -p kauppa-web templates::tests::secret_pages_skip_history
    ```
  - `context` gains `templates/admin/invitations.hbs`;
  - P1's "the one the RR approves" points at RR-0018; Conceptual plan
    step 2 gains "Answered by RR-0018."
- `CRP-0018`: `state: open` → `answered`.

No Mentorship Pack or Runbook change.

## Supersession and history

Status: active once approved. Supersedes nothing. Adds a second vendored
file under RR-0015's rules; depends on RR-0017 for ordering (and for the
AGENTS.md wording it edits).

Amended by RR-0025 (2026-11-10): `hx-ext="response-targets"` is declared on
the layout's `<body>`, not on the forms (sub-answer 1). RR-0025 constraint
2 replaces constraint 5.
