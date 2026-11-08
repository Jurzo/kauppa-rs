# Consultation Request Pack — Vendor a pinned htmx into `assets/` (MB-0027)

```yaml
---
id: CRP-0015
mission: MB-0027
owner: "@juuso-kalliomaki"
target_role: platform
risk_tier: low
urgency: low
state: answered
runbook_trigger: "AGENTS.md Do-not-touch (`assets/` holds only vendored, pinned JS from a reviewed source; vendoring htmx is a platform decision). MB-0027 autonomy.escalate: \"this brief's whole change\" and \"which source is reviewed ... and whether to upgrade the version\" (platform-owner); \"adding a `Content-Security-Policy` header in the same PR\" (security). .agentic/runbooks/dependency-bump.md (Q3 option A adds `sha2` as a dev-dependency in `crates/kauppa-web/Cargo.toml`). The AGENTS.md wording change is an RR roll-back (MB-0027 forbidden: edit AGENTS.md)."
---
```

## Decision statement

Approve vendoring htmx into `assets/vendor/`, and choose:

- the reviewed source;
- the version;
- where the checksum is recorded and how the test computes it;
- whether a Content-Security-Policy ships now;
- the AGENTS.md wording that the RR rolls back.

Routing: platform-owner for Q1–Q4 and Q6–Q7; security for Q5 (CSP). Both
roles are @juuso-kalliomaki, so one RR can answer everything.

## Current state

Verified on `63da727` (2026-11-08).

- **`templates/layout.hbs`:** `<script src="https://unpkg.com/htmx.org@2.0.3"
  defer></script>`, with no `integrity` and no `crossorigin`.
  - unpkg answers that URL with a redirect to
    `/htmx.org@2.0.3/dist/htmx.min.js` (`curl -I`: `location:
    /htmx.org@2.0.3/dist/htmx.min.js`).
  - No other `<script>` in `templates/`.
- **htmx features used** (grep over `templates/`):
  - `hx-post` ×32, `hx-target` ×33, `hx-swap` ×33, `hx-confirm` ×5;
  - one `hx-get` with `hx-trigger="input changed delay:300ms"`;
  - `hx-boost="true"` on `<body>`.
  - **None of** `hx-on*`, trigger filters (`[...]`), `hx-vals`/`hx-vars` with
    `js:`, inline `<script>`, `<style>` or `style="..."`.
- **`assets/`:** `app.css` and `manifest.webmanifest`; no JS.
- **Serving.** `Files::new("/assets", "./assets")` in `main.rs`, ahead of
  `routes::configure` (RR-0010). actix-files 0.7.4 uses mime_guess 2.0.5,
  which maps `.js` → `text/javascript` (`mime_types.rs:579`). Subdirectories
  are served as they are.
- **Hashing in Rust.**
  - Not available without a crate: `kauppa-web` has no direct hashing
    dependency.
  - `sha2 0.10.9` is in `Cargo.lock` already, through `cookie` (actix-web's
    `cookies` feature) and sqlx. Using it in a test still means a `Cargo.toml`
    line (dependency-bump runbook).
- **No `.gitattributes`.** A checkout with `core.autocrlf=true` could rewrite
  line endings in a text file and change its hash. The minified file has
  none to rewrite, but nothing guarantees that.
- **Tests.** `templates.rs` has one top-level `#[cfg(test)]` test
  (`layout_params_do_not_shadow_body`) and no `tests` module yet. The brief's
  P1/P2 names (`templates::tests::…`) imply adding one.

## Questions, options and recommendations

### Q1 — Which source counts as reviewed (platform-owner)

Each source was fetched on 2026-11-08 and the bytes compared:

