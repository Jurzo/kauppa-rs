# Mission Brief — Serve a pinned, vendored copy of htmx

```yaml
---
id: MB-0027
owner: "@juuso-kalliomaki"
goal: "Stop loading htmx from unpkg at runtime: the app serves a pinned copy from `/assets/`, verified against its published checksum, so pages don't depend on a third-party CDN being up or uncompromised, and nothing on a page runs code from another origin."
non_goals:
  - Upgrading htmx beyond the version in use (2.0.3), unless the RR chooses to.
  - "A Content-Security-Policy header (a follow-up this makes possible)."
  - Any change to `hx-*` attributes or behaviour.
acceptance:
  - id: P1
    property: "`templates/layout.hbs` loads htmx from `/assets/` and no template references an external script origin."
    check: cargo test -p kauppa-web templates::tests::no_external_scripts
  - id: P2
    property: "The vendored file matches the SHA-256 checksum recorded in the repo for the pinned version and source, checked in the test suite so a changed file fails CI."
    check: cargo test -p kauppa-web templates::tests::vendored_htmx_checksum
  - id: P3
    property: "The file is served at its `/assets/` URL with a JavaScript content type, and the boosted-navigation and fragment-swap pages still work (route tests unchanged)."
    check: cargo test -p kauppa-web --all-targets
context:
  - templates/layout.hbs
  - assets/
  - crates/kauppa-web/src/templates.rs
  - AGENTS.md
autonomy:
  may_decide:
    - "the file name under `assets/` (e.g. `assets/vendor/htmx-2.0.3.min.js`) and where the checksum is recorded"
  escalate:
    - { when: "this brief's whole change: AGENTS.md says vendoring htmx is a platform decision, and `assets/` may only hold vendored JS from a reviewed source", to: platform-owner }
    - { when: "which source is reviewed (npm tarball, GitHub release) and whether to upgrade the version", to: platform-owner }
    - { when: "adding a `Content-Security-Policy` header in the same PR", to: security }
  forbidden:
    - "commit any other executable file to `assets/`"
    - modify the vendored file
    - "edit AGENTS.md (if the RR updates its do-not-touch note, that's the RR's roll-back)"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
  - .agentic/mentorship/security.md@v1
resolutions: [RR-0015]
---
```

## Context

Found in the deployment assessment (2026-11-08). `templates/layout.hbs`
loads `https://unpkg.com/htmx.org@2.0.3` with no `integrity` attribute.
Every page view therefore depends on unpkg's availability, and a
compromised CDN response would run with the app's origin. The app works
without htmx (forms post normally), so the risk is integrity more than
availability.

AGENTS.md → Do-not-touch: "`assets/` is served directly; nothing executable
may be committed there beyond vendored, pinned JS from a reviewed source
(right now htmx is loaded from CDN; vendoring it is a platform decision)."
So this mission starts with a short CRP to the platform owner.

If MB-0024 (embedded assets) lands first, the vendored file is embedded
along with the rest; the two don't conflict.

## Conceptual plan

1. **Clarify (S1).** CRP to platform-owner: source, version, checksum
   location; whether to add CSP now (security). Answered by RR-0015.
2. Download from the chosen source, verify against its published checksum,
   commit with the checksum; change the `<script>` tag.

## Validation plan

- P1, P2 as unit tests reading the templates and the vendored file.
- P3: the full web suite; smoke in a browser with the network tab showing
  no request to unpkg.
