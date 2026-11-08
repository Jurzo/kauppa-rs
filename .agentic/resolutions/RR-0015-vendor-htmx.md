# Resolution Record — Vendor htmx 2.0.3 into `assets/vendor/`

```yaml
---
id: RR-0015
closes: CRP-0015
mission: MB-0027
decided_by: "@juuso.kalliomaki"
role: platform
risk_tier: low
decided_at: 2026-11-08T18:25:00Z
status: approved
superseded_by: null
rolls_back_into:
  - AGENTS.md
  - .agentic/missions/MB-0027-vendor-htmx.md
  - .agentic/consults/CRP-0015-vendor-htmx.md
---
```

## Decision and outcome

Approve CRP-0015's **recommended set, Q1–Q7**: vendor **htmx 2.0.3** from
the **npm registry tarball**, byte-for-byte, into
`assets/vendor/htmx-2.0.3.min.js`, loaded with an `integrity` attribute,
with its checksums held as test constants computed with a **`sha2`
dev-dependency**, and **no Content-Security-Policy** in this mission.

Sub-answers:

1. **Source (Q1, A).** The reviewed source is
   `https://registry.npmjs.org/htmx.org/-/htmx.org-2.0.3.tgz`.
   - Before vendoring, the tarball's sha512 must equal the registry's
     `dist.integrity`
     (`sha512-AeoJUAjkCVVajbfKX+3sVQBTCt8Ct4lif1T+z/tptTXo8+8yyq3QIMQQe/IT+R8ssfrO1I0DeX4CAronzCL6oA==`).
   - The extracted `package/dist/htmx.min.js` must then match both:
     - sha256 `491955cd1810747d7d7b9ccb936400afb760e06d25d53e4572b64b6563b2784e`
       (50,387 bytes);
     - the SRI that htmx's own docs publish at tag `v2.0.3`:
       `sha384-0895/pl2MU10Hqc6jd4RvrthNlDiE9U1tWmX7WRESftEDRosgxNsQG/Ze9YMRzHq`.
   - Any mismatch stops the mission (slow mode); nothing is vendored.
   - The commit message records the tarball URL, `dist.integrity`, the
     sha256 and the docs SRI.
2. **Version (Q2, A).** Stay on **2.0.3**, the bytes users run today. An
   upgrade to the latest 2.0.x is a follow-up brief (constraint 5).
3. **Checksum location and computation (Q3, (a) + (i)).**
   - **Location:** constants in a new `#[cfg(test)] mod tests` in
     `crates/kauppa-web/src/templates.rs`: `HTMX_PATH`, `HTMX_SHA256` and
     `HTMX_SRI_SHA384`, with a doc comment naming the tarball URL, the npm
     `dist.integrity` and the 0BSD license. No `SHA256SUMS` file under
     `assets/`, and no root-level checksum file.
   - **Computation:** `sha2 = "0.10.9"` as a `[dev-dependencies]` entry in
     `crates/kauppa-web/Cargo.toml`. The same crate gives SHA-384 for the
     SRI check.
   - **Dependency approval.** This RR approves that dev-dependency in its
     own right, as the dependency-bump runbook's RR: `sha2` 0.10.9 is
     already in `Cargo.lock` (via `cookie` and sqlx), so the lockfile gains
     an edge and no package; it is dev-only and not in the release binary;
     the supply-chain check is CRP-0015 Q3's table. RR-0012 (MB-0023) also
     approves `sha2` 0.10 as a direct dependency and as this kauppa-web
     dev-dependency. Either RR suffices for MB-0027; RR-0015 stands alone
     if RR-0012 is signed later or rejected.
   - `cargo deny check` is expected to be unchanged; the PR shows its
     output.