| Source | 2.0.3 `dist/htmx.min.js` | Integrity it publishes |
| --- | --- | --- |
| npm registry tarball `htmx.org-2.0.3.tgz` | sha256 `491955cd…b2784e`, 50,387 B | `dist.integrity` sha512 (matches the tarball), npm registry ECDSA signature; publisher `carsongross` |
| htmx docs at tag `v2.0.3` (`www/content/docs.md`) | — | SRI `sha384-0895/pl2MU10Hqc6jd4RvrthNlDiE9U1tWmX7WRESftEDRosgxNsQG/Ze9YMRzHq`, which **matches** the npm file |
| GitHub release `v2.0.3` asset `htmx.min.js` | 50,387 B (API) | no digest: the asset predates GitHub recording asset digests. The download returned 503 from the agent's network, so its bytes weren't compared |
| unpkg `htmx.org@2.0.3` | same sha256 as npm | none; unpkg mirrors the npm tarball |

**Option A — npm registry tarball, verified by `dist.integrity`, cross-checked
against the SRI in htmx's own docs (recommended)**

- Pros:
  - the canonical publication: what every CDN, including today's unpkg URL,
    serves;
  - two independent published values, checked against each other: the npm
    registry hash, and the maintainers' SRI in the git-tagged docs;
  - repeatable by anyone with `curl` and `openssl`.
- Cons:
  - npm publish is a single maintainer account; there's no npm provenance
    attestation for htmx.
- Risk: low.

**Option B — GitHub release asset**

- Pros: built from the tagged repo and uploaded by the maintainers.
- Cons:
  - for 2.0.3 there's no published digest to verify against;
  - it isn't the bytes users have been running.
  - (From 2.0.11 on, release assets do carry a sha256 digest. 2.0.11's
    `htmx.min.js` digest `d6fdc75f…00f717` matches its npm tarball, a useful
    third check on upgrade.)
- Risk: low.

**Option C — unpkg pinned file (`/htmx.org@2.0.3/dist/htmx.min.js`)**

- Pros: what's in production today.
- Cons: a mirror of A, not a source. It's the CDN this brief stops trusting.
- Risk: low (same bytes today), but it isn't "reviewed".

**Recommendation: A.**

The commit records:

- the tarball URL;
- the npm `dist.integrity` (sha512);
- the file's sha256;
- the docs SRI (sha384).

The vendored file is `package/dist/htmx.min.js` from the tarball,
byte-for-byte.

### Q2 — Version: stay on 2.0.3 or move to the latest 2.0.x (platform-owner)

Facts:

- **Latest 2.x:** 2.0.11 (npm `latest`, 2026-09-22).
- **4.0.x** is the `next` tag (4.0.0 on 2026-08-28, 4.0.1 on 2026-11-01). A
  new major is out of scope.
- **No security advisories** for `htmx.org`:
  - the GitHub Advisory Database (`gh api /advisories?ecosystem=npm&affects=htmx.org`)
    and the repository's security advisories both return none;
  - CHANGELOG 2.0.4–2.0.11 has no entry labelled as a security fix. The
    nearest is 2.0.10's "improved escaping of tag and id in settle lookup
    using `CSS.escape()`", a robustness fix.
- **Behaviour changes since 2.0.3** that touch features this app uses
  (`hx-boost`, history, swaps):
  - 2.0.5: the history cache moves from `localStorage` to `sessionStorage`,
    and history restoration "now follows the standard swapping code paths";
  - 2.0.6: fixes a 2.0.5 regression where boosted links with child elements
    did full reloads;
  - 2.0.8: `parseHTML` switches to `Document.parseHTMLUnsafe()`, and the
    history relative-path fix;
  - 2.0.9: `HX-Location` and history `normalizePath` fixes;
  - 2.0.11: `<hx-partial>` backport, an `hx-push-url` fix for boosted
    elements.

**Option A — Stay on 2.0.3 (recommended)**

- Pros:
  - a pure supply-chain change: identical bytes to what users run now;
  - P3's "route tests unchanged" means what it says;
  - matches the brief's non-goal.
- Cons: two years of bug fixes not taken, including boost and history fixes.
- Risk: lowest.

