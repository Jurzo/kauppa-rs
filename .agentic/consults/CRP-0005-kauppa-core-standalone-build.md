# Consultation Request Pack — kauppa-core standalone build: enable argon2 `std`

```yaml
---
id: CRP-0005
mission: MB-0009
owner: "@juuso-kalliomaki"
target_role: platform
risk_tier: low
urgency: medium
state: answered
runbook_trigger: ".agentic/runbooks/dependency-bump.md (\"Changing enabled features\"; entry gate 2–3: CRP open, RR approved before the PR); feature.md escalation trigger `Cargo.toml`, `crates/*/Cargo.toml` dependency changes → platform; MB-0009 autonomy.escalate (any change to Cargo.toml or crates/*/Cargo.toml)"
---
```

## Decision statement

Choose which already-present crate feature makes kauppa-core compile on its
own, so `cargo test -p kauppa-core …` runs as the Mission Briefs write it, and
approve adding a per-crate kauppa-core step to the fast gate and CI.

## Options and trade-offs

All three options were tried locally and reverted (see Evidence bundle). Each
one makes P1, P2, and P3 pass. None adds a package or changes a version.

### Option A — `argon2 = { workspace = true, features = ["std"] }` in `crates/kauppa-core/Cargo.toml`

- Pros: the crate declares what it actually uses (`OsRng` via
  `argon2/std` → `password-hash/std` → `rand_core/std` → `rand_core/getrandom`).
  No change to `Cargo.lock`. Only kauppa-core's manifest changes.
- Cons: relies on a transitive chain through `std` instead of naming
  `getrandom` directly. A future argon2 release could restructure its
  features, but that would be a version bump, which goes through this runbook
  anyway.
- Risk: low. Across the workspace, the resolved feature sets change only by
  adding `std` to `argon2`, `password-hash`, and `base64ct`. `rand_core`
  already had `getrandom,std` through kauppa-web. The KDF algorithm,
  parameters, and PHC format don't change, and P3 checks that.
- Cost / timeline: a one-line manifest change plus the gate step, in one PR.

### Option B — the same feature on the workspace `argon2` entry in `Cargo.toml`

- Pros: one place for the feature. The resolved feature sets are the same as
  A, and `Cargo.lock` doesn't change.
- Cons: only kauppa-core depends on argon2 today, so it's equivalent for now.
  But every future member that adds `argon2 = { workspace = true }` inherits
  `std`, and the feature sits away from the crate that needs it.
- Risk: low; same as A.
- Cost / timeline: same as A.

### Option C — a direct `password-hash = { version = "0.5", features = ["getrandom"] }` in kauppa-core

- Pros: names the exact feature (`getrandom`) that `OsRng` needs.
- Cons: adds a direct dependency edge (`Cargo.lock` gains one dependency
  line, no new package) that must stay on the same minor as argon2's
  `password-hash`, or two versions get built. Arguably "adding a dependency",
  which MB-0009 forbids (`add, remove, or bump a crate`).
- Risk: low at runtime; medium for maintenance (version drift).
- Cost / timeline: same as A.

### Do nothing / switch the briefs' checks to `--workspace`

Rejected in MB-0009 (`forbidden`). Every brief's core checks stay broken as
written, and building only `--workspace` is what hid MB-0001's vacuous checks.

## Recommendation and question

Recommended option: **A**.
Because: it has the smallest blast radius (one crate manifest, no lockfile
change), it declares the dependency's needs where the code uses it, and it
stays within MB-0009's "features only" non-goal. B is an acceptable
alternative if you'd rather keep all features on the workspace entries.

Also part of this decision (MB-0009 P4, within the brief's `may_decide`): add a
`cargo test -p kauppa-core --all-targets` step to `scripts/agentic/fast-gate.sh`
and to the `fmt / clippy / test` job in `.github/workflows/ci.yml`, after the
workspace test step, so a feature that arrives only through another crate
fails the gate.

Crisp question to answer: **Approve Option A (or B) together with the
per-crate gate step?**

## Blast radius and rollback

- **What can break:** nothing at runtime. The workspace binary already built
  with `rand_core/getrandom,std`; the only new resolved features are `std` on
  `argon2`, `password-hash`, and `base64ct` (mostly `std::error::Error` impls
  and std-backed helpers). The gate gets one more test step (~1 s on a warm
  cache).
- **Who is affected:** anyone building or testing kauppa-core alone; CI time.
- **Rollback plan:** revert the PR. No data, schema, or hash-format change, so
  there's nothing to migrate back.

## Minimal context

- MB-0009 → Context and Conceptual plan (options A/B/C come from there).
- `crates/kauppa-core/src/password.rs:9` imports
  `argon2::password_hash::rand_core::OsRng`, which needs
  `rand_core/getrandom`. `password.rs` is not edited by any option.
- kauppa-core depends on `argon2 = { workspace = true }` (defaults: `alloc`,
  `password-hash`, `rand`). kauppa-web does not depend on argon2 directly; it
  gets `rand_core/getrandom` via actix-web cookie crypto → `aes-gcm` →
  `aead/getrandom`.
- Locked versions: `argon2 0.5.3`, `password-hash 0.5.0`, `rand_core 0.6.4`,
  `getrandom 0.2.17`. All already in `Cargo.lock`.

## Evidence bundle (Hardened)

Run on branch `agentic/crp-0005-kauppa-core-standalone-build` at 3403932, on
2026-10-31.

- **Baseline (no option), after `cargo clean -p kauppa-core`:**
  `cargo check -p kauppa-core --all-targets` → exit 101,
  `error[E0432]: unresolved import argon2::password_hash::rand_core::OsRng`.
- **P3 test written first, against the unchanged manifests:**
  `crates/kauppa-core/tests/password_compat.rs` with a fixed vector generated by
  the current `password::hash`. `cargo test --workspace --test password_compat`
  → 3 passed.
- **Each option applied, then reverted:**

  | Check | A | B | C |
  | --- | --- | --- | --- |
  | `cargo check -p kauppa-core --all-targets` | ok | ok | ok |
  | `cargo test -p kauppa-core repo::` | 10 passed | 10 passed | 10 passed |
  | `cargo test -p kauppa-core --test password_compat` | 3 passed | 3 passed | 3 passed |
  | `cargo test -p kauppa-core --all-targets` | 13 + 3 passed | 13 + 3 passed | 13 + 3 passed |
  | `git diff Cargo.lock` | none | none | +1 dependency line, 0 packages |

- **Option A, resolved features across the workspace**
  (`cargo tree --workspace --all-targets -f '{p} [{f}]' --prefix none`, before
  vs after): exactly three lines differ —

  ```
  argon2 v0.5.3        [alloc,default,password-hash,rand] → [… ,std]
  base64ct v1.8.3      [alloc]                            → [alloc,std]
  password-hash v0.5.0 [alloc,default,rand_core]          → [… ,std]
  ```

- **Option A, `cargo deny check`:** `advisories ok, bans ok, licenses ok, sources ok`.
- **Supply chain:** no new crate. `argon2` and `password-hash` (RustCrypto,
  MIT OR Apache-2.0) are already in the lockfile and pass `cargo deny`.
