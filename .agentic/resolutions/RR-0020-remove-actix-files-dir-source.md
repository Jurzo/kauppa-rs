# Resolution Record — Remove `actix-files`, handlebars' `dir_source`, and the runtime image's file copies

```yaml
---
id: RR-0020
closes: CRP-0020
mission: MB-0032
decided_by: "@juuso.kalliomaki"
role: platform
risk_tier: low
decided_at: 2026-11-10T08:20:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0032-remove-file-serving-leftovers.md
  - .agentic/consults/CRP-0020-remove-actix-files-dir-source.md
---
```

## Decision and outcome

Approve CRP-0020 **Option A**. Remove `actix-files` and handlebars'
`dir_source` feature, rewrite the two tests that used them, and stop
copying `templates/`, `assets/` and `migrations/` into the runtime image.
This is the follow-up that RR-0011 sub-answer 2 deferred.

Sub-answers:

1. **Removal (Q1, platform).** The change is two commits, as the
   dependency-bump runbook requires.
   - **Commit 1, manifests:**
     - drop `actix-files = "0.7.4"` from the workspace `Cargo.toml`;
     - drop `actix-files = { workspace = true }` from
       `crates/kauppa-web/Cargo.toml`;
     - change `handlebars = { version = "6", features = ["dir_source"] }`
       to `handlebars = "6"`.
   - **Commit 2, `cargo update -w`:** the lockfile only removes exactly
     these 9 packages, and nothing is added or moves version:

     | Package | Version |
     | --- | --- |
     | actix-files | 0.7.4 |
     | http-range | 0.1.6 |
     | mime_guess | 2.0.5 |
     | unicase | 2.10.0 |
     | v_htmlescape | 0.17.0 |
     | v_escape-base | 0.1.0 |
     | walkdir | 2.5.0 |
     | same-file | 1.0.6 |
     | winapi-util | 0.1.11 |

   - Any other lockfile change stops the PR (slow mode).
   - `cargo deny check` and `cargo tree -i actix-files` ("did not match")
     go in the Merge-Readiness Pack.
2. **Tests (Q2, platform).** Both rewrites are accepted.
   - `serves_every_asset` (P2) keeps only the pinned `SERVED_AS` contract
     and drops the reference `Files` app. It still checks that:
     - the CWD has no `./assets`;
     - the embedded list equals the files on disk;
     - `GET` and `HEAD` answer `200` with the pinned type;
     - the bytes equal the disk copy;
     - a missing row panics.
   - `registry_is_embedded` (P3) compares the embedded names with a
     test-local `std::fs::read_dir` walk. The walk applies the directory
     source's rules: `.hbs` files only, a `/`-separated stem, and hidden
     `.x` and temporary `#x` files skipped. Strict mode and the
     byte-for-byte comparison stay.
   - Comments that name `actix-files` in `build.rs`, `routes/assets.rs`,
     `templates.rs` and the tests may be reworded (MB-0032 may_decide).
3. **Image (Q3, owner).** `deploy/Dockerfile`'s runtime stage drops
   `COPY templates ./templates`, `COPY assets ./assets` and
   `COPY migrations ./migrations`, and the stale "Templates load from
   ./templates …" comment.
   - It keeps `WORKDIR /app` as the empty working directory.
   - The builder stage's copies stay, and so does `.dockerignore`.
   - `scripts/deploy/smoke_image.sh` changes to match:
     - `contents_ok` asserts that `/app/templates`, `/app/assets` and
       `/app/migrations` are absent;
     - `app_files_read_only` checks only the binary;
     - the header comment and the check title are updated.
4. **MB-0022 P1 (Q4, owner).** The closed brief MB-0022 is left as
   history: its P1 wording is not rewritten. MB-0032 P4 is the current
   contract for the image contents.
5. **`deploy/README.md` (Q5, owner).** In the same PR, the "After MB-0024
   (embedded templates and assets)" note is replaced with a short line
   saying the runtime image holds only the binary (MB-0032, RR-0020).

Covers:

- MB-0032's implementation PR, with P1–P4 as written;
- the two manifest edits and the 9-package lockfile removal;
- the two test rewrites and the comment wording;
- the `deploy/Dockerfile`, `smoke_image.sh` and `deploy/README.md` changes
  above.

Does not cover:

- adding, bumping or changing the features of any other dependency;
- changing `deny.toml`, including its stale "handlebars/walkdir pull
  windows-sys 0.48" `skip` comment;
- changing the content types in `build.rs` or `SERVED_AS`, or what
  `/assets` serves;
- `main.rs` or the `/assets` service's behaviour;
- caching headers for `/assets`;
- rewriting MB-0022's P1 in place;
- removing `WORKDIR /app` or any other runtime-stage change.

## Rationale and trade-offs

