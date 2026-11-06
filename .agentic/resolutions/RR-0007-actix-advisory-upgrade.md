# Resolution Record — Upgrade actix-web, actix-http and actix-files for the 2026-11 advisories

```yaml
---
id: RR-0007
closes: CRP-0007
mission: MB-0017
decided_by: "@juuso.kalliomaki"
role: platform
risk_tier: medium
decided_at: 2026-11-06T21:16:42Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0017-actix-advisories.md
  - .agentic/consults/CRP-0007-actix-advisory-upgrade.md
---
```

## Decision and outcome

Approve CRP-0007 **Option A**:

- actix-web 4.18.1 → 4.18.2 and actix-http 3.20.0 → 3.20.3, by
  `cargo update -p actix-web -p actix-http` (lockfile only);
- actix-files `"0.6"` → `"0.7.4"` in the workspace `Cargo.toml`, with the
  resolver's lockfile changes.

Covers:

- those two commits, kept separate per `dependency-bump.md`;
- the transitive lockfile changes that come with actix-files 0.7.4, as listed
  in CRP-0007:
  - `v_htmlescape` 0.15.8 → 0.17.0;
  - `v_escape-base` 0.1.0 added;
  - `getrandom` 0.3.4, `r-efi` 5.3.0, `wasip2` and `wit-bindgen` removed.

Does not cover:

- any other crate update;
- feature changes (`http2`, `compress-zstd` and `ws` stay excluded);
- using new actix-files 0.7 APIs;
- source changes. If the upgrade needs one, the agent stops and re-consults
  (MB-0017 escalation).

## Rationale and trade-offs

- **A — chosen.**
  - Clears all five advisories (RUSTSEC-2026-0443, -0446, -0447, -0448,
    -0455) without starting an ignore list.
  - Needs no source changes, as the trial showed.
  - Moves actix-files onto the maintained 0.7 line.
- **B — rejected.** Ignoring RUSTSEC-2026-0443 would rest on a deployment
  assumption (Linux only). Self-hosters may run Windows. It would also leave
  the crate on a line that gets no further fixes.
- **C — rejected.** The request-smuggling and decompression flaws are
  reachable by any client. A reverse proxy can't be assumed (AGENTS.md goal
  3), and CI would stay red.

## Approvals and authority

Approved by platform-owner @juuso.kalliomaki

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0007-actix-advisory-upgrade.md`
- Mission Brief: `.agentic/missions/MB-0017-actix-advisories.md`
- Evidence links:
  - CRP-0007 → Evidence bundle and Supply-chain check: the trial on c2c9c86,
    `cargo deny check` all ok, fast gate OK, crates.io owners and licenses;
  - https://rustsec.org/advisories/RUSTSEC-2026-0443 (and -0446, -0447,
    -0448, -0455).

## Constraints and follow-on rules (Hardened)

1. **Patched floor.** `actix-files` stays at `"0.7.4"` or higher in
   `Cargo.toml`, so a fresh lockfile can't resolve a vulnerable 0.7.x.
2. **No advisory ignores.** `deny.toml`'s `[advisories] ignore` stays empty
   unless a later RR approves an entry.
3. **Excluded features stay excluded.** `http2`, `compress-zstd` and `ws`
   remain off, for the reasons in the `Cargo.toml` comment.
4. **Static assets keep their guarantees.** `/assets` serves only files
   under `assets/`, with no hidden files and no directory listing (MB-0017
   P4).

## Roll-back into the system (Hardened)

To be applied in MB-0017's PR:

- `MB-0017`: `resolutions: [RR-0007]`.
- `CRP-0007`: `state: open` → `answered`.

No Mentorship Pack or Runbook change.

## Supersession and history

Status: active. Supersedes nothing.