**Option B — Move to 2.0.11 in this mission**

- Pros: current, with the bug fixes; three-way checksum agreement (npm, docs
  SRI, GitHub digest).
- Cons:
  - changes client behaviour on every page (history, parsing) with no
    browser tests in the repo. The route suite can't see JS regressions, so
    the only check is the manual browser smoke;
  - widens the brief past its non-goal.
- Risk: low to medium.

**Recommendation: A**, with a follow-up brief "upgrade vendored htmx to the
latest 2.0.x". That one is a one-file swap plus checksum constants, verified
by a browser smoke of boost, back/forward and the ingredient autocomplete.

### Q3 — Where the checksum lives, and how the test computes it (platform-owner)

The brief lets the agent choose the location, but the computation needs a
dependency decision, so both are asked here.

**Location**

- **(a) Test constants (recommended).** In `templates.rs`'s new `tests`
  module:
  - `HTMX_PATH`, `HTMX_SHA256` and `HTMX_SRI_SHA384`;
  - a doc comment carrying the tarball URL and npm `dist.integrity`.
  - Changing the file without changing the constant fails CI. Changing both
    is a reviewed Rust diff that shows the provenance comment.
- **(b) `assets/vendor/SHA256SUMS`.** Usable with `sha256sum -c` by hand.
  - But `assets/` is served as-is, so it would be public at
    `/assets/vendor/SHA256SUMS`. That's harmless, but not an asset.
  - MB-0024 would embed it too.
  - A non-served path (`vendor.sha256` at the root) avoids that, at the cost
    of one more top-level file.

**Computation**

- **(i) `sha2 = "0.10.9"` as a `[dev-dependencies]` entry in
  `crates/kauppa-web/Cargo.toml` (recommended).**
  - Already locked at that version, so `Cargo.lock` gains no new package,
    only an edge.
  - Supply-chain check (crates.io, 2026-11-08):

    | Crate | Version | Owners | Downloads (all / recent) | License | Yanked / advisories |
    | --- | --- | --- | --- | --- | --- |
    | sha2 | 0.10.9 (latest 0.11.1) | tarcieri, newpavlov, `github:rustcrypto:hashes` | 1.19 B / 365 M | MIT OR Apache-2.0 | no / none |

  - The same crate gives SHA-384 for the SRI check (Q4).
  - Pinned to 0.10.9 rather than 0.11, so it doesn't duplicate the locked
    version.
  - Dev-only: not in the release binary.
- **(ii) Hand-written SHA-256 in the test module** (about 60 lines). No
  manifest change, but crypto code nobody should have to review.
- **(iii) Check outside `cargo test`.** A `sha256sum -c` step in
  `fast-gate.sh` or CI (`scripts/agentic/` and `.github/` are platform
  CODEOWNERS paths). It needs a brief delta, because P2's check is a
  `cargo test` name.

**Recommendation: (a) + (i).** This CRP doubles as the dependency-bump
runbook's consult for the dev-dependency: current state, target,
alternatives and supply-chain check are above. `cargo deny check` is
expected to be unchanged, since no new package or license is added.

### Q4 — File location and tag (platform-owner; brief: agent may decide)

- **Path:** `assets/vendor/htmx-2.0.3.min.js`, the minified file.
  - 50 KB, or about 16 KB gzipped. Mind the 3G goal.
  - The version in the name keeps a later upgrade from being served from a
    stale cache.
- **No license file:** htmx is 0BSD (`package.json`), which requires no
  notice. The provenance comment names the license.
- **`.gitattributes`:** add `assets/vendor/** -text`, so no checkout
  normalizes the bytes. It's a new root file but not a CODEOWNERS path.
