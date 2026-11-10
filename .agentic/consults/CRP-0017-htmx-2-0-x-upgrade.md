# Consultation Request Pack — Upgrade the vendored htmx to 2.0.11 (MB-0030)

```yaml
---
id: CRP-0017
mission: MB-0030
owner: "@juuso-kalliomaki"
target_role: platform
risk_tier: low
urgency: medium
state: answered
runbook_trigger: "AGENTS.md Do-not-touch (upgrading a vendored file is a platform decision); RR-0015 Does not cover (any htmx version other than 2.0.3) and constraints 2–3 (new file, constants and integrity together under an RR, verified against registry integrity, docs SRI and, from 2.0.11, the GitHub release digest); MB-0030 autonomy.escalate (version and source → platform-owner; release notes that change boost, history, swap defaults or responseHandling → owner). The AGENTS.md wording change is an RR roll-back (MB-0030 forbidden: edit AGENTS.md except as the RR's roll-back)."
---
```

## Decision statement

Approve replacing `assets/vendor/htmx-2.0.3.min.js` with **htmx 2.0.11**
(the latest 2.0.x), byte-for-byte from the npm registry tarball, and accept
the behaviour changes between 2.0.3 and 2.0.11 listed in Q3.

Routing: platform-owner for Q1, Q2, Q4, Q5; owner for Q3 (behaviour). Both
are @juuso-kalliomaki, so one RR can answer everything.

## Current state

Verified on `50322c0` (2026-11-09).

- `assets/vendor/htmx-2.0.3.min.js`, 50,387 B, sha256 `491955cd…b2784e`
  (re-checked against a fresh npm download today: identical).
- Pinned in four places, all of which change together (RR-0015
  constraint 2):
  - `templates/layout.hbs:10`: `src` and `integrity`;
  - `crates/kauppa-web/src/templates/tests.rs:13-26`: `HTMX_PATH`,
    `HTMX_SHA256`, `HTMX_SRI_SHA384` and the provenance doc comment
    (tarball URL, npm `dist.integrity`, 0BSD);
  - `tests.rs:185`: `assert_eq!(bytes.len(), 50_387)`;
  - `crates/kauppa-web/src/routes/assets/tests.rs:20`: the `SERVED_AS` row
    `("vendor/htmx-2.0.3.min.js", "text/javascript")`.
- `.gitattributes`: `assets/vendor/** -text` (unchanged by this mission).
- `AGENTS.md` Do-not-touch names "htmx 2.0.3 from the npm registry,
  RR-0015" (roll-back below).
- htmx features the app uses (grep over `templates/`): `hx-boost` ×1 (on
  `<body>`), `hx-post` ×32, `hx-target` ×33, `hx-swap` ×33 (all
  `outerHTML` except one `none`), `hx-confirm` ×6, one `hx-get` with
  `hx-trigger="input changed delay:300ms"` (ingredient autocomplete). No
  `hx-on*`, `hx-push-url`, `hx-history`, `hx-indicator`, `hx-select`,
  `hx-vals`, extensions, `htmx-config` meta, or `htmx.ajax` calls.
- Server-side htmx headers used: `HX-Redirect` (recipe create/delete). No
  `HX-Location`, `HX-Push-Url`, `HX-Retarget`, `HX-Reswap`.

## Questions, options and recommendations

### Q1 — Target version (platform-owner)

npm `dist-tags` (2026-11-09): `latest: 2.0.11`, `next: 4.0.1`. 2.0.11 is
the newest 2.0.x (published to npm 2026-09-22). No 2.1 exists; 4.x is out
of scope (brief non-goal).

**Option A — 2.0.11 (recommended).**

- Pros: the brief's goal; all 2.0 fixes; three-way checksum agreement
  (Q2); same config defaults as 2.0.3 for everything MB-0029 and MB-0031
  will configure (Evidence).
- Cons: changes client behaviour on every page (Q3) with no browser tests;
  the only check is P4's manual smoke.
- Risk: low.

**Option B — 2.0.6 (smallest step that carries the history/storage fix).**

- Pros: fewer changes (2.0.4–2.0.6).
- Cons: misses the history relative-path fixes (2.0.8, 2.0.9), the
  indicator-style change (2.0.7) MB-0031 should copy, and the boosted
  `hx-push-url` fix (2.0.11). No GitHub release digest before 2.0.11, so
  only two checks. A later bump repeats this whole procedure.
