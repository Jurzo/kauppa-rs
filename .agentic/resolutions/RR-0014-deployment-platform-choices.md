# Resolution Record — Deployment platform choices for the first deploy

```yaml
---
id: RR-0014
closes: CRP-0014
mission: MB-0022
decided_by: "@juuso.kalliomaki"
role: platform
risk_tier: high
decided_at: 2026-11-08T18:23:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/mentorship/security.md
  - .agentic/missions/MB-0022-first-deployment.md
  - .agentic/consults/CRP-0014-deployment-platform-choices.md
---
```

## Decision and outcome

Approve CRP-0014's **recommended set, Q1–Q7**: GitHub Actions builds the
image and pushes it to public GHCR; Terraform state lives in a Spaces bucket
with `use_lockfile`; the session secret is generated on the droplet and no
secret passes through Terraform; a scheduled `VACUUM INTO` job ships
6-hourly snapshots to Spaces; DNS stays outside Terraform; a `fra1`
`s-1vcpu-1gb` droplet with Spaces in `ams3`; one new CI workflow in its own
PR.

Sub-answers:

1. **Image delivery (Q1, A).**
   - A workflow on push to `main` (and on tags) builds the multi-stage image
     on `ubuntu-latest` (amd64) and pushes
     `ghcr.io/jurzo/kauppa-rs:sha-<commit>` plus a moving `:main` tag. The
     package is public; the droplet pulls anonymously and holds no registry
     credential.
   - `compose.yaml` on the droplet pins one `sha-<commit>` tag or a digest,
     never `:main`. Updating is: pre-update backup (sub-answer 4) → change
     the pin → `docker compose pull && docker compose up -d`. Rolling back is
     pinning the previous tag, plus restoring the pre-update backup if the
     update ran a migration (MB-0022 P7).
   - **Fallback until the workflow PR merges:** the owner builds locally
     (`docker buildx build --platform linux/amd64`) and pushes to GHCR with
     a personal token scoped to `write:packages`, held on the owner's
     machine only. `deploy/README.md` documents this one path; the
     `docker save | ssh … docker load` variant is not documented.
2. **Terraform state (Q2, B).**
   - `backend "s3"` against Spaces with DO's documented `skip_*` flags and
     `use_lockfile = true` (Terraform ≥ 1.10; `required_version` pins
     `>= 1.10`). No DynamoDB.
   - The state bucket is created by hand in the DO console before
     `terraform init` (one README step), in `ams3`, with **versioning on**.
     It is separate from the backup bucket.
   - Credentials come only from `AWS_ACCESS_KEY_ID` /
     `AWS_SECRET_ACCESS_KEY` in the operator's shell: a limited-access
     Spaces key scoped to the state bucket. Never `-backend-config`, so they
     don't land in `.terraform/` or plan files.
3. **Secrets (Q3, A).**
   - **Session secret.** cloud-init (which carries no secret) writes
     `/mnt/<volume>/secrets/app.env`, mode 0600, root-owned, **only if the
     file doesn't exist**, with
     `openssl rand -base64 64 | tr -d '\n' | cut -c1-64`. Compose reads it
     via `env_file`. A replaced droplet keeps it, because it sits on the
     volume. Rotation: regenerate the file, then `docker compose up -d`
     (logs everyone out, loses nothing else).
   - **Backup Spaces key.** A limited-access key, created in the DO
     console and scoped to the backup bucket, `scp`'d by the owner as
     `secrets/backup.env` (0600) next to `app.env`.
   - **Operator-only credentials** (`DIGITALOCEAN_TOKEN`, the state key in
     `AWS_*`, the Spaces key in `SPACES_*` that Terraform uses to manage the
     backup bucket) live in the operator's shell and never on the droplet.
   - **Dev-secret rejection (MB-0022 P2)** goes in the **container
     entrypoint**, not in Rust: a shell entrypoint exits non-zero, before
     exec'ing the binary, when `KAUPPA_SESSION_SECRET` equals the dev
     default from `.env.example` or is under 64 bytes. `config.rs`'s own
     64-byte check stays as the second layer. `smoke_image.sh` covers both
     cases.
   - **No secret passes through Terraform** (constraint 1).
