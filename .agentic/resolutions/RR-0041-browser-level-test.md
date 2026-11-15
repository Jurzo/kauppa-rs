# Resolution Record — A browser-level test: Playwright with its pinned Chromium, in `tests/browser/`, not required at first

```yaml
---
id: RR-0041
closes: CRP-0041
mission: MB-0047
decided_by: "@Jurzo"
role: platform
risk_tier: low
decided_at: 2026-11-15T07:00:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0047-browser-level-test.md
  - .agentic/consults/CRP-0041-browser-level-test.md
  - .agentic/mentorship/actix-web.md
---
```

## Decision and outcome

Approve CRP-0041 Q1 A, Q2 A1, Q3 (i) and Q4.

Sub-answers:

1. **The tool (Q1 A, platform).** Playwright driving its own Chromium
   build, pinned by the Playwright version. Chromium only. No jsdom (it
   can't observe CSP violations, P2), no Rust WebDriver crate.
2. **Pinning (Q2 A1, platform).**
   - `tests/browser/` holds `package.json` and `package-lock.json` with
     exact versions (`playwright` and nothing else at the top level), the
     flows, and nothing executable under `assets/`.
   - CI installs with `npm ci --ignore-scripts`, then
     `npx playwright install --with-deps chromium` from the pinned version.
     Node comes from `actions/setup-node`, pinned by SHA (RR-0032), with an
     exact Node version in the workflow.
   - `scripts/browser/run.sh` builds the release binary, seeds a user with
     `kauppa-web admin`, starts the app on a loopback port with a temporary
     database, runs the flows and stops the app. It is P1's and P2's check.
   - `.github/dependabot.yml` gains an `npm` entry for `/tests/browser`,
     grouped. Its PRs are reviewed by hand; they are not a class of
     RR-0032's standing approval.
3. **When it runs (Q3 (i), owner).** A CI job on pull requests that touch
   `templates/`, `assets/`, `crates/kauppa-web/`, `tests/browser/`,
   `scripts/browser/` or the job's workflow, and on every push to `main`.
   It is **not** a required check. The owner revisits after two weeks of
   runs; MB-0047's pack records timings and any flake for that.
4. **`actix-web.md` (Q4, owner).** "Reserve full E2E (Playwright) for
   user-visible flows." is replaced by a line naming
   `scripts/browser/run.sh`, what it covers (htmx wiring, CSP, the three
   flows) and when a flow is added (a change that wires htmx or its
   extensions in a way markup tests can't prove, RR-0025 constraint 4).

Covers:

- MB-0047's implementation PR: `tests/browser/`, `scripts/browser/run.sh`,
  the CI job, the Dependabot entry;
- the roll-backs below.

Does not cover:

- other browsers, visual regression, or making the check required (a later
  decision by the owner);
- any Rust dependency, or anything executable under `assets/`;
- loading anything from another origin in the test page (brief: forbidden).

## Rationale and trade-offs

- **A real engine over an emulator**: P2 asks for CSP violations, which
  only a browser enforces; jsdom also needs stubs where it differs from
  browsers (CRP-0025).
- **Playwright over WebDriver**: the browser is pinned by the library
  version instead of drifting with the runner image, and console and CSP
  events are first-class; one npm package with no further dependencies.
- **Not required at first**: a new browser job's flake rate is unknown; a
  required check that flakes blocks every web PR.

## Approvals and authority

@Jurzo as platform for sub-answers 1 and 2; as owner for sub-answers 3 and 4.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0041-browser-level-test.md`
- Mission Brief: `.agentic/missions/MB-0047-browser-level-test.md`
- Evidence: CRP-0041 → Evidence bundle.

## Constraints and follow-on rules (Hardened)

1. Waits use htmx's events or Playwright's auto-waiting, never fixed
   sleeps.
2. The run fails on any `securitypolicyviolation` event, console error or
   `htmx:*Error` event during the flows (P2).
3. The test page loads only the app's origin.
4. A Playwright bump is a hand-reviewed PR; it updates the browser with it.

## Roll-back into the system (Hardened)

To be applied after this record is signed, with the owner's OK:

- `MB-0047`: P1's and P2's check → `scripts/browser/run.sh`; P3's check
  names the CI job; `resolutions: [RR-0041]`; plan step 1 marked done.
- `CRP-0041`: `state: open` → `answered`.
- `.agentic/mentorship/actix-web.md` → Testing: the Playwright line
  replaced as sub-answer 4 says, in the implementation PR (P4), once the
  test exists.

## Supersession and history

Status: active once signed. Supersedes nothing.
