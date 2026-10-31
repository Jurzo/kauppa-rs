# Mission Brief — kauppa-core builds and tests on its own

```yaml
---
id: MB-0009
owner: "@juuso-kalliomaki"
goal: Make kauppa-core compile and run its tests on its own (`cargo test -p kauppa-core`), so the core check commands in every Mission Brief run as written instead of depending on kauppa-web's dependency tree for crate features.
non_goals:
  - Changing the password KDF — algorithm, parameters, salt, or PHC hash format (`crates/kauppa-core/src/password.rs` is security-tier).
  - Adding, removing, or bumping any crate version; only enabled features may change.
  - Rewriting existing Mission Brief check commands to use `--workspace` as a workaround.
  - Migrating to sqlx compile-time query macros (a separate target state in persistence.md).
acceptance:
  - id: P1
    property: kauppa-core's lib and test targets compile in isolation, with no other workspace crate in the build.
    check: cargo check -p kauppa-core --all-targets
  - id: P2
    property: The kauppa-core check commands from MB-0001 and MB-0002 run as written and select their tests.
    check: "cargo test -p kauppa-core repo::"
  - id: P3
    property: Password hashing is unchanged — a hash produced before this change still verifies (and a wrong password still fails), and new hashes keep the `$argon2id$v=19$m=19456,t=2,p=1$` PHC prefix.
    check: cargo test -p kauppa-core --test password_compat
  - id: P4
    property: The fast gate and CI build and test kauppa-core on its own, so a feature that only arrives through another crate's dependencies fails the gate.
    check: scripts/agentic/fast-gate.sh and the CI `fmt / clippy / test` job log show a passing `cargo test -p kauppa-core --all-targets` step
  - id: P5
    property: The change only enables features — Cargo.lock gains no packages and no versions change; `cargo deny check` stays clean.
    check: cargo deny check, plus `git diff Cargo.lock` adding no `[[package]]` entries
context:
  - crates/kauppa-core/Cargo.toml
  - Cargo.toml
  - crates/kauppa-core/src/password.rs
  - scripts/agentic/fast-gate.sh
  - .github/workflows/ci.yml
  - .agentic/runbooks/dependency-bump.md
  - .agentic/mentorship/security.md
autonomy:
  may_decide:
    - which already-depended-on crate feature closes the gap, within what the approved RR allows
    - whether the per-crate check is a separate CI/fast-gate step or part of the existing test step
    - the fixed hash vector used by the password_compat test
  escalate:
    - { when: "any change to Cargo.toml or crates/*/Cargo.toml — the dependency-bump runbook needs a CRP and an approved RR before the PR", to: platform-owner }
    - { when: the fix appears to require editing password.rs, to: security-owner }
    - { when: CI or gate-script changes beyond adding the per-crate step, to: platform-owner }
  forbidden:
    - change the password KDF algorithm, parameters, or hash format
    - add, remove, or bump a crate
    - lower lint strictness or `-D warnings`
    - make the checks pass by switching brief check commands to `--workspace`
runbook: .agentic/runbooks/dependency-bump.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/security.md@v1
resolutions:
  - RR-0005
---
```

## Context

`cargo test -p kauppa-core …` fails to compile, on `main` too:

```
error[E0432]: unresolved import `argon2::password_hash::rand_core::OsRng`
  --> crates/kauppa-core/src/password.rs:9
```

`OsRng` needs `rand_core`'s `getrandom` feature. kauppa-core depends on
`argon2` with default features (`alloc`, `password-hash`, `rand`), and none of
them enables `getrandom`. The full workspace build only compiles because
Cargo's feature unification picks it up from kauppa-web's tree (actix-web's
cookie crypto → `aes-gcm` → `aead/getrandom` → `rand_core/getrandom`). Any
build that selects only kauppa-core — every `cargo test -p kauppa-core …`
check in MB-0001 and MB-0002 — fails. The fast gate and CI don't notice
because they only build `--workspace`.

`cargo tree -p kauppa-core -e features -i rand_core@0.6.4` shows `rand_core`
reached only through `password-hash` with `alloc`. The feature chains already
present in the lockfile are:

- `argon2/std` → `password-hash/std` → `rand_core/std` → `rand_core/getrandom`
- `password-hash/getrandom` → `rand_core/getrandom`

## Conceptual plan

This is a dependency-feature change, so the dependency-bump runbook applies:
open a CRP and get an RR approved before the PR. The CRP should weigh:

- **A.** `argon2 = { workspace = true, features = ["std"] }` in
  `crates/kauppa-core/Cargo.toml`. Smallest blast radius: it declares what the
  crate actually uses, and kauppa-web is unaffected.
- **B.** The same feature on the workspace `argon2` entry in `Cargo.toml`.
  Equivalent today, but it couples every member to it.
- **C.** A direct `password-hash = { version = "0.5", features = ["getrandom"] }`
  dependency. Explicit, but it adds a direct dependency edge to keep in sync
  with argon2.
- **Do nothing / switch checks to `--workspace`.** Rejected: it leaves every
  brief's core checks broken as written, and that hid vacuous checks in MB-0001.

Then add the per-crate step to `scripts/agentic/fast-gate.sh` and `ci.yml`
(P4), and the hash-compatibility integration test
`crates/kauppa-core/tests/password_compat.rs` (P3). It is an integration test
so `password.rs` itself is not edited.

Checkpoints: P1 passes before touching the gate; P3 is written and passing
**before** the manifest change, so it proves "unchanged" rather than "still
self-consistent". Slow-mode triggers: Cargo.lock gains a package or changes a
version → stop and revisit the option. The fix seems to need `password.rs`
edits → stop and escalate to security.

## Validation plan

- P1, P2: run the commands as written, from a clean `cargo clean -p kauppa-core`
  so nothing is reused from a workspace build. Evidence: command output (P2
  should select the 10 `repo::category::*` / `repo::product::*` tests).
- P3: an integration test with a fixed `$argon2id$…` vector generated by the
  current code before the change. It asserts the vector verifies, a wrong
  password fails, and a fresh hash has the same PHC prefix.
- P4: the fast-gate output and the CI job log on the PR.
- P5: `cargo deny check` output and the `Cargo.lock` diff in the
  Merge-Readiness Pack.