- Risk: low.

**Option C — stay on 2.0.3; close MB-0030.**

- Pros: nothing changes.
- Cons: keeps the `localStorage` history cache (Q3, finding), and MB-0029 /
  MB-0031 get configured against a version that is then upgraded later.

**Recommendation: A.**

### Q2 — Source and provenance (platform-owner)

Same procedure as RR-0015 sub-answer 1, plus the GitHub digest that
constraint 3 asks for from 2.0.11 on. All fetched 2026-11-09 into an empty
scratch directory; nothing executed, nothing committed.

| Check | Published value | Computed from the npm tarball | Match |
| --- | --- | --- | --- |
| npm `dist.integrity` of `htmx.org-2.0.11.tgz` | `sha512-Thx/WtpeOQqSrqBCw/A1cwGJGg4UrVa3+sW0GmrM3p4gJgO89ecH4qtbnyzDDWFvBTqjnIMCgELTNt636dtamA==` | same (`openssl dgst -sha512 -binary \| base64`) | yes |
| `package/dist/htmx.min.js` | — | 52,182 B, sha256 `d6fdc75f204e6bdefa99b69bf1e6d4ac69b8a364f77929f45c13476b4000f717` | — |
| htmx docs SRI at tag `v2.0.11` (`www/content/docs.md` l.127, l.1167) | `sha384-2OatzQy1H+Zd/IIrjr1TcuDGqLXeHhbooAyJY1KdQMKnr4LZ22k31GBLdYKHmVjg` | same | yes |
| GitHub release `v2.0.11` asset `htmx.min.js` `digest` | `sha256:d6fdc75f…4000f717`, 52,182 B | same; the asset itself was also downloaded and hashes the same | yes |
| `package.json` license | `0BSD` | — | unchanged |

Notes for the RR:

- The npm publisher is still `carsongross` for every 2.0.x; no npm
  provenance attestation (as for 2.0.3).
- The GitHub release `v2.0.11` was *created* 2026-09-22 (with the npm
  publish) but *published* 2026-11-01 by `1cg`. The asset digest is
  GitHub-computed at upload, so the later publish date doesn't weaken it.
- No advisories: `gh api '/advisories?ecosystem=npm&affects=htmx.org'` → 0;
  the repository's security advisories → 0. No CHANGELOG entry from 2.0.4
  to 2.0.11 is labelled a security fix.
- The same values were in CRP-0015's evidence on 2026-11-08; they are
  unchanged.

**Recommendation:** source `https://registry.npmjs.org/htmx.org/-/htmx.org-2.0.11.tgz`,
file `package/dist/htmx.min.js`, verified by all three values above; the
commit records the tarball URL, `dist.integrity`, the sha256, the docs SRI
and the GitHub digest.

### Q3 — Behaviour changes 2.0.3 → 2.0.11 that touch the app (owner)

From the CHANGELOG at `v2.0.11` and a diff of `dist/htmx.js` 2.0.3 vs
2.0.11. Only entries touching features the app uses (boost, history, swap,
`responseHandling`, indicators, eval) are listed.