4. **Backups (Q4, A).**
   - A small pinned image (sqlite3 CLI ≥ 3.27, an S3 client, a scheduler
     such as `supercronic`) runs in the Compose stack. Each run:
     1. `sqlite3` with a busy timeout runs
        `VACUUM INTO '/tmp/kauppa-<ts>.db'` against `/data/kauppa.db`.
        The snapshot comes from SQLite, so commits still only in
        `kauppa.db-wal` are included (MB-0022 P5, MB-0025). `VACUUM INTO`
        is the one method; `.backup` is not used, so the drill tests the
        same path the schedule runs.
     2. `PRAGMA integrity_check` on the copy; anything but `ok` fails the
        run.
     3. gzip, upload, delete the local copy. Failure exits non-zero and is
        logged.
   - The job mounts `/data` **read-write**: a WAL reader needs to map
     `kauppa.db-shm`. It never copies, moves or deletes `kauppa.db`,
     `-wal` or `-shm`.
   - **Schedule and retention** (Spaces lifecycle rules by prefix):
     - `6h/`: every 6 hours, kept **14 days**;
     - `daily/`: the 03:00 UTC run, also written here, kept **90 days**;
     - `monthly/`: the first daily of each month, kept **12 months**.
     - RPO: 6 hours. Hourly is not chosen.
   - **Pre-update backup.** The update procedure (README, P7) runs the
     same job once on demand before pulling a new image, uploading under
     `pre-update/` (kept **90 days**). That is the restore point for the
     one-way-migration rollback.
   - **Terraform owns the backup bucket** (`ams3`), with its lifecycle
     rules, `lifecycle { prevent_destroy = true }` and the provider's
     default `force_destroy = false`. **Versioning is on**, with
     noncurrent versions expiring after 30 days. The README adds an
     occasional local pull by the owner, because a key with
     Read/Write/Delete on the droplet could still delete versions.
   - **Restore** (README): stop the app, download and gunzip, replace
     `kauppa.db`, remove any stale `kauppa.db-wal` and `kauppa.db-shm`,
     start the app.
   - Litestream (B) is a later brief, if a 6-hour RPO proves too much once
     MB-0025 has run in production for a while.
5. **DNS (Q5, A).** Outside Terraform: one A record (and AAAA if the
   droplet has IPv6) at the domain's current DNS provider, pointing at the
   reserved IP. It is added by hand, as a **deliberate step after MB-0023,
   MB-0025 and MB-0026 have merged** and the stack is running. README
   order: apply → start the stack → add the record → Caddy obtains the
   certificate over HTTP-01. The CRP's "B with `enable_dns = false`"
   variant for a new dedicated domain is not taken.
6. **Size and region (Q6).** Overridable Terraform variables with defaults:
   droplet `fra1`, `s-1vcpu-1gb`, with a 1 GiB swap file from cloud-init;
   a 5 GiB volume; both Spaces buckets in `ams3`. About $11.50/month before
   VAT. `s-1vcpu-2gb` is the documented step up.
7. **CI (Q7, A).** One new workflow under `.github/workflows/`, in **a
   separate PR from the `deploy/` files** (platform CODEOWNERS), with three
   jobs:
   - `image`: on PRs touching `deploy/**`, `crates/**`, `templates/**`,
     `assets/**`, `migrations/**` or `Cargo.*`, builds the image and runs
     `smoke_image.sh` and `smoke_compose.sh` (local CA). On push to `main`
     it also pushes to GHCR;
   - `terraform`: on PRs touching `deploy/terraform/**`, runs
     `check_terraform.sh` with no credentials;
   - `secrets`: on every PR, runs `check_no_secrets.sh`.
   - Permissions: `contents: read` at the top, `packages: write` only on the
     job that pushes. No deploy job: CI never holds an SSH key or DO token.
   - Third-party actions are pinned by major tag, like the existing
     workflows.

Covers:

- MB-0022's P1–P7 as written, with the sub-answers above;
- the separate `.github/workflows/` PR in sub-answer 7;
- the `security.md` and brief changes below.

Does not cover:

- any change to Rust code, `Cargo.toml`, `migrations/`, or session and
  cookie settings;
