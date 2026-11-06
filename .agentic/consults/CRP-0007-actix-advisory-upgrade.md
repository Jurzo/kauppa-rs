# Consultation Request Pack — Upgrade actix-web, actix-http and actix-files for the 2026-11 advisories

```yaml
---
id: CRP-0007
mission: MB-0017
owner: "@juuso-kalliomaki"
target_role: platform
risk_tier: medium
urgency: high
state: open
runbook_trigger: ".agentic/runbooks/dependency-bump.md (\"Bumping a crate across a major version\": actix-files 0.6 → 0.7; entry gate 2–3: CRP open, RR approved before the PR); CI trigger path `Cargo.toml`; MB-0017 autonomy.escalate (any version change in Cargo.toml)"
---
```

## Decision statement

Approve moving actix-web to 4.18.2, actix-http to 3.20.3 (both lock-only) and
actix-files to 0.7.4 (a `Cargo.toml` change, 0.6 → 0.7), to clear five
RustSEC advisories (MB-0017 → Context).

## Current state

- `Cargo.toml` (workspace):
  - `actix-web = { version = "4", default-features = false, features = [macros, cookies, unicode, compat, compress-brotli, compress-gzip] }`;
  - `actix-files = "0.6"`.
- Locked versions: actix-web 4.18.1, actix-http 3.20.0 (transitive), actix-files 0.6.10.
- Call sites:
  - actix-web and actix-http: all of `kauppa-web`;
  - actix-files: one, `Files::new("/assets", "./assets")` in `crates/kauppa-web/src/main.rs`.
- `cargo deny check advisories` fails with RUSTSEC-2026-0443, -0446, -0447, -0448 and -0455.

## Options and trade-offs

### Option A — patch actix-web/http, bump actix-files to 0.7.4 (recommended)

- **Changes:**
  - `cargo update -p actix-web -p actix-http`;
  - `actix-files = "0.7.4"`, where the floor is the patched release.
- **Pros:**
  - clears all five advisories with no ignore entries;
  - no source changes;
  - actix-files 0.7 also carries fixes made since 0.6.10 (Range and precondition handling, a `use_hidden_files` panic, and the stored XSS in `show_files_listing`, a feature we don't enable).
- **Cons:**
  - a pre-1.0 minor bump. 0.7.0 removed `experimental-io-uring` and `NamedFile::open_async`, which we don't use. `Files::new` became generic over `Into<FilesDirs>`, and `&str` still converts.
- **Transitive changes:**
  - `v_htmlescape` 0.15.8 → 0.17.0;
  - `v_escape-base` 0.1.0 is new (both from actix-files);
  - `getrandom` 0.3.4, `r-efi` 5.3.0, `wasip2` and `wit-bindgen` drop out.
- **Risk:** low to medium. The static-asset route is the only behavior that changes, and P4 smoke-tests it.

### Option B — patch actix-web/http only; ignore RUSTSEC-2026-0443

- **Changes:** lock-only for the four actix-web and actix-http advisories, plus a `deny.toml` ignore entry for the actix-files advisory. The rationale for the ignore: the app is deployed on Linux, where 8.3 short names don't exist, and `assets/` holds no hidden files.
- **Pros:** no `Cargo.toml` version change.
- **Cons:**
  - the ignore list is empty today, and this would start it;
  - the ignore must be revisited;
  - it stays on 0.6.x, which gets no further fixes;
  - self-hosters on Windows would be exposed.

### Option C — do nothing, or mitigate at the reverse proxy

- **Pros:** no change.
- **Cons:**
  - CI's `cargo deny` stays red on every PR;
  - the smuggling and decompression flaws are reachable by any client that can reach the app directly;
  - "one binary, one `.env`" self-hosting (AGENTS.md goal 3) can't assume a proxy.

## Recommendation and question

Recommended: **Option A.**

Crisp question to answer: **Approve updating actix-web to 4.18.2 and
actix-http to 3.20.3 in the lockfile, and actix-files to `"0.7.4"` in
`Cargo.toml`, as two separate commits?**

## Supply-chain check

From crates.io on 2026-11-06:

| Crate | Target | Owners | Downloads (all / recent) | License | Open advisories at target |
| --- | --- | --- | --- | --- | --- |
| actix-web | 4.18.2 | fafhrd91, github:actix:core | 89.5M / 13.3M | MIT OR Apache-2.0 | none |
| actix-http | 3.20.3 | fafhrd91, github:actix:core | 89.3M / 13.4M | MIT OR Apache-2.0 | none |
| actix-files | 0.7.4 | fafhrd91, github:actix:core | 13.0M / 1.5M | MIT OR Apache-2.0 | none |

- Same owners as the versions already locked.
- All three come from crates.io, the same repository (`actix/actix-web`) as today.
- No new licenses: `cargo deny check licenses` passes in the trial.

## Blast radius and rollback

- **What can break:**
  - HTTP parsing and decompression for every request (patch releases);
  - static asset serving at `/assets`.
- **Who is affected:** every user; no data or schema is touched.
- **Rollback plan:** revert the PR (two commits). The lockfile restores the previous versions exactly.

## Evidence bundle

- **Trial** (2026-11-06): in a scratch worktree off `main` at c2c9c86, applied Option A, ran the checks below, then reverted.
  - `cargo deny check` → `advisories ok, bans ok, licenses ok, sources ok`.
  - `scripts/agentic/fast-gate.sh` → OK: core 46, password_compat 3 and web 23 tests passed; clippy clean. No source changes.
- **Dry run:** `cargo update --dry-run -p actix-web -p actix-http` → `actix-http v3.20.0 -> v3.20.3`, `actix-web v4.18.1 -> v4.18.2`, nothing else.
- **Advisories:** https://rustsec.org/advisories/RUSTSEC-2026-0443, -0446, -0447, -0448, -0455.
- **Changelog:** actix-files `CHANGES.md` 0.7.0–0.7.4.
