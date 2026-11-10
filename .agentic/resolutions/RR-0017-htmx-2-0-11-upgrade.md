# Resolution Record — Upgrade the vendored htmx to 2.0.11

```yaml
---
id: RR-0017
closes: CRP-0017
mission: MB-0030
decided_by: "@juuso.kalliomaki"
role: platform
risk_tier: low
decided_at: 2026-11-10T08:00:00Z
status: approved
superseded_by: null
rolls_back_into:
  - AGENTS.md
  - .agentic/missions/MB-0030-upgrade-vendored-htmx.md
  - .agentic/consults/CRP-0017-htmx-2-0-x-upgrade.md
---
```

## Decision and outcome

Approve CRP-0017's **recommended set**: replace
`assets/vendor/htmx-2.0.3.min.js` with **htmx 2.0.11**, byte-for-byte from
the npm registry tarball, verified three ways, and accept the 2.0.4–2.0.11
behaviour changes with MB-0030's P4 browser smoke as the check.

Sub-answers (numbered as CRP-0017's crisp questions):

1. **Version (Q1, A).** htmx **2.0.11**, the latest 2.0.x (npm
   `dist-tags.latest`, 2026-11-09).
2. **Source (Q2).** The approved source is
   `https://registry.npmjs.org/htmx.org/-/htmx.org-2.0.11.tgz`, file
   `package/dist/htmx.min.js`. Before vendoring, all of these must hold:
   - tarball sha512 equals npm `dist.integrity`
     `sha512-Thx/WtpeOQqSrqBCw/A1cwGJGg4UrVa3+sW0GmrM3p4gJgO89ecH4qtbnyzDDWFvBTqjnIMCgELTNt636dtamA==`;
   - `htmx.min.js` is **52,182 B** with sha256
     `d6fdc75f204e6bdefa99b69bf1e6d4ac69b8a364f77929f45c13476b4000f717`;
   - its sha384 equals the docs SRI at tag `v2.0.11`
     (`www/content/docs.md`):
     `sha384-2OatzQy1H+Zd/IIrjr1TcuDGqLXeHhbooAyJY1KdQMKnr4LZ22k31GBLdYKHmVjg`;
   - its sha256 equals the GitHub release `v2.0.11` asset `htmx.min.js`
     digest `sha256:d6fdc75f204e6bdefa99b69bf1e6d4ac69b8a364f77929f45c13476b4000f717`.

   Any mismatch stops the mission (MB-0030 escalate → platform-owner);
   nothing is vendored. The commit message records the tarball URL,
   `dist.integrity`, the sha256, the docs SRI and the GitHub digest.
3. **Behaviour (Q3).** Accept the changes in CRP-0017 Q3's table, in
   particular the history cache moving from `localStorage` to
   `sessionStorage` (2.0.5) and restoration through the normal swap path.
   The check is P4's manual smoke on the release binary: boosted
   navigation, back/forward, a list item fragment, the `hx-confirm`
   dialogs and the ingredient autocomplete, recorded in MB-0030's
   Continuity Pack.
4. **AGENTS.md (Q5 in the CRP body).** The Do-not-touch bullet names the
   new version and this RR (roll-back below).
5. **Order.** MB-0030 merges before MB-0029 and MB-0031 are implemented.
   Their CRPs (CRP-0018, CRP-0019) may be answered in parallel.

Files (CRP-0017 Q4), in one PR, commit order the agent's:

- delete `assets/vendor/htmx-2.0.3.min.js`; add
  `assets/vendor/htmx-2.0.11.min.js`;
- `templates/layout.hbs`: `src="/assets/vendor/htmx-2.0.11.min.js"`,
  `integrity="sha384-2OatzQy1H+Zd/IIrjr1TcuDGqLXeHhbooAyJY1KdQMKnr4LZ22k31GBLdYKHmVjg"`;
- `crates/kauppa-web/src/templates/tests.rs`: `HTMX_PATH`, `HTMX_SHA256`,
  `HTMX_SRI_SHA384`, the provenance doc comment (tarball URL,
  `dist.integrity`, GitHub digest, 0BSD) and the length assertion
  `50_387` → `52_182`;
- `crates/kauppa-web/src/routes/assets/tests.rs`: the `SERVED_AS` row
  becomes `("vendor/htmx-2.0.11.min.js", "text/javascript")`, and
  `serves_every_asset` asserts `/assets/vendor/htmx-2.0.3.min.js` is 404.

Covers:

- MB-0030's implementation PR, with P1–P4 as written;
- the file swap, constants, `integrity` and `SERVED_AS` changes above;
- the AGENTS.md and brief changes below.

Does not cover:

- any htmx version other than 2.0.11 (2.1+, 4.x), or any extension
  (CRP-0018 / MB-0029 decide the response-targets extension separately);
- modifying the vendored bytes;
- any `hx-*`, `hx-history` or `htmx-config` change (MB-0029, MB-0031),
  including the history-cache finding, which CRP-0018 answers;
- any `Cargo.toml`, `Cargo.lock` or `.gitattributes` change.

## Rationale and trade-offs

- **Q1 A (2.0.11) — chosen.** Carries every 2.0 fix, including the
  `sessionStorage` history cache (narrows the secret-in-history finding),
  the history relative-path fixes and the 2.0.7 indicator rules MB-0031
  copies. Three independent values agree on the bytes. Config defaults
  MB-0029 and MB-0031 rely on (`responseHandling`, `allowEval`,
  `includeIndicatorStyles`, `htmx-config` parsing) are unchanged from
  2.0.3.
  - **B (2.0.6) not chosen:** misses 2.0.7–2.0.11 fixes, has no GitHub
    digest (two checks only), and a later bump repeats the procedure.
  - **C (stay on 2.0.3) not chosen:** keeps the `localStorage` history
    cache, and MB-0029/MB-0031 would be configured against a version
    upgraded later.
- **Q2 — chosen.** Same procedure as RR-0015 sub-answer 1, plus the GitHub
  digest RR-0015 constraint 3 requires from 2.0.11 on. The release's later
  publish date (2026-11-01, created 2026-09-22) doesn't weaken the
  GitHub-computed asset digest. No advisories against `htmx.org`.
- **Q3 — accepted.** No release note between 2.0.4 and 2.0.11 touches a
  feature the app uses in a way the server can see; the app degrades to
  plain forms if htmx fails to load. With no browser tests, the manual
  smoke is the proportionate check.
- **Order — chosen.** MB-0029 and MB-0031 change the same `<head>` and
  should be checked in a browser against the version users get.

## Approvals and authority

Approved by platform-owner @juuso.kalliomaki.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0017-htmx-2-0-x-upgrade.md`
- Mission Brief: `.agentic/missions/MB-0030-upgrade-vendored-htmx.md`
- Related decisions: RR-0015 (vendoring rules, constraint 5 opens this
  mission); CRP-0018 (MB-0029, history-cache finding); CRP-0019 (MB-0031,
  CSP and indicator styles).
- Evidence links: CRP-0017 → Evidence bundle:
  - npm `dist-tags` (`latest: 2.0.11`, `next: 4.0.1`), 2.0.x publish dates,
    publisher `carsongross`, 0BSD;
  - 2.0.11 tarball sha512 = `dist.integrity`; `htmx.min.js` sha256/sha384;
  - `gh api repos/bigskysoftware/htmx/releases/tags/v2.0.11` asset digest;
  - docs SRI at `v2.0.11` (blob `bdff5c7a`);
  - advisories: none; CHANGELOG 2.0.4–2.0.11; `dist/htmx.js` diff of
    config defaults, `responseHandling`, indicator styles and history
    storage.

## Constraints and follow-on rules (Hardened)

1. **One htmx file.** `assets/vendor/` holds exactly one htmx file,
   `htmx-2.0.11.min.js`; the 2.0.3 file is deleted in the same PR. No CDN
   load, even temporarily (RR-0015 constraint 4).
2. **Byte-for-byte, never edited.** The file is the tarball's
   `package/dist/htmx.min.js` unchanged; `-text` in `.gitattributes`
   stays. No minifying, patching or banner.
3. **Checksums in the test suite.** `HTMX_SHA256`, `HTMX_SRI_SHA384` and
   the 52,182 B length are checked by
   `templates::tests::vendored_htmx_checksum`; the `integrity` attribute
   equals `HTMX_SRI_SHA384` (`no_external_scripts`). File name, constants,
   length, `integrity` and `SERVED_AS` change together, and only under an
   RR (RR-0015 constraint 2).
4. **Three-way verification.** Any later htmx version is checked against
   npm `dist.integrity`, the docs SRI at its tag and the GitHub release
   digest, all recorded in the commit (RR-0015 constraint 3).
5. **Order.** MB-0029 and MB-0031 implement only after MB-0030 merges.
6. **RR-0015 otherwise stands.** Its constraints 1–4 and 6 apply to
   2.0.11 unchanged; constraint 5's upgrade follow-up is closed by
   MB-0030.

## Roll-back into the system (Hardened)

To be applied in MB-0030's implementation PR:

- `AGENTS.md` → Engineering intent → Do-not-touch boundaries: in the
  bullet beginning "`assets/` is served directly", replace
  "(htmx 2.0.3 from the npm registry, RR-0015)" with
  "(htmx 2.0.11 from the npm registry, RR-0017)". The bullet then reads:
  "`assets/` is served directly; nothing executable may be committed there
  except under `assets/vendor/`: pinned third-party JS from a reviewed
  source, byte-for-byte, with its checksum checked in the test suite (htmx
  2.0.11 from the npm registry, RR-0017). Adding, upgrading or modifying a
  vendored file is a platform decision."
- `AGENTS.md` → Map → "Client interactivity" row: unchanged ("htmx first
  (vendored at `assets/vendor/`, RR-0015); …"); it names no version, and
  RR-0015 still holds the vendoring rules.
  <!-- check: CRP-0017 Q5 says "Nothing else in AGENTS.md changes"; if the owner wants the Map row to cite both, use "(vendored at `assets/vendor/`, RR-0015, RR-0017)". -->
- `MB-0030`: `resolutions: []` → `resolutions: [RR-0017]`; Conceptual plan
  step 1 gains "Answered by RR-0017."
- `CRP-0017`: `state: open` → `answered`.

## Supersession and history

Status: active once approved. Supersedes nothing; amends RR-0015's version
pin (2.0.3 → 2.0.11) and leaves its vendoring rules in force. RR-0015 is
not edited. CRP-0018 (MB-0029) builds on this version.