- a deploy job, or any credential held by CI beyond `GITHUB_TOKEN`;
- DigitalOcean DNS, or moving the domain's nameservers;
- Litestream, droplet backups or volume snapshots as the backup of record;
- rate limiting in Caddy (MB-0026's RR decides; a Caddy plugin is a
  separate security escalation);
- Caddy access logging with query strings (constraint 5);
- more than one app instance.

## Rationale and trade-offs

- **Q1 A — chosen.** $0 for a public package; no compiling on a 1 GB host;
  no registry credential on the droplet; reproducible images from reviewed
  commits; the same workflow runs the smoke scripts the agent can't.
  - **B (DOCR) not chosen:** 500 MiB Starter cap and garbage collection as
    a chore, plus credentials on both ends, one of them potentially in
    state.
  - **C (build on the droplet) not chosen:** a bigger droplet, a toolchain
    on the internet-facing host, and an OOM can take the app down.
- **Q2 B — chosen.** State survives the laptop, locking is ready if CI ever
  plans, versioning gives history, and it shares the Spaces subscription
  with backups.
  - **A (local) not chosen:** lost with the laptop; no locking.
  - **C (HCP) not chosen:** a third-party account whose free-tier terms
    have already changed once.
- **Q3 A — chosen.** The session secret never leaves the host or appears in
  a terminal, plan or state, and rotation needs no Terraform run.
  - **B not chosen:** the secret would also exist on the laptop for no use.
  - **C (`user_data`) rejected:** plain text in plan files, readable from
    droplet metadata (including from containers), and rotation replaces the
    droplet.
- **Q4 A — chosen.** It is exactly P5, works before and after MB-0025, gives
  plain files and a simple drill, and costs nothing extra. `VACUUM INTO`
  over `.backup`: one method, used by both the schedule and the drill.
  - **B (Litestream) not chosen now:** needs WAL, takes over checkpointing,
    has its own format and upgrade path, and a harder drill.
  - **C (droplet backups / snapshots) not chosen:** droplet backups don't
    cover the volume, and snapshots are crash-consistent, not SQLite
    snapshots.
- **Q5 A — chosen.** No nameserver move, other records stay put, and
  publishing the site stays a human act behind the app-side gate. B could
  publish the site on `apply`.
- **Q6 — chosen.** EU jurisdiction for both; backups in a different
  datacenter from the droplet at no cost; 1 GB is enough once nothing
  compiles on the host.
- **Q7 A — chosen.** P1–P4 and P6 become green or red on every PR for agent
  and owner alike, and GHCR images come only from `main`. B and C leave
  P1–P3 evidence to the owner's machine.

## Approvals and authority

