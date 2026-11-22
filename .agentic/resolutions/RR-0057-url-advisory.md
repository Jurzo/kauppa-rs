# Resolution Record — RUSTSEC-2026-0592: `url` 2.5.9 in `Cargo.lock` (MB-0072)

```yaml
---
id: RR-0057
closes: CRP-0057
mission: MB-0072
decided_by: "@Jurzo"
role: platform
risk_tier: low
decided_at: 2026-11-22t10:10:00z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0072-url-advisory.md
  - .agentic/consults/CRP-0057-url-advisory.md
---
```

## Decision and outcome

Approve CRP-0057 option A (owner, 2026-11-22: "CRP-0057 option A
approved"): update `url` 2.5.8 → 2.5.9 and `form_urlencoded` 1.2.2 →
1.2.3 in `Cargo.lock`, as `cargo update -p url` writes them, so
RUSTSEC-2026-0592 no longer applies and `cargo deny check` passes without
an ignore.

Covers:

- MB-0072's pull request: one commit changing `Cargo.lock` only (two
  versions, two checksums);
- the roll-back below.

Does not cover:

- any other crate's update, or any `Cargo.toml`, `deny.toml` or
  `rust-toolchain.toml` change;
- an ignore for this or any other advisory;
- a later `url` update, which follows RR-0032 (Dependabot) or this runbook
  again.

## Rationale and trade-offs

- A released fix exists. It is a patch update of a transitive dependency,
  from the same maintainers and repository (servo/rust-url), with the same
  licence (MIT OR Apache-2.0).
- Nothing here calls `Url::origin()` or `url::quirks::origin()`: not our
  crates, and not the six dependents that pull `url` in. So the update
  changes no behaviour the app uses; it clears the advisory `deny.toml`
  fails on.
- Tried on `main` `f3fef8d`:
  - `cargo deny check` passes;
  - clippy with `-D warnings` is clean, apart from kauppa-core's two known
    unused-dependency warnings;
  - `cargo test --workspace --all-targets` passes: 108 + 3 + 242.
- Rejected:
  - waiting for Dependabot (B): every PR's `cargo deny` fails until then;
  - an interim ignore (C): a lint-config change to hide an advisory a
    released fix resolves;
  - doing nothing (D).

## Approvals and authority

@Jurzo as platform-owner.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0057-url-advisory.md` (supply-chain
  check, exposure search, the local run)
- Mission Brief: `.agentic/missions/MB-0072-url-advisory.md`
- Continuity Pack: `.agentic/continuity/MB-0072.md`
- Evidence: CI run 45009661837 (PR #146, `cargo deny`);
  https://rustsec.org/advisories/RUSTSEC-2026-0592
- Related: RR-0032 (standing approval for Dependabot's lockfile-only
  updates); RR-0039 (the SQLite advisory, MB-0041).

## Constraints and follow-on rules (Hardened)

1. The PR changes `Cargo.lock` only, in one commit made by `cargo update -p
   url` (dependency-bump runbook, gates).
2. `cargo deny check` passes in the PR's CI, with no new `ignore` entry.
3. Every existing test passes unchanged, and the image's smoke tests pass.

## Roll-back into the system (Hardened)

To be applied after this record is signed, with the owner's OK:

- `MB-0072`: `resolutions: [RR-0057]`; the fix-path escalation gets
  "(RR-0057)"; plan step 1 marked done.
- `CRP-0057`: `state: open` → `answered`.

## Supersession and history

Status: active once signed. Supersedes nothing.

- 2026-11-22: roll-back applied to MB-0072 and CRP-0057 (PR #147).
