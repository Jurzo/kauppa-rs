# Consultation Request Pack — A browser-level test: which tool, where its dependencies are pinned, and whether it gates merges

```yaml
---
id: CRP-0041
mission: MB-0047
owner: "@juuso-kalliomaki"
target_role: platform
risk_tier: low
urgency: low
state: answered
runbook_trigger: "MB-0047 autonomy.escalate (the tool and its CI toolchain → platform-owner; where its dependencies are declared and pinned → platform-owner; whether the check is required for merge → owner; the Mentorship Pack edit → owner); feature.md escalation trigger `.agentic/{mentorship,resolutions}/**`"
---
```

## Decision statement

Choose how CI drives the built app in a real DOM for MB-0047's three flows
(a boosted wrong-password sign-in shows its error, a list item add and
check swap their fragment, boosted navigation updates the page), and fails
on a Content-Security-Policy violation or an htmx error. Decide the tool,
where its dependencies are declared and pinned, when the job runs, and
whether it is a required check.

## Terms

- **DOM emulator**: a JavaScript implementation of the DOM that runs in
  Node (jsdom). No layout, no real browser security model.
- **Headless browser**: a real browser engine (Chromium, Firefox) without a
  window, driven by a test program.
- **Playwright**: a Node library that drives Chromium, Firefox and WebKit.
  It downloads its own browser builds, pinned to the library's version.
- **WebDriver**: the W3C protocol for driving a browser; Rust clients
  (`thirtyfour`, `fantoccini`) talk to a driver process (`chromedriver`).
- **Required check**: a status check that branch protection makes every
  PR pass before merge (eight today, RR-0032).

## Current state

- No browser-level test exists. `actix-web.md` → Testing says "Reserve
  full E2E (Playwright) for user-visible flows"; there are none.
- MB-0029's first PR (#58) passed CI with `hx-ext` where htmx never reads
  it; the owner's manual check caught it. CRP-0025 reproduced it with an
  uncommitted jsdom 24.1.3 script that needed stubs for `XPathEvaluator`
  and `CSS.escape`. RR-0025 constraint 4: such wiring isn't done until it
  works in a browser.
- The app sends `Content-Security-Policy: default-src 'self'; script-src
  'self'; style-src 'self'; …` (`crates/kauppa-web/src/csp.rs`, RR-0019).
  htmx 2.0.11 and response-targets 2.0.4 are vendored under
  `assets/vendor/` (RR-0017, RR-0018).
- The repository has no `package.json`; Node is not used anywhere in CI.
  CI runs on `ubuntu-latest`, actions pinned by SHA (RR-0032), Dependabot
  covers actions, cargo, Docker images and the Rust toolchain.
- Registration is invite-only; `kauppa-web admin` creates the first
  account (MB-0023), so a test run can seed its own user.

Facts checked 2026-11-14 (npm registry, crates.io):

| Candidate | Version | Released | Notes |
| --- | --- | --- | --- |
| `playwright` | 1.65.1 | 2026-11-13 | one dependency (`playwright-core`, same version, none of its own); Node ≥ 20; browsers downloaded by `playwright install`, pinned to the library version |
| `jsdom` | 31.0.1 | 2026-11-13 | 18 direct dependencies; Node ≥ 22.22 |
| `thirtyfour` (Rust) | 0.38.3 | 2026-11-14 | WebDriver client; needs a `chromedriver` matching the browser |
| `fantoccini` (Rust) | 0.22.1 | 2026-02-28 | WebDriver client |

## Options and trade-offs

### Q1 — The tool

**A. Playwright (Node) with its pinned Chromium.**
- Real engine: CSP is enforced, and `securitypolicyviolation` events and
  console errors are observable, so P2 is a direct assertion.
- `package.json` + `package-lock.json` pin one package (plus its core);
  the browser build is pinned by that version, not by the runner image.
- CI cost: `npm ci` plus `playwright install --with-deps chromium`
  (a ~150 MB download, cacheable), then the release binary and the flows.
  Expected 1–3 minutes; the real figure goes in the pack (validation plan).
- Adds Node as a CI toolchain and an npm ecosystem for Dependabot.

**B. jsdom under Node.**
- Smallest download; reproduced #58 in CRP-0025.
- **Cannot meet P2**: jsdom doesn't implement Content-Security-Policy, so
  a CSP violation never happens there. It also needs stubs
  (`XPathEvaluator`, `CSS.escape`), each a place where it differs from a
  browser. 18 dependencies to pin; needs Node 22.
- Not recommended.

**C. A Rust WebDriver client (`thirtyfour`) with the runner's Chrome and
`chromedriver`.**
- No Node; the test is a Rust test (an ignored-by-default integration test
  or a separate crate).
- A new Rust dev-dependency with a sizeable tree (dependency-bump
  runbook). The browser is whatever `ubuntu-latest` ships that week, so a
  runner image update can change results without a commit; pinning it means
  downloading a Chrome for Testing build by hand.
- Console and CSP events need BiDi or injected listeners; more code to
  write than A.

**D. Do nothing** (keep the owner's manual checks). The #58 class of bug
stays invisible to CI; `actix-web.md` should then drop the Playwright line.

### Q2 — Where the dependencies live (for A)

- **A1.** `tests/browser/` with its own `package.json`, `package-lock.json`
  (`npm ci`, exact versions, `--ignore-scripts`), the flows, and a
  `scripts/browser/run.sh` that builds the release binary, seeds a user with
  `kauppa-web admin`, starts it on a loopback port with a temporary
  database, runs the flows, and stops it. Nothing under `assets/`. Dependabot
  gets an `npm` entry for `/tests/browser`, grouped, under RR-0032's
  standing approval as a new class, or each bump by hand.
- **A2.** The same at the repository root. More visible, but a root
  `package.json` suggests a JS build that doesn't exist.

### Q3 — When it runs and whether it gates merges

- A path filter: PRs touching `templates/`, `assets/`, `crates/kauppa-web/`,
  `tests/browser/` or the workflow itself (P3), and every push to `main`.
- **Required or not** (owner). A job skipped by a path filter never
  reports, and a required check that never reports blocks the PR. Options:
  (i) not required, at first; required once it has run green for a while;
  (ii) required, with the job always running and skipping its steps inside
  when no path matched, so it always reports.

## Recommendation and question

Recommended: Q1 A (Playwright, Chromium only); Q2 A1 (`tests/browser/`,
exact pins, `npm ci --ignore-scripts`, browser downloaded by the pinned
`playwright install`, Dependabot `npm` entry grouped, bumps by hand until
the owner extends RR-0032); Q3 path-filtered on PRs plus `main`, and
**not required at first** (i): the owner revisits after two weeks of runs,
and the pack records its timings and any flake.

Crisp questions:

1. Tool: Playwright with its pinned Chromium (A)? Or jsdom (B, can't check
   CSP), a Rust WebDriver crate (C), or no automated test (D)?
2. Pinning: `tests/browser/` with its own `package.json` and lockfile, `npm
   ci --ignore-scripts`, Node from `actions/setup-node` pinned by SHA with
   an exact version (A1)? Dependabot `npm` entry: bumps reviewed by hand,
   or a new standing-approval class?
3. Required check: not at first, revisit after two weeks (i), or required
   from the start with an always-reporting job (ii)?
4. `actix-web.md`: replace "Reserve full E2E (Playwright) for user-visible
   flows" with a line naming `scripts/browser/run.sh`, what it covers and
   when to add a flow (roll-back, after the RR is signed)?

## Blast radius and rollback

What can break:
- CI time: one more job on web-touching PRs (estimated 1–3 minutes).
- Flakes: a browser test that fails without a cause blocks nothing while
  it isn't required (Q3 i); timeouts use htmx's own events
  (`htmx:afterSwap`), not sleeps.
- Supply chain: an npm package and a downloaded browser in CI only; never
  in the image, never under `assets/`, and the test page loads nothing
  from another origin (brief: forbidden).

Who is affected: contributors (one more check on web PRs). No user-visible
change.

Rollback plan: delete the job and `tests/browser/`; the app doesn't depend
on them.

## Minimal context

- `.agentic/missions/MB-0047-browser-level-test.md`
- `.agentic/consults/CRP-0025-response-targets-on-body.md` (the jsdom
  reproduction)
- `crates/kauppa-web/src/csp.rs`
- `.github/workflows/ci.yml`, `.github/dependabot.yml`
- `.agentic/resolutions/RR-0032-ci-pinning-and-maintenance.md` (standing approval
  classes)
- `.agentic/mentorship/actix-web.md` → Testing

## Evidence bundle (Hardened)

```text
npm view playwright@1.65.1 dependencies engines
  → { "playwright-core": "1.65.1" }, node >=20
npm view playwright-core@1.65.1 dependencies
  → (none)
npm view jsdom@31.0.1 → 18 dependencies, node ^22.22.2 || ^24.15.0 || >=26
crates.io: thirtyfour 0.38.3 (2026-11-14), fantoccini 0.22.1 (2026-02-28)
grep -rn "package.json" (repository) → none
crates/kauppa-web/src/csp.rs:22  POLICY = "default-src 'self'; script-src 'self'; style-src 'self'; …"
.agentic/mentorship/actix-web.md:40  "Reserve full E2E (Playwright) for user-visible flows."
```

No trial was run: this machine has Node 20 (jsdom 31 needs 22) and no
browser; downloading one is part of the approved path, not of the consult.
The first CI run records the timings.

## Owner's answer (2026-11-14)

1. Playwright (A).
2. As recommended (A1): `tests/browser/` with its own `package.json` and
   lockfile, `npm ci --ignore-scripts`, Node from `actions/setup-node`
   pinned by SHA with an exact version; Dependabot `npm` entry, bumps
   reviewed by hand.
3. Not required at first (i).
4. Yes: `actix-web.md`'s Playwright line is replaced.

Recorded in RR-0041 (scaffold; the owner signs).