- **Tag:**
  `<script src="/assets/vendor/htmx-2.0.3.min.js" integrity="sha384-0895/pl2MU10Hqc6jd4RvrthNlDiE9U1tWmX7WRESftEDRosgxNsQG/Ze9YMRzHq" defer></script>`.
  - Sub-question: keep `integrity` on a same-origin script? Recommendation:
    **yes**. The browser then refuses a file that differs at serve time, for
    example one tampered with on the host.
  - It's the value htmx's docs publish.
  - P1's test also asserts the attribute equals `HTMX_SRI_SHA384`.
  - Cost: one more value to change on upgrade.

**Recommendation:** as above, with `integrity`.

### Q5 — Add a Content-Security-Policy now? (security)

What htmx 2.0.3 needs under a strict CSP (read from `dist/htmx.js` in the
npm tarball):

- **`eval`.** `htmx.config.allowEval` **defaults to `true`** (line 154).
  - It isn't eval by default. `eval`/`Function` run only through
    `maybeEval`, for features this app doesn't use: trigger filters,
    `hx-on*`, `js:` values in `hx-vals`/`hx-vars`, and `htmx.eval`-style
    calls.
  - So `script-src 'self'` without `'unsafe-eval'` breaks nothing here.
  - The default is *on*, though: if a template ever adds one of those
    features, it fails under CSP rather than silently using eval. Setting
    `allowEval: false` makes that explicit.
- **Inline styles.** `includeIndicatorStyles: true` makes htmx insert a
  `<style>` element for `.htmx-indicator` at load (line 4959), with a nonce
  only if `inlineStyleNonce` is set.
  - A `style-src 'self'` policy blocks it, so loading indicators lose their
    default styling.
  - Fix: `includeIndicatorStyles: false` plus the two rules moved into
    `app.css`, or allow inline styles.
- **Swapped `<script>`.** `allowScriptTags: true` makes htmx re-create
  `<script>` elements in swapped content. Inline ones are blocked by
  `script-src 'self'` anyway; the app renders none.
- **Request origin.** `selfRequestsOnly: true` is the default, which suits a
  same-origin app.
- **Where config would go.** These settings go in
  `<meta name="htmx-config" content='{…}'>` in `layout.hbs`: a template
  change, not a script.

**Option A — Not in this PR; a follow-up brief, security (recommended)**

