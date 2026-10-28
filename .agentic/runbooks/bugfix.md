# Workflow Runbook — Bugfix

Version: v1 · Owner: @juuso-kalliomaki.

Bugfixes follow the feature runbook with these overrides:

1. **Reproduce first.** The Mission Brief must contain either a failing test
   or a precise repro recipe (URL, headers, body, expected vs actual). Without
   this the entry gate fails.
2. **Red → green.** Add the failing test *before* the fix. The PR must show it
   failing in a commit (or the Merge-Readiness Pack must link to a prior CI
   run that failed on that test).
3. **Scope.** Do not refactor unrelated code. If you see adjacent debt, file a
   separate Mission Brief rather than widening this one.
4. **Blast radius.** The Merge-Readiness Pack must describe which other call
   sites or users might be affected, and why they are not broken.