Approved by platform-owner @juuso.kalliomaki (Q1 image delivery, Q2 state backend,
Q7 CI workflow); security @juuso.kalliomaki (Q3 secret delivery, constraint 1 and
the Caddy access-log rule in constraint 5); data-owner @juuso.kalliomaki (Q4
backup target, schedule, retention and Terraform ownership of the backup
bucket); owner @juuso.kalliomaki (Q5 DNS and its merge gate, Q6 size and region).
Risk tier high: each role above is a co-approver for its questions.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0014-deployment-platform-choices.md`
- Mission Brief: `.agentic/missions/MB-0022-first-deployment.md` (as of
  af1a838: P5/P7 cover WAL's three files and one-way migrations)
- Related consults and decisions:
  - RR-0010 (`KAUPPA_PUBLIC_ORIGIN`, `Origin` on smoke-test POSTs);
  - CRP-0012 Q5 on `agentic/mb-0023-invites` (invitation tokens in
    `?invite=`; MB-0022's Caddy logs must drop query strings);
  - CRP-0012's rollback finding (old binaries refuse a migrated database),
    already in MB-0022 P7;
  - MB-0025 (WAL, `-shm`, single instance).
- Evidence links: CRP-0014 → Current state and Evidence bundle:
  - DO droplet, volume, Spaces, DOCR and backup pricing; Spaces as a
    Terraform backend with `use_lockfile`; limited-access Spaces keys;
  - GHCR free for public packages; repo visibility `PUBLIC`;
  - DO provider `user_data`: hashed in state, `ForceNew`, readable through
    droplet metadata;
  - droplet backups exclude volumes;
  - Litestream v0.5.18 status; HCP free-tier change.

## Constraints and follow-on rules (Hardened)

1. **No secret through Terraform.** No secret value passes through
   Terraform variables, resources, data sources, `user_data` or plan files.
   No `random_password`, `digitalocean_spaces_key`, or
   `container_registry_docker_credentials` resources. Secrets on the
   droplet live only in 0600 files under the volume's `secrets/`
   directory. `check_terraform.sh` fails on any variable named
   `*token*|*secret*|*key*` that isn't `sensitive` or has a default; an SSH
   *public* key variable is the only name-based exception, and it is
   listed in the script.
2. **The dev secret never starts in a container.** The image's entrypoint
   refuses the `.env.example` dev value and anything under 64 bytes before
   the binary runs. Removing or bypassing that check needs an RR.
3. **Backups come from SQLite.** Every backup, scheduled or pre-update, is
   `VACUUM INTO` plus `integrity_check`; no job copies `kauppa.db`, `-wal`
   or `-shm`. Only one **app** container opens the database; the backup
   job is a short-lived SQLite reader on the same host and volume, which
   WAL supports. It never runs from another host or over a network share.
   A backup job that has failed silently is a P5 failure, so the README's
   routine includes listing the bucket.
4. **Updates take a backup first.** The README's update procedure runs the
   pre-update backup before `docker compose pull`. A rollback across a
   migration restores it; it never edits `_sqlx_migrations` by hand unless
   an RR says so.
5. **Caddy writes no query strings to logs.** The `Caddyfile` has no access
   log (no `log` directive). If a later change enables one, it must strip
   the query from `request>uri` (at least `invite`), so invitation tokens
   (CRP-0012 Q5) never reach a log file. Enabling access logs is a security
   review item.
6. **DNS is a deliberate manual step.** No Terraform resource, script or CI
   job creates the public DNS record. The owner adds it only after MB-0023,
   MB-0025 and MB-0026 have merged and the image in use contains them.
   Doing it earlier is an owner escalation (MB-0022).
7. **CI stays credential-free.** The new workflow holds no SSH key, DO
   token or Spaces key. Adding a deploy job or any such secret needs a new
   RR. The workflow lands in its own PR under `.github/` (platform
   CODEOWNERS), separate from the `deploy/` PR.
8. **The volume and backup bucket are protected.** `prevent_destroy` on the
   volume and the backup bucket. A plan that destroys or replaces either is
   a slow-mode stop (MB-0022).

## Roll-back into the system (Hardened)

To be applied in MB-0022's implementation PR (the `deploy/` PR, not the
`.github/` PR):

- `.agentic/mentorship/security.md` → Rules (must): add
  "**Deployment secrets.** No secret value passes through Terraform
  variables, resources, data sources, `user_data` or plan files. On the
  host, secrets live only in 0600 files under the data volume's `secrets/`
  directory; the session secret is generated there and never leaves it.
  The container refuses the dev session secret (MB-0022, RR-0014)."
- `.agentic/missions/MB-0022-first-deployment.md`:
  - `resolutions: []` → `resolutions: [RR-0014]`;
  - P2: after "the app refuses to start with the dev session secret or a
    secret under 64 bytes", add "(the dev-secret check is in the container
    entrypoint; RR-0014)";
  - P3: append "Caddy writes no access log containing query strings
    (RR-0014 constraint 5)."
  - P5: after "copied off the droplet on a schedule", add "(`VACUUM INTO`
    every 6 hours to Spaces; retention per RR-0014)";
  - P7: "only one container may open the file" → "only one app container
    may open the file (the backup job is a short-lived reader on the same
    host; RR-0014)";
  - Conceptual plan step 1: "A CI job may come later." → "CI: one new
    workflow in a separate `.github/` PR (RR-0014 Q7)."; "Stop until an RR
    answers it." → "Answered by RR-0014.";
  - Conceptual plan step 5: "A scheduled job (per the RR)" → "A scheduled
    `VACUUM INTO` job (RR-0014)".
- `CRP-0014`: `state: open` → `answered`.

## Supersession and history

Status: active once approved. Supersedes nothing. Constraint 5 carries CRP-0012 Q5's
finding for MB-0022; if MB-0023's RR answers Q5 differently, this
constraint stays as a logging rule regardless.