- Vendoring is what makes `script-src 'self'` possible. The policy itself
  touches different ground:
  - the header wiring in `main.rs` (a security path; `/assets` sits outside
    `routes::configure`, so a scope-level wrap wouldn't cover it, much as with
    RR-0010's ordering);
  - the htmx meta config, which changes htmx behaviour (a non-goal here);
  - indicator CSS;
  - possibly `Report-Only` first.
- Sketch for that brief:
  - `default-src 'self'; script-src 'self'; style-src 'self';
    img-src 'self' data:; form-action 'self'; frame-ancestors 'none';
    base-uri 'none'; object-src 'none'`;
  - plus `htmx-config` `{"allowEval":false,"includeIndicatorStyles":false}`.
- Pros: keeps MB-0027 a byte-identical swap; the CSP gets its own tests and
  rollback.
- Cons: the protection arrives one PR later.

**Option B — Add the header in this PR**

- Pros: closes the loop at once.
- Cons:
  - widens the brief past its non-goal;
  - pulls in a `main.rs` security-path change and an htmx behaviour change.
- Risk: low to medium.

**Option C — Set it in Caddy only (MB-0022)**

- Pros: no app change.
- Cons: self-hosters without Caddy don't get it (AGENTS.md goal 3), and the
  route tests can't see it.

**Recommendation: A.** No CSP in MB-0027; open a follow-up brief.

### Q6 — Interaction with MB-0024 (embedded templates and assets) (platform-owner)

- **Order-independent.** P2's test reads the file from
  `CARGO_MANIFEST_DIR/../../assets/vendor/…`, so it doesn't care how
  `/assets` is served.
- **If MB-0027 lands first:**
  - MB-0024's `assets::tests::serves_every_asset` will enumerate the vendored
    file and must serve it under its nested path with `text/javascript`, the
    content type actix-files gives it today. That's within MB-0024's P2
    ("same content type").
  - Its 1 MB binary-growth threshold has room: +50 KB.
- **If MB-0024 lands first:** this mission adds one file under `assets/`,
  which MB-0024's build-time list picks up on rebuild. Its forbidden "change
  asset paths" isn't touched, because a new path isn't a changed one.
- **MB-0022's image** copies `assets/` until MB-0024 lands, so the vendored
  file ships either way. Its smoke script could curl
  `/assets/vendor/htmx-2.0.3.min.js` too (a note for MB-0022's pack, not a
  change here).

No decision needed beyond "no ordering constraint". Recorded for the RR.

### Q7 — AGENTS.md do-not-touch note (RR roll-back) (platform-owner)

Current: "`assets/` is served directly; nothing executable may be committed
there beyond vendored, pinned JS from a reviewed source (right now htmx is
loaded from CDN; vendoring it is a platform decision)."

Proposed replacement, for the RR to roll back. The agent doesn't edit
AGENTS.md (MB-0027 forbidden).

> `assets/` is served directly; nothing executable may be committed there
> except under `assets/vendor/`: pinned third-party JS from a reviewed source,
> byte-for-byte, with its checksum checked in the test suite (htmx 2.0.3 from
> the npm registry, RR-<id>). Adding, upgrading or modifying a vendored file is
> a platform decision.

Optionally, the Map table's "Client interactivity" row could note
`assets/vendor/` (htmx, vendored).

## Recommendation and question

| Q | Recommendation |
| --- | --- |
| Q1 Source | npm registry tarball, verified by `dist.integrity` and cross-checked against the SRI in htmx's docs at `v2.0.3` |
| Q2 Version | Stay on 2.0.3; upgrade to the latest 2.0.x in a follow-up brief (no advisories against 2.0.3) |
| Q3 Checksum | Test constants in `templates.rs` tests; `sha2 = "0.10.9"` dev-dependency (already locked, no new package) |
| Q4 Location | `assets/vendor/htmx-2.0.3.min.js`, `integrity="sha384-…"` on the tag, `.gitattributes` `-text` |
| Q5 CSP | Not in this PR; follow-up security brief (policy sketch above) |
| Q6 MB-0024 | No ordering constraint |
| Q7 AGENTS.md | Replace the do-not-touch bullet with the wording above, as the RR's roll-back |

**Crisp question to answer:** Approve vendoring **htmx 2.0.3 from the npm
tarball** into `assets/vendor/htmx-2.0.3.min.js`, with the checksum as test
constants computed with a **`sha2` dev-dependency**, an `integrity`
attribute, **no CSP in this PR**, and the AGENTS.md wording in Q7? Or name
the deviating option per question.

## Blast radius and rollback

- **What can break:**
  - htmx not loading, from a wrong path, a blocked SRI or a wrong content
    type. The app degrades to plain forms and full page loads (by design),
    with nothing lost.
  - A CI failure if the file's bytes change.
- **Who is affected:** every page view's interactivity. No data, schema,
  session or auth path is touched.
- **Rollback plan:** revert the PR. `layout.hbs` goes back to the unpkg URL,
  and the vendored file and dev-dependency are removed.

## Minimal context

- MB-0027 P1 (no external script origin), P2 (checksum in the test suite),
  P3 (served as JavaScript; route tests unchanged).
- Forbidden: any other executable in `assets/`, modifying the vendored file,
  editing AGENTS.md.
- Sibling briefs: MB-0024 (embedding), MB-0022 (deploy image copies
  `assets/`).

## Evidence bundle (Hardened)

All fetched 2026-11-08 into the agent's scratchpad. Nothing was vendored or
committed.

- **npm metadata** (`https://registry.npmjs.org/htmx.org`): `dist-tags`
  `latest: 2.0.11`, `next: 4.0.1`.
  - **2.0.3** (published 2024-10-04):
    - `integrity` `sha512-AeoJUAjkCVVajbfKX+3sVQBTCt8Ct4lif1T+z/tptTXo8+8yyq3QIMQQe/IT+R8ssfrO1I0DeX4CAronzCL6oA==`;
    - publisher `carsongross`;
    - registry signature keyid `SHA256:jl3bwswu80PjjokCgh0o2w5c2U4LhQAE57gj9cz1kzA`.
  - **2.0.11:** `sha512-Thx/WtpeOQqSrqBCw/A1cwGJGg4UrVa3+sW0GmrM3p4gJgO89ecH4qtbnyzDDWFvBTqjnIMCgELTNt636dtamA==`.
- **Tarball check:**
  - `openssl dgst -sha512 -binary htmx.org-2.0.3.tgz | base64` equals the
    published `integrity` (same for 2.0.11).
  - `package/dist/htmx.min.js`:
    - 2.0.3: 50,387 B, sha256
      `491955cd1810747d7d7b9ccb936400afb760e06d25d53e4572b64b6563b2784e`, sha384
      (base64) `0895/pl2MU10Hqc6jd4RvrthNlDiE9U1tWmX7WRESftEDRosgxNsQG/Ze9YMRzHq`;
    - 2.0.11: 52,182 B, sha256
      `d6fdc75f204e6bdefa99b69bf1e6d4ac69b8a364f77929f45c13476b4000f717`, sha384
      `2OatzQy1H+Zd/IIrjr1TcuDGqLXeHhbooAyJY1KdQMKnr4LZ22k31GBLdYKHmVjg`.
- **unpkg:** `https://unpkg.com/htmx.org@2.0.3` and
  `/htmx.org@2.0.3/dist/htmx.min.js` both hash to the npm file's sha256
  (same for 2.0.11).
- **htmx docs SRI.** `gh api repos/bigskysoftware/htmx/contents/www/content/docs.md?ref=v2.0.3`
  carries `integrity="sha384-0895/pl2MU10Hqc6jd4RvrthNlDiE9U1tWmX7WRESftEDRosgxNsQG/Ze9YMRzHq"`,
  equal to the computed sha384. At `v2.0.11` it carries
  `sha384-2OatzQy1H+Zd/IIrjr1TcuDGqLXeHhbooAyJY1KdQMKnr4LZ22k31GBLdYKHmVjg`,
  also equal.
- **GitHub releases** (`gh api repos/bigskysoftware/htmx/releases/tags/…`):
  - `v2.0.3` assets have no `digest`; `htmx.min.js` is 50,387 B;
  - `v2.0.11` `htmx.min.js` has `digest: sha256:d6fdc75f…00f717`, equal to
    the npm file.
- **Advisories:** `gh api '/advisories?ecosystem=npm&affects=htmx.org'` → none.
  `gh api repos/bigskysoftware/htmx/security-advisories` → none.
- **Changelog:**
  <https://raw.githubusercontent.com/bigskysoftware/htmx/v2.0.11/CHANGELOG.md>,
  2.0.4–2.0.11 as summarized in Q2.
- **htmx source** (`dist/htmx.js` 2.0.3):
  - config defaults: `allowEval: true` (l.154), `allowScriptTags: true`
    (l.160), `inlineStyleNonce: ''` (l.172), `selfRequestsOnly: true`
    (l.241), `includeIndicatorStyles: true` (l.118);
  - `maybeEval` gates `eval`/`Function` on `allowEval` (l.3811);
  - indicator `<style>` insertion at l.4959.
  - 2.0.11 has the same defaults.
- **License:** `package.json` `"license": "0BSD"` (2.0.3 and 2.0.11).
- **sha2:** crates.io API (owners, downloads, license, 0.10.9 not yanked);
  `Cargo.lock` has `sha2 0.10.9` via `cookie` 0.16.2 and sqlx.
- **Content type:** `mime_guess-2.0.5/src/mime_types.rs:579`:
  `("js", &["text/javascript"])`.