4. **File and tag (Q4).**
   - Path: `assets/vendor/htmx-2.0.3.min.js`, the minified file. No
     license file (0BSD needs no notice).
   - New root `.gitattributes` with `assets/vendor/** -text`.
   - `templates/layout.hbs`:
     `<script src="/assets/vendor/htmx-2.0.3.min.js" integrity="sha384-0895/pl2MU10Hqc6jd4RvrthNlDiE9U1tWmX7WRESftEDRosgxNsQG/Ze9YMRzHq" defer></script>`.
     The `integrity` attribute stays, although the script is same-origin
     (the sub-question): the browser refuses a file changed on the host.
   - Tests: P1's `no_external_scripts` also asserts that the `integrity`
     value equals `HTMX_SRI_SHA384`. P2's `vendored_htmx_checksum` reads
     the file from `CARGO_MANIFEST_DIR/../../assets/vendor/…` and checks
     both hashes.
5. **CSP (Q5, A).** No `Content-Security-Policy` header and no
   `htmx-config` meta in this mission. A follow-up security brief takes it
   up, starting from CRP-0015 Q5's sketch (`script-src 'self'`, and
   `{"allowEval":false,"includeIndicatorStyles":false}` with the indicator
   rules in `app.css`).
6. **MB-0024 (Q6).** No ordering constraint. Whichever lands second picks
   up the other's change: MB-0024's asset list embeds the vendored file and
   serves it as `text/javascript`. MB-0022's image copies `assets/` until
   MB-0024 lands, so the file ships either way.
7. **AGENTS.md (Q7).** The do-not-touch bullet is replaced with the CRP's
   wording, and the Map table's "Client interactivity" row names
   `assets/vendor/` (roll-back below).

Covers:

- MB-0027's implementation PR, with P1–P3 as written;
- the `sha2 = "0.10.9"` dev-dependency in `crates/kauppa-web/Cargo.toml`
  and the lockfile edge it adds;
- the new `.gitattributes`;
- the AGENTS.md and brief changes below.

Does not cover:

- any htmx version other than 2.0.3, or any other vendored file;
- modifying the vendored bytes (minifying, patching, adding a banner);
- a CSP header, `htmx-config`, or any change to htmx behaviour or `hx-*`
  attributes;
- `sha2` as a non-dev dependency of `kauppa-web` (that is RR-0012's scope,
  for `kauppa-core`/MB-0023);
- a checksum check outside `cargo test` (CI or `fast-gate.sh` steps).

## Rationale and trade-offs

