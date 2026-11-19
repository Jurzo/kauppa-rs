# Mission Brief — List each package's tests once in the brief-test check

```yaml
---
id: MB-0067
owner: "@juuso-kalliomaki"
goal: "`scripts/agentic/check_brief_tests.sh` gives the same verdict as today (every `cargo test` check in a Mission Brief selects at least one test) but lists each package's tests once per run instead of starting cargo for every check, so the CI step drops from 74–90 s to a few seconds."
non_goals:
  - "Changing what the check guarantees (MB-0019 P4, RR-0009 constraint 2), which brief lines it reads, or which briefs it skips."
  - "Removing or changing the `cargo test -p kauppa-core --all-targets` step (RR-0005; its constraint 3 needs a consult)."
  - "Changing any other CI step, the workspace `cargo test`, or the step order in `ci.yml`."
  - "Running doc tests, or changing the check format briefs use."
  - "Caching listings between runs."
acceptance:
  - id: P1
    property: "On `main`, the new script prints the same per-check lines (brief id, command, number of tests selected) and the same final line as the current script, and exits with the same code."
    check: "manual: both scripts' outputs diffed on the same commit, recorded in MB-0067's Continuity Pack"
  - id: P2
    property: "A run starts cargo at most twice per package (its targets' list and its doc tests' list), whatever the number of checks; the CI step takes under 10 s."
    check: "manual: the step's time in a green CI run on the PR, recorded in MB-0067's Continuity Pack"
  - id: P3
    property: "It fails, naming the brief and the check, on: a filter that selects no test; a `--test <name>` check that selects nothing in that target; a check whose shape it doesn't recognise (an unknown flag or package); and a listing that cargo fails to produce (never read as zero tests or as a pass)."
    check: scripts/agentic/test_check_brief_tests.sh
  - id: P4
    property: "Matching follows cargo: a check's filter selects the tests whose path contains it, in the packages and targets the command names; a check without a filter selects all of them; `--workspace` means every workspace package. Briefs whose pack says \"In flight: … not started\" are still skipped."
    check: scripts/agentic/test_check_brief_tests.sh
context:
  - scripts/agentic/check_brief_tests.sh
  - scripts/agentic/test_check_triggers.sh
  - .github/workflows/ci.yml
  - .github/workflows/agentic-gate.yml
  - .agentic/missions/MB-0019-split-source-files.md
  - .agentic/missions/MB-0045-ci-supply-chain-and-toolchain.md
  - .agentic/resolutions/RR-0009-source-layout-and-test-paths.md
  - .agentic/resolutions/RR-0032-ci-pinning-and-maintenance.md
  - .agentic/resolutions/RR-0005-kauppa-core-standalone-build.md
autonomy:
  may_decide:
    - "how the script parses check lines and matches filters (bash, or Python like `check_triggers.py`), within P3 and P4"
    - "how the self-test feeds the script canned listings and fixture briefs (for example an environment variable naming a directory), so the self-test needs no Rust build"
    - "the wording of the script's output, as long as P1's lines stay the same"
  escalate:
    - { when: "running the self-test in CI (a step in `agentic-gate.yml`'s `agentic scripts` job, where RR-0032 sub-answer 4 placed the other script tests), or any other workflow change", to: platform-owner }
    - { when: "a check shape in a brief that the new matching can't treat exactly as cargo does (rewrite the brief's check, or accept the difference)", to: owner }
    - { when: "anything that changes what the check guarantees, or the kauppa-core step (RR-0005)", to: owner }
  forbidden:
    - "rewrite a closed brief's check to make it pass (a moved test's path is updated per RR-0009 constraint 2, in its own PR)"
    - "skip a brief or a check that today's script checks"
    - "add dependencies"
    - "edit Runbooks, Mentorship Packs or RRs"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
resolutions: []
---
```

## Context

Requested by the owner (2026-11-19), after asking whether the
`fmt / clippy / test` job runs tests more than once.

Step times in the last three green CI runs on `main`:

| Step | Time |
| --- | --- |
| `cargo test --workspace --all-targets` | 114–168 s (each test once) |
| `scripts/sqlx-prepare.sh --check` | 2 s |
| `cargo test -p kauppa-core --all-targets` | 8–11 s (kauppa-core's 111 tests again, without feature unification: RR-0005, kept) |
| `scripts/agentic/check_brief_tests.sh` | 74–90 s |

- **The brief-test check runs no tests.** For each of the 257 `check:`
  lines that is a single `cargo test` command (218 distinct), it runs the
  command with `-- --list` and counts `: test$` lines. Each of those
  starts cargo, checks that the build is fresh and runs the test binaries
  to list them; that is where the time goes.
- **The check shapes today:** `-p kauppa-web <filter>` 166;
  `-p kauppa-core <filter>` 55; `-p kauppa-web --all-targets` 14;
  `--workspace --all-targets` 8; `--test <name>` 1; bare
  `-p kauppa-core` 1.
- **What cargo selects.** `cargo test -p P <filter>` runs P's library,
  binary and integration-test targets and its doc tests, and keeps the
  tests whose path contains the filter. `--all-targets` drops doc tests;
  `--test <name>` keeps only that integration-test target.
- **A prototype** (scratch, not committed) that lists each package once
  (`--all-targets -- --list`, plus `--doc -- --list`) and matches the
  filters with a substring search took 0.8 s locally against the current
  script's 23.9 s, with the same result (250 checks, 0 selecting
  nothing). It only approximated `--test` and accepted any shape it didn't
  recognise; P3 closes both.
- The CI step runs last in the job, after the steps that build every test
  target, so the listings need no new build. Its comment cites MB-0019 P4,
  RR-0027 and RR-0032 sub-answer 4.

## Conceptual plan

1. **Clarify (S1).** Only the escalation above: whether the self-test
   joins the `agentic scripts` job (recommended; it needs no Rust build if
   the script can be fed canned listings). If the owner answers it when
   accepting the brief, no consult is needed; otherwise a short CRP
   (platform).
2. **Script.** Split listing from matching: list each package named by any
   check once per target kind it needs, keep each target's tests apart (for
   `--test`), then match each check; fail on an unknown shape or a failed
   listing (P3, P4).
3. **Self-test** `scripts/agentic/test_check_brief_tests.sh`: fixture
   briefs and packs and canned listings covering each shape, each failure in
   P3 and the "not started" skip.
4. **Prove it** on `main` against the old script (P1) and in CI (P2).

Slow-mode triggers: a brief check the new matching treats differently from
cargo; a listing that needs a build the CI job hasn't done.

## Validation plan

- P1: run the old and new scripts on the same commit; the diff is empty
  (the pack records the command and result).
- P2: the step's time in the PR's CI run.
- P3, P4: the self-test, locally and (if approved) in CI.