- **A, chosen.**
  - Both crates only served checks that compared the embedded copy with
    what it replaced. Since MB-0024, the pinned tables are the contract
    (RR-0011 constraint 4).
  - The removal is pure: 9 fewer packages, nothing added or moved.
  - The release binary shrinks by 3,104 B, and clippy, all 96
    `kauppa-web` tests and `cargo deny` stay green (CRP-0020 trial).
  - The image stops carrying 144 KB of files the binary never reads.
  - **B (crates only), rejected.** It leaves dead files in every image and
    a smoke check that requires them, and it misses MB-0032 P4.
  - **C (nothing), rejected.** It keeps `actix-files` (five advisories
    cleared in 2026-11, CRP-0007) and `walkdir` for tests only, and leaves
    RR-0011's follow-up open.
- **Losing the differential check (Q2).** It was only meaningful while
  `actix-files` existed. The pinned content types already equal what
  `actix-files` sent.
- **Keeping `WORKDIR /app` (Q3).** It is the smallest change, and the
  process keeps a predictable, root-owned, empty CWD instead of `/`.
- **MB-0022 as history (Q4).** P1 described the image MB-0022 shipped.
  MB-0032 P4 states the new contents, and `smoke_image.sh` enforces them.
  RR-0009 constraint 2 is about check paths, and this check's path does not
  change.

## Approvals and authority

Approved by platform-owner @juuso.kalliomaki (Q1, Q2, and the removal under the
dependency-bump runbook). Approved by owner @juuso.kalliomaki (Q3–Q5: image
contents, MB-0022 wording and `deploy/README.md`, under MB-0022's
escalation).

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0020-remove-actix-files-dir-source.md`
- Mission Brief: `.agentic/missions/MB-0032-remove-file-serving-leftovers.md`
- Continuity Pack: `.agentic/continuity/MB-0032.md`
- Branch: `agentic/mb-0032-file-serving-cleanup`
- Runbook: `.agentic/runbooks/dependency-bump.md` (entry gates 2–3; this
  RR is the approval)
- Related decisions:
  - RR-0011: sub-answer 2 (the deferral) and constraints 4–5;
  - RR-0007: `actix-files` 0.7.4 floor, constraints 1 and 4;
  - RR-0009 constraint 2.
- Evidence links: CRP-0020 → Supply-chain check and Evidence bundle, from
  the trial on `50322c0` (2026-11-09):
  - `cargo update -w` only removes the 9 packages;
  - `cargo tree -i actix-files` reports no match;
  - clippy is clean;
  - `kauppa-web` passes 96/96;
  - `cargo deny check` is ok;
  - the release binary goes from 9,327,064 B to 9,323,960 B;
  - the image payload sizes are measured.

## Constraints and follow-on rules (Hardened)

1. **No filesystem source in production or tests.** Templates and assets
   come only from the embedded copies. Re-adding `actix-files`,
   handlebars' `dir_source`, or any runtime read of `./templates`,
   `./assets` or `./migrations` needs an RR.
2. **The pinned tables are the contract.** `SERVED_AS` and `build.rs`'s
   `CONTENT_TYPES` stay the only source of content types (RR-0011
   constraint 4, unchanged). There is no differential reference
   implementation.
3. **The runtime image holds the binary.** Beyond the binary, the entrypoint
   and the healthcheck script, the runtime stage copies no project
   directories. `smoke_image.sh` asserts that `templates/`, `assets/` and
   `migrations/` are absent. Adding files to the image is an owner
   decision.
4. **The lockfile change is removal only.** The PR's `Cargo.lock` diff is
   exactly the 9 removals above.

## Roll-back into the system (Hardened)

To be applied in MB-0032's implementation PR:

- `MB-0032`: `resolutions: []` → `resolutions: [RR-0020]`; Conceptual plan
  step 1: append "Answered by RR-0020."
- `CRP-0020`: `state: open` → `answered`.
- `.agentic/continuity/MB-0032.md`: record the RR under Decisions and
  constraints, and close the follow-up from MB-0024's pack (RR-0011
  sub-answer 2).

No Mentorship Pack or Runbook change. MB-0022 is not edited (sub-answer 4).

## Supersession and history

Status: active once approved. This RR **supersedes RR-0011 constraint 5
and sub-answer 2's deferral**: `actix-files` and `dir_source` leave
through this change. RR-0011 otherwise stays active, including the `/assets`
registration, rule 6, exact names only and the pinned content types.
RR-0007 constraints 1 and 4 (the `actix-files` version floor and its
static-asset guarantees) no longer have a subject. The embedded `Assets`
service keeps the guarantees in constraint 4 under RR-0011.
RR-0007 constraint 1 ("actix-files stays at 0.7.4 or higher") becomes moot
with the crate gone; mark RR-0007 marked as partly superseded.