- **Q1 A — chosen.** The npm tarball is the canonical publication every
  CDN mirrors, and two independent published values (the registry
  integrity and the maintainers' docs SRI) agree with it. Anyone can repeat
  the check with `curl` and `openssl`.
  - **B (GitHub release) not chosen:** no published digest for 2.0.3, and
    its bytes weren't compared.
  - **C (unpkg) not chosen:** a mirror of A, and the CDN this brief stops
    trusting.
- **Q2 A — chosen.** A pure supply-chain change, byte-identical to
  production, so "route tests unchanged" (P3) means what it says. No
  advisories exist against 2.0.3.
  - **B (2.0.11) not chosen:** history, boost and parsing changes on every
    page, with no browser tests to catch a regression.
- **Q3 (a) + (i) — chosen.** A changed file fails CI; changing the constant
  is a reviewed Rust diff next to its provenance. `sha2` is already locked
  and widely used, and is dev-only.
  - **(b) not chosen:** a checksum file under `assets/` would be served
    publicly and embedded by MB-0024.
  - **(ii) not chosen:** hand-written crypto nobody should review.
  - **(iii) not chosen:** needs a brief delta (P2 is a `cargo test` name)
    and touches platform CODEOWNERS paths.
- **Q4 — chosen.** The versioned name avoids stale caches on upgrade;
  `-text` guards the bytes on any checkout; `integrity` catches tampering
  on the host for one extra constant per upgrade.
- **Q5 A — chosen.** Keeps MB-0027 a byte-identical swap. The CSP touches a
  security path in `main.rs` and changes htmx config, so it gets its own
  tests and rollback.
  - **B not chosen:** widens past the non-goal.
  - **C (Caddy only) not chosen:** self-hosters without Caddy miss it, and
    route tests can't see it.

## Approvals and authority

Approved by platform-owner @juuso.kalliomaki (Q1–Q4, Q6, Q7, and the `sha2`
dev-dependency under the dependency-bump runbook); security @juuso.kalliomaki
(Q5: no CSP in this mission, follow-up brief).

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0015-vendor-htmx.md`
- Mission Brief: `.agentic/missions/MB-0027-vendor-htmx.md`
- Runbook: `.agentic/runbooks/dependency-bump.md` (CRP-0015 Q3 is its
  consult for the dev-dependency)
- Related decisions: RR-0012 (MB-0023; `sha2` 0.10, sibling approval);
  RR-0010 (ordering of `/assets` ahead of `routes::configure`).
- Evidence links: CRP-0015 → Evidence bundle:
  - npm metadata and tarball check (`dist.integrity`, sha256, sha384);
  - unpkg bytes equal to npm;
  - htmx docs SRI at `v2.0.3`;
  - GitHub release assets (no 2.0.3 digest);
  - no advisories for `htmx.org`; CHANGELOG 2.0.4–2.0.11;
  - htmx 2.0.3 config defaults relevant to a later CSP;
  - 0BSD license; `sha2` crates.io check; mime_guess `.js` →
    `text/javascript`.

## Constraints and follow-on rules (Hardened)

1. **Only `assets/vendor/` holds executable files**, and only pinned
   third-party JS from a reviewed source, byte-for-byte. Adding another
   vendored file needs an RR.
2. **The vendored file is never edited.** Its sha256 and SRI are checked by
   `cargo test`. Changing the file means changing the version in its name,
   the constants and the `integrity` attribute together, under an RR.
3. **Verify before vendoring.** Every vendored version is checked against
   its registry integrity and at least one independent published value
   (the docs SRI; from 2.0.11 on, also the GitHub release digest), and the
   commit records them.
4. **No external script origins.** No template loads a script from another
   origin (P1's test). Reintroducing a CDN needs an RR.
5. **Follow-ups.** Two briefs are opened after MB-0027 merges:
   - "Upgrade vendored htmx to the latest 2.0.x" (platform): one-file swap,
     new constants, browser smoke of boost, back/forward and the ingredient
     autocomplete;
   - "Content-Security-Policy" (security): starting from CRP-0015 Q5's
     sketch.
6. **`sha2` stays a dev-dependency of `kauppa-web`** under this RR, at the
   locked 0.10.x. Moving it to `[dependencies]` there needs its own RR.

## Roll-back into the system (Hardened)

To be applied in MB-0027's implementation PR:

- `AGENTS.md` → Engineering intent → Do-not-touch boundaries: replace the
  bullet "`assets/` is served directly; nothing executable may be committed
  there beyond vendored, pinned JS from a reviewed source (right now htmx
  is loaded from CDN; vendoring it is a platform decision)." with:
  "`assets/` is served directly; nothing executable may be committed there
  except under `assets/vendor/`: pinned third-party JS from a reviewed
  source, byte-for-byte, with its checksum checked in the test suite (htmx
  2.0.3 from the npm registry, RR-0015). Adding, upgrading or modifying a
  vendored file is a platform decision."
- `AGENTS.md` → Map → "Where changes belong": the "Client interactivity"
  row's notes become "htmx first (vendored at `assets/vendor/`, RR-0015);
  add JS only when htmx can't."
- `MB-0027`: `resolutions: []` → `resolutions: [RR-0015]`; Conceptual plan
  step 1 "Stop until the RR." → "Answered by RR-0015."
- `CRP-0015`: `state: open` → `answered`.

## Supersession and history

Status: active once approved. Supersedes nothing. Overlaps RR-0012 only on
the `sha2` dev-dependency, which both approve; neither depends on the
other.