| Version | Change | Effect on kauppa-rs |
| --- | --- | --- |
| 2.0.4 | Boosted `GET` forms with no/empty `action` replace the query | None: every boosted form is `POST` with an `action`. |
| 2.0.5 | **History cache moves from `localStorage` to `sessionStorage`**; history restoration "follows the standard swapping code paths"; `hx-on` listeners added earlier | Back/forward restores from a per-tab cache that is cleared when the tab closes. **Security plus** (finding below). Restoration path change is the main thing P4's back/forward smoke checks. |
| 2.0.6 | Fix for 2.0.5's regression where boosted links with child elements did full reloads | Covered by P4 (the nav links are plain text, but the brand link and buttons are boosted). |
| 2.0.7 | Indicator styles add `visibility:hidden`; `reportValidity()` behind `reportValidityOfForms` (default `false`) | No indicator is used today. MB-0031 copies the 2.0.11 rules into `app.css`, which is why it should follow this mission. `reportValidityOfForms` stays off. |
| 2.0.8 | `parseHTML` uses `Document.parseHTMLUnsafe()` where available; history relative-path fix | Swapped fragments are parsed by a different API in current browsers; same result for the app's plain HTML. CSP-neutral (no Trusted Types policy is planned). |
| 2.0.9 | History `normalizePath` resolves relative paths against the current page; `hx-disabled-elt` preserves pre-disabled elements; `HX-Location` `replace` fix | History entries for boosted pages keep their correct paths. App uses neither `hx-disabled-elt` nor `HX-Location`. |
| 2.0.10 | `CSS.escape()` for tag/id in settle lookup | Robustness; ids here are UUIDs and fixed words. |
| 2.0.11 | `<hx-partial>` backport (responses may carry `<hx-partial hx-target>` elements that swap into their own targets); `hx-push-url="false"` on an ancestor of a boosted element now honoured; `hasHeader()` exact match | `<hx-partial>`: the app never emits it, and user text is HTML-escaped by Handlebars, so injected text can't become one. `hx-push-url` fix matters only if MB-0029 uses it. `hasHeader` fix: `HX-Redirect` detection unchanged. |

Unchanged between 2.0.3 and 2.0.11 (read from `dist/htmx.js`):

- `responseHandling` default `[{204, swap:false}, {[23].., swap:true},
  {[45].., swap:false, error:true}]` and `resolveResponseHandling` /
  `codeMatches` logic: MB-0029's options behave the same on either version;
- `allowEval: true`, `includeIndicatorStyles: true`, `allowScriptTags:
  true`, `inlineStyleNonce: ''`, `selfRequestsOnly: true`,
  `historyCacheSize: 10`, `defaultSwapStyle: 'innerHTML'`;
- `htmx-config` meta parsing (`querySelector('meta[name="htmx-config"]')`,
  `JSON.parse`, shallow merge) and `maybeEval` gating.

**Finding (security, for MB-0029/MB-0031).** On 2.0.3, every boosted
navigation snapshots the current `<body>` into `localStorage`
(`htmx-history-cache`), which persists after the browser closes. Two pages
put secrets in attribute values that end up in that snapshot: the register
form's hidden `invite` token (`templates/auth/register.hbs:9`) and the
admin page's one-time invitation link
(`templates/admin/invitations.hbs:12`). 2.0.11 moves the cache to
`sessionStorage` (per tab, cleared on close), which narrows but doesn't
remove this. The fix (`hx-history="false"` on those pages, or
`historyCacheSize: 0`) is a separate decision raised in CRP-0018 Q5; it is
not part of MB-0030 (behaviour non-goal).

**Recommendation:** accept all of the above; P4's smoke covers boosted
navigation, back/forward (now `sessionStorage`), a list item fragment, the
`hx-confirm` dialogs and the ingredient autocomplete.

### Q4 — Files and tests (platform-owner; brief: commit order is the agent's)

One PR:

- delete `assets/vendor/htmx-2.0.3.min.js`, add
  `assets/vendor/htmx-2.0.11.min.js` (bytes from Q2);
- `layout.hbs`: `src="/assets/vendor/htmx-2.0.11.min.js"
  integrity="sha384-2OatzQy1H+Zd/IIrjr1TcuDGqLXeHhbooAyJY1KdQMKnr4LZ22k31GBLdYKHmVjg"`;
- `templates/tests.rs`: constants, provenance comment (adds the GitHub
  digest), and the length assertion → `52_182`;
- `routes/assets/tests.rs`: `SERVED_AS` row renamed; P3's "old URL is 404"
  is one added assertion in `serves_every_asset` (the service already 404s
  any unlisted name).

No manifest, `Cargo.lock` or `.gitattributes` change. `cargo deny`
unaffected.

### Q5 — AGENTS.md wording (RR roll-back, platform-owner)

Replace "(htmx 2.0.3 from the npm registry, RR-0015)" with "(htmx 2.0.11
from the npm registry, RR-<this RR>)" in the Do-not-touch bullet. Nothing
else in AGENTS.md changes.

## Recommendation and question

