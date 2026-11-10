# Mission Brief — A browser-level test for what markup tests can't see

```yaml
---
id: MB-0047
owner: "@juuso-kalliomaki"
goal: "CI exercises the app's htmx behaviour in a real DOM, so a change that renders correct markup but doesn't work in the browser fails before a person has to find it."
non_goals:
  - "Full end-to-end coverage of every page."
  - "Visual regression testing."
  - "Replacing the owner's manual checks for layout and feel."
acceptance:
  - id: P1
    property: "A test drives a built app through a DOM with the vendored htmx and extension loaded: a boosted wrong-password sign-in shows its error (the #58 regression), a list item add and check swap their fragment, and boosted navigation updates the page."
    check: "the command the RR approves, recorded here when the brief delta lands (for example `scripts/browser/run.sh`)"
  - id: P2
    property: "The same run fails on any Content-Security-Policy violation or htmx error reported during those flows."
    check: "the same command (CSP and console assertions)"
  - id: P3
    property: "The test runs in CI on pull requests that touch templates, assets or the web crate, with its tools pinned and no executable added under `assets/`."
    check: "manual: the CI run linked in MB-0047's Continuity Pack"
  - id: P4
    property: "`actix-web.md`'s testing section describes the test that exists (replacing the Playwright line) through an RR roll-back."
    check: "manual: the RR roll-back applied, recorded in MB-0047's Continuity Pack"
context:
  - assets/vendor/
  - templates/layout.hbs
  - .agentic/consults/CRP-0025-response-targets-on-body.md
  - .agentic/continuity/MB-0029.md
  - .agentic/mentorship/actix-web.md
  - .github/workflows/ci.yml
autonomy:
  may_decide:
    - "which flows beyond the three named are covered"
    - "the harness's file layout outside `assets/` and `crates/`"
  escalate:
    - { when: "the tool: jsdom under Node (needs stubs for `XPathEvaluator` and `CSS.escape`, CRP-0025), a headless browser (Playwright or similar), or a Rust WebDriver crate; each adds a toolchain or dependency to CI", to: platform-owner }
    - { when: "where its dependencies are declared and pinned (a `package.json` and lockfile are new to this repo)", to: platform-owner }
    - { when: "whether the check is required for merge", to: owner }
    - { when: "the Mentorship Pack edit", to: owner }
  forbidden:
    - "commit executables under `assets/`"
    - "load anything from another origin in the test page"
    - "add a Rust dependency without the dependency-bump runbook"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/templating.md@v1
  - .agentic/mentorship/actix-web.md@v1
resolutions: []
---
```

## Context

Found in the audit of 2026-11-10 (finding 10).

MB-0029's first PR (#58) put `hx-ext` where htmx never reads it. Its tests
checked the attributes were present, CI passed, and the owner's manual check
found that errors still didn't show. A jsdom script written during the fix
reproduced the failure and confirmed the repair, but it was never committed
(CRP-0025 describes it). RR-0025 constraint 4 now says such wiring isn't
done until it works in a browser, and nothing automated checks that.
`actix-web.md` reserves Playwright E2E tests that don't exist.

## Conceptual plan

1. **Clarify (S1).** CRP to platform-owner: tool, pinning, CI cost.
2. The harness and the three flows; CSP and error assertions.
3. CI job with a path filter; roll-back into `actix-web.md`.

## Validation plan

- Mutation check: move `hx-ext` back onto the form and see the test fail.
- CI timing recorded in the pack.
