# Workflow Runbook — Dependency Bump

Version: v1 · Owner: @juuso-kalliomaki (platform).

Any change to `Cargo.toml`, `crates/*/Cargo.toml`, `Cargo.lock` beyond what
`cargo` writes automatically for an in-scope task, or `rust-toolchain.toml` is
a trigger path.

## When to use

- Pulling in a new crate.
- Bumping a crate across a major version.
- Replacing a crate.
- Changing enabled features.

Automatic `Cargo.lock` churn from an in-scope feature change is NOT a bump;
record it in the Merge-Readiness Pack and move on.

## Entry gate

1. Mission Brief exists and names the dependency change.
2. A Consultation Request Pack is open with:
   - Current state (version, features, call sites).
   - Target state and justification.
   - Alternatives considered (including "do nothing").
   - Supply-chain check: crates.io owner, download count, open advisories,
     license.
3. A Resolution Record is approved before the PR is opened.
4. The brief's Continuity Pack exists and is kept current
   (`AGENTS.md` → Continuity Packs; feature runbook entry gate, item 4).

## Gates

- `cargo deny check` passes with no new advisories or unlisted licenses.
- `cargo update -p <crate>` is a separate commit from the version-string
  change.
- The call sites compile with no new `#[allow]` attributes.

## Forbidden

- Adding a dependency with wildcard versions.
- Adding a crate from a non-crates.io source without an approved RR.
- Lowering lint strictness or `-D warnings` to accommodate a bump.