| Q | Recommendation |
| --- | --- |
| Q1 Version | 2.0.11 |
| Q2 Source | npm tarball, verified by `dist.integrity`, docs SRI at `v2.0.11` and GitHub release digest (all agree) |
| Q3 Behaviour | Accept 2.0.4–2.0.11 changes; P4 smoke as listed |
| Q4 Files | One-PR swap as listed; `bytes.len()` → 52,182; old URL 404 asserted |
| Q5 AGENTS.md | Version and RR id in the Do-not-touch bullet |

Crisp questions to answer:

1. **Q1:** Upgrade to **2.0.11** (A), or 2.0.6 (B), or stay (C)?
2. **Q2:** Is the npm tarball, checked against the three values in Q2's
   table, the approved source for 2.0.11?
3. **Q3:** Do you accept the listed behaviour changes, in particular the
   history cache moving to `sessionStorage` and restoration via the normal
   swap path, with the P4 smoke as the only check?
4. **Q5:** May the RR roll the version change into AGENTS.md's
   Do-not-touch bullet as worded?
5. **Order:** confirm MB-0030 merges before MB-0029 and MB-0031 implement
   (their CRPs can be answered in parallel).

## Blast radius and rollback

- **What can break:** client-side behaviour on every page (boosted
  navigation, back/forward, fragment swaps). Worst case htmx fails to load
  (wrong `integrity`) and the app degrades to plain forms and full page
  loads, which every route already supports (P3 of MB-0027); nothing is
  lost.
- **Who is affected:** every user's interactivity; no data, schema,
  session or auth path.
- **Rollback plan:** revert the PR; the 2.0.3 file, constants and tag come
  back together. Users' `sessionStorage` cache is per tab and harmless; the
  old `localStorage` cache key stays until cleared (htmx 2.0.11 no longer
  reads it).

## Minimal context

- MB-0030 P1–P4; forbidden: modifying the vendored file, CDN loads, editing
  AGENTS.md outside the roll-back.
- RR-0015 constraints 1–4; CRP-0015 Q2 (why 2.0.3 first).
- Siblings: MB-0029 (CRP-0018) and MB-0031 (CRP-0019) both add an
  `htmx-config` meta and depend on 2.0.11's defaults and indicator CSS.

## Evidence bundle (Hardened)

All fetched 2026-11-09 into the agent's scratchpad, one empty directory per
download, tarballs extracted with `--no-same-owner` and only hashed and
read.

- `curl https://registry.npmjs.org/htmx.org`: `dist-tags` `{latest:
  2.0.11, next: 4.0.1}`; 2.0.x publish dates and integrities (2.0.4
  2024-12-13 … 2.0.11 2026-09-22), all `0BSD`, all `_npmUser carsongross`.
- 2.0.11 tarball: sha512 equals `dist.integrity`; `dist/htmx.min.js` 52,182
  B, sha256 `d6fdc75f…4000f717`, sha384 `2OatzQy1…YKHmVjg`.
- 2.0.3 tarball re-downloaded: `dist/htmx.min.js` sha256 equals the
  committed file (`491955cd…b2784e`).
- `gh api repos/bigskysoftware/htmx/releases/tags/v2.0.11`: `htmx.min.js`
  52,182 B, `digest: sha256:d6fdc75f…4000f717`; downloaded asset hashes the
  same.
- `gh api 'repos/bigskysoftware/htmx/contents/www/content/docs.md?ref=v2.0.11'`
  (blob `bdff5c7a`): `integrity="sha384-2OatzQy1H+Zd/IIrjr1TcuDGqLXeHhbooAyJY1KdQMKnr4LZ22k31GBLdYKHmVjg"`.
- Advisories: GitHub Advisory Database and repo advisories → none.
- CHANGELOG: `CHANGELOG.md` at `v2.0.11`, sections 2.0.4–2.0.11 (Q3).
- Source diff (`dist/htmx.js`): config defaults l.83–288 (2.0.11 adds
  `reportValidityOfForms: false`, l.288); `responseHandling` l.264–267 in
  both; `insertIndicatorStyles` 2.0.3 l.4958 vs 2.0.11 l.5135 (adds
  `visibility`); history storage `localStorage` (2.0.3 l.3034–3089) vs
  `sessionStorage` (2.0.11 l.3193–3308); `saveCurrentPageToHistory` honours
  `hx-history="false"` in both; `parseHTMLUnsafe` 2.0.11 l.528.
