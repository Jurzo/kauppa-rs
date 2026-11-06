# Mission Brief — Upgrade actix-web, actix-http and actix-files past the 2026-11 advisories

```yaml
---
id: MB-0017
owner: "@juuso-kalliomaki"
goal: Clear the five RustSEC advisories against the locked actix-web, actix-http and actix-files, so `cargo deny check` passes again and the app no longer ships the known request-smuggling, decompression and hidden-file flaws.
non_goals:
  - Re-enabling `http2`, `compress-zstd` or `ws` (excluded for other advisories; see the comment in `Cargo.toml`).
  - Bumping any other crate beyond what the resolver moves for these three.
  - Using new actix-files 0.7 features (multiple roots, pre-compressed files).
  - Changing the MSRV (`rust-version = "1.89"`), which actix-files 0.7 already meets.
acceptance:
  - id: P1
    property: "`cargo deny check` passes: advisories, bans, licenses and sources, with no entries added to `deny.toml`'s ignore list."
    check: cargo deny check
  - id: P2
    property: The lockfile resolves actix-web ≥ 4.18.2, actix-http ≥ 3.20.3 and actix-files ≥ 0.7.4, and `Cargo.toml` requires the patched actix-files as its floor.
    check: "cargo tree -i actix-web -i actix-http -i actix-files --depth 0"
  - id: P3
    property: The app builds and behaves as before, with no source changes and no new `#[allow]` attributes; the full suite passes.
    check: scripts/agentic/fast-gate.sh
  - id: P4
    property: Static assets are still served by the built binary — `/assets/app.css` is 200 with `text/css`, a missing asset is 404, and a path-traversal request does not escape `assets/`.
    check: "curl smoke against the built binary (recorded in the Merge-Readiness Pack)"
context:
  - Cargo.toml
  - Cargo.lock
  - deny.toml
  - crates/kauppa-web/Cargo.toml
  - crates/kauppa-web/src/main.rs
  - .agentic/runbooks/dependency-bump.md
autonomy:
  may_decide:
    - the order and messages of the lock-update and version-string commits
    - the exact smoke requests for P4
  escalate:
    - { when: "any version or feature change in Cargo.toml", to: platform-owner }
    - { when: the upgrade needs a source change in kauppa-web or kauppa-core, to: platform-owner }
    - { when: "a new advisory, license or duplicate-version ban appears", to: platform-owner }
  forbidden:
    - add or remove dependencies
    - add entries to `deny.toml`'s advisory ignore list without an approved RR
    - lower lint strictness
    - re-enable actix-web features excluded for other advisories
runbook: .agentic/runbooks/dependency-bump.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/actix-web.md@v1
resolutions: []
---
```

## Context

On 2026-11-06, `cargo deny check advisories` started failing on `main`'s
lockfile. The failure appeared during MB-0016 (PR #20); it's unrelated to that
change.

| Advisory | Crate (locked) | Flaw | Fixed in |
| --- | --- | --- | --- |
| RUSTSEC-2026-0446 | actix-http 3.20.0 | Request smuggling through malformed chunked trailers | 3.20.2 |
| RUSTSEC-2026-0448 | actix-http 3.20.0 | Unbounded decompression exhausts memory/CPU | 3.20.1 |
| RUSTSEC-2026-0455 | actix-http 3.20.0 | Large-window Brotli bodies bypass decompression limits | 3.20.3 |
| RUSTSEC-2026-0447 | actix-web 4.18.1 | Compressed bodies allocate before extractor limits apply | 4.18.2 (with actix-http ≥ 3.20.1) |
| RUSTSEC-2026-0443 | actix-files 0.6.10 | Hidden files served through Windows 8.3 short names | 0.7.4 |

`compress-brotli` and `compress-gzip` are enabled (`Cargo.toml`), so the three
decompression advisories apply to every form post.

actix-web and actix-http fixes are semver-compatible patch releases: a
`cargo update -p` moves them, with no `Cargo.toml` change. The actix-files fix
exists only in 0.7.x, and pre-1.0 0.6 → 0.7 is a breaking range. So
`Cargo.toml` must change from `"0.6"`, which makes this a version bump under
`dependency-bump.md`. The only call site is `Files::new("/assets", "./assets")`
in `main.rs`. In 0.7 it takes `impl Into<FilesDirs>`, and a `&str` still
converts.

## Conceptual plan

1. Lock-only commit: `cargo update -p actix-web -p actix-http`.
2. Version-string commit: `actix-files = "0.7.4"` and the resolver's lock
   changes. These are separate commits, per the runbook gate.
3. Gates, deny, smoke, then PR with the approved RR linked.

The lock trial is recorded in CRP-0007 → Evidence.

Checkpoints: CRP-0007 answered → RR approved → (1) → (2) → gates.

Slow-mode triggers:
- any compile error or new warning at a call site;
- any new advisory or license;
- a duplicate-version warning that wasn't there before;
- a test that changes behavior.

## Validation plan

- P1: `cargo deny check` output in the PR, with all four checks `ok`.
- P2: `cargo tree -i … --depth 0` output, showing the three versions.
- P3: fast gate (fmt, clippy `-D warnings`, workspace and per-crate tests) and
  `git diff --stat main -- crates/`, which shows no source changes.
- P4: curl against the built binary on a fresh DB:
  - `GET /assets/app.css` → 200, `text/css`;
  - `GET /assets/nope.css` → 404;
  - `GET /assets/../Cargo.toml` and `GET /assets/%2e%2e/Cargo.toml` → not the
    manifest.
