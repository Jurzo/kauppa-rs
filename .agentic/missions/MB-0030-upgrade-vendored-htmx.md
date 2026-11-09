# Mission Brief — Upgrade the vendored htmx to the latest 2.0.x

```yaml
---
id: MB-0030
owner: "@juuso-kalliomaki"
goal: "Serve the latest htmx 2.0.x patch release instead of 2.0.3, vendored with the same provenance checks, so the app gets the 2.0 line's fixes without changing behaviour."
non_goals:
  - "htmx 2.1 or later minor versions, htmx 4, or any extension."
  - "Any change to `hx-*` attributes, `htmx-config`, or behaviour (MB-0029, MB-0031 own those)."
  - "Modifying the vendored bytes (minifying, patching, banners)."
acceptance:
  - id: P1
    property: "`assets/vendor/` holds exactly one htmx file, `htmx-<version>.min.js` for the version the RR approves, byte-for-byte from the approved source; `HTMX_PATH`, `HTMX_SHA256` and `HTMX_SRI_SHA384` and their provenance comment match it, and the 2.0.3 file is gone."
    check: cargo test -p kauppa-web templates::tests::vendored_htmx_checksum
  - id: P2
    property: "`layout.hbs` loads the new file with the matching `integrity`, and no template references an external script origin."
    check: cargo test -p kauppa-web templates::tests::no_external_scripts
  - id: P3
    property: "The new file is served at its `/assets/vendor/` URL as `text/javascript` (`SERVED_AS` row), and the old URL is 404."
    check: cargo test -p kauppa-web routes::assets::tests::serves_every_asset
  - id: P4
    property: "Boosted navigation, back/forward, list item fragments and the ingredient autocomplete work in a browser as with 2.0.3 (RR-0015 constraint 5), and the CRP lists the release notes between 2.0.3 and the target that touch features the app uses."
    check: "manual: browser smoke on the release binary, recorded in MB-0030's Continuity Pack"
context:
  - assets/vendor/
  - templates/layout.hbs
  - crates/kauppa-web/src/templates/tests.rs
  - crates/kauppa-web/src/routes/assets/tests.rs
  - .gitattributes
  - .agentic/resolutions/RR-0015-vendor-htmx.md
  - .agentic/consults/CRP-0015-vendor-htmx.md
autonomy:
  may_decide:
    - "the order of commits (file swap, constants, template)"
  escalate:
    - { when: "the version and source (RR-0015: any version other than 2.0.3 needs an RR; AGENTS.md: upgrading a vendored file is a platform decision)", to: platform-owner }
    - { when: "the npm `dist.integrity`, the docs SRI and the GitHub release digest disagree", to: platform-owner }
    - { when: "a release note between 2.0.3 and the target changes behaviour the app relies on (boost, history, swap defaults, `responseHandling`)", to: owner }
  forbidden:
    - "modify the vendored file, or commit any other executable file to `assets/`"
    - "load htmx from a CDN, even temporarily"
    - "edit AGENTS.md except as the RR's roll-back"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
  - .agentic/mentorship/security.md@v1
resolutions: []
---
```

## Context

RR-0015 constraint 5 opens this follow-up after MB-0027 (merged 2026-11-08,
#39). MB-0027 vendored 2.0.3 byte-for-byte from the npm tarball, the version
the app already loaded from unpkg. RR-0015 constraint 3 sets the provenance
checks for any vendored version: the npm tarball's `dist.integrity`, the
docs SRI, and from 2.0.11 on the GitHub release digest, all recorded in the
commit. AGENTS.md names "htmx 2.0.3 from the npm registry, RR-0015" in its
do-not-touch bullet, so the RR's roll-back updates that wording.

Do this before MB-0029 (form errors) and MB-0031 (CSP), which add
configuration that should be checked against the version actually served.

## Conceptual plan

1. **Clarify (S1).** CRP to platform-owner: target version, release notes
   since 2.0.3 that touch the app's usage, hashes from each source.
2. Swap the file, constants, `integrity` and `SERVED_AS` row in one PR.
3. Browser smoke (P4).

## Validation plan

- P1–P3: the named tests (they already exist; their constants change).
- P4: browser session on the release binary; network tab shows only
  same-origin requests.
