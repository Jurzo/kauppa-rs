# Resolution Record — kauppa-core standalone build: enable argon2 `std`

```yaml
---
id: RR-0005
closes: CRP-0005
mission: MB-0009
decided_by: "@juuso.kalliomaki"
role: platform
risk_tier: low
decided_at: 2026-10-31T09:35:42Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0009-kauppa-core-standalone-build.md
  - .agentic/consults/CRP-0005-kauppa-core-standalone-build.md
---
```

## Decision and outcome

Approve CRP-0005 **Option A**: enable the `std` feature of `argon2` in
`crates/kauppa-core/Cargo.toml`
(`argon2 = { workspace = true, features = ["std"] }`), and add a
`cargo test -p kauppa-core --all-targets` step to
`scripts/agentic/fast-gate.sh` and to the `fmt / clippy / test` job in
`.github/workflows/ci.yml`.

Covers: that one manifest line and the two gate steps (MB-0009 P1–P5).

Does not cover: any crate addition, removal, or version bump; any change to
the workspace `argon2` entry in `Cargo.toml`; any edit to
`crates/kauppa-core/src/password.rs`; any other CI or gate-script change.

## Rationale and trade-offs

- **A — chosen.** The crate declares what it uses (`OsRng` via
  `argon2/std` → `password-hash/std` → `rand_core/getrandom`). One manifest
  changes, `Cargo.lock` doesn't, and across the workspace the resolved
  features only gain `std` on `argon2`, `password-hash`, and `base64ct`.
- **B — rejected.** The feature on the workspace `argon2` entry is equivalent
  today, but every future member using argon2 would inherit it, away from the
  crate that needs it.
- **C — rejected.** A direct `password-hash` dependency names `getrandom`
  exactly, but adds a dependency edge that must track argon2's minor version,
  and MB-0009 forbids adding a crate.
- **Do nothing / `--workspace` checks — rejected** (MB-0009 `forbidden`).

## Approvals and authority

Approved by platform-owner @juuso.kalliomaki

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0005-kauppa-core-standalone-build.md`
- Mission Brief: `.agentic/missions/MB-0009-kauppa-core-standalone-build.md`
- Evidence links: CRP-0005 → Evidence bundle (per-option results, resolved
  feature diff, `cargo deny check`); P3 test
  `crates/kauppa-core/tests/password_compat.rs`, written and passing before
  the manifest change.

## Constraints and follow-on rules (Hardened)

1. **Features only.** The PR changes `Cargo.lock` not at all: no
   `[[package]]` added and no version changed (MB-0009 P5). If it does, stop
   and reopen the consult.
2. **Hash format guarded.** `password_compat` (P3) passes before and after the
   manifest change, with the same fixed vector. `password.rs` is not edited.
3. **Standalone build stays gated.** The per-crate step stays in the fast gate
   and CI, so a feature that only arrives through another crate's dependency
   tree fails the gate. Removing it needs a new consult.
4. **Crates declare their own features.** A crate that uses an API behind a
   dependency feature enables that feature in its own manifest instead of
   relying on feature unification.

<!-- Resolver: confirm constraint 4, or drop it if it is broader than you want. -->

## Roll-back into the system (Hardened)

- `MB-0009`: `resolutions: []` → `resolutions: [RR-0005]`.
- `CRP-0005`: `state: open` → `answered`.
- For the resolver to decide: whether `.agentic/runbooks/feature.md` →
  "Deterministic checks" should list the per-crate test step next to
  "Unit tests", and whether constraint 4 belongs in a Mentorship Pack. Either
  is a governance edit, so it needs your go-ahead; it isn't part of the
  MB-0009 PR unless you say so.

## Supersession and history

Status: active. Supersedes nothing.
