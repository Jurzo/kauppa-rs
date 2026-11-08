# Consultation Request Pack — Deployment platform choices for the first deploy (MB-0022)

```yaml
---
id: CRP-0014
mission: MB-0022
owner: "@juuso-kalliomaki"
target_role: platform
risk_tier: high
urgency: medium
state: open
runbook_trigger: "MB-0022 autonomy.escalate: \"choosing how the image reaches the droplet\" (platform-owner); \"choosing the Terraform state backend ... or how state is locked\" (platform-owner); \"choosing how secrets reach the droplet ... and whether any secret may pass through Terraform state\" (security); \"choosing the backup target and schedule ... and the retention\" (data-owner); \"whether the domain's DNS is managed by Terraform ... or outside it\" (owner); \"adding or changing anything under `.github/`\" (platform-owner). MB-0022 Conceptual plan step 1 (S1 stops until an RR answers). CODEOWNERS `/.github/` (platform)."
---
```

## Decision statement

Choose, for MB-0022's first deployment, how the image reaches the droplet,
where Terraform keeps its state, how secrets reach the droplet, where and how
often the database is backed up, whether DNS lives in Terraform, the default
droplet size and region, and whether a CI job builds the image and checks the
Terraform.

Routing. The schema takes one `target_role`, so this pack is filed as
`platform`. Each question below names its role:

| Q | Topic | Role (MB-0022 escalation) |
| --- | --- | --- |
| Q1 | Image delivery | platform-owner |
| Q2 | Terraform state backend and locking | platform-owner |
| Q3 | Secret delivery; secrets in state | security |
| Q4 | Backup target, schedule, retention | data-owner |
| Q5 | DNS in or out of Terraform | owner |
| Q6 | Droplet size and region defaults | owner (brief: agent may decide; asked here because cost and data location follow from it) |
| Q7 | CI jobs under `.github/` | platform-owner |

All roles are @juuso-kalliomaki. One RR can answer every question.

## Current state

Verified on `63da727` (2026-11-08), the commit that adds MB-0022..MB-0027.

**What the app needs from its host:**

- **Config.** Env only, prefix `KAUPPA_` (`config.rs`): `BIND`,
  `DATABASE_URL`, `LOG_LEVEL`, `SESSION_SECRET`, `COOKIE_SECURE`,
  `PUBLIC_ORIGIN`.
  - `main.rs` reads a `.env` if one exists ("best-effort"). In the container,
    env vars come from Compose's `env_file`.
  - `config.rs` refuses to start when the secret is under 64 bytes. It doesn't
    reject the dev default, which is 65 bytes. MB-0022 P2 wants the dev secret
    refused. That check can't be in Rust (non-goal and forbidden), so it goes
    in the container entrypoint or a Compose-level check. That's within the
    brief; it's noted here so the RR doesn't read P2 as a code change.
- **One process, one SQLite file.**
  - `repo/sqlite/mod.rs`: `create_if_missing`, `foreign_keys`,
    `max_connections(8)`.
  - No `journal_mode`, `busy_timeout` or `synchronous` pragma, so it uses
    SQLite's rollback journal until MB-0025 (WAL) lands.
  - Migrations run on start.
- **Files from the working directory.** `./templates` (`templates.rs`) and
  `./assets` (`main.rs`). The image copies both until MB-0024.
- **Health.** `GET /healthz` exists (brief: the container health check).
- **Sessions.** Signed cookie (`CookieSessionStore`). Rotating
  `KAUPPA_SESSION_SECRET` logs everyone out and loses nothing else.

**Repository and CI:**

- `Jurzo/kauppa-rs` is **public** on GitHub (`gh repo view`: `PUBLIC`). That
  makes GHCR images free (Q1).
- `.github/workflows/`:
  - `ci.yml`: fmt, clippy, test, `cargo deny`;
  - `agentic-gate.yml`: artifact, evidence and trigger-path checks.
  - Neither has Docker or Terraform steps.
  - Actions are pinned by major tag (`actions/checkout@v4`).
- `.github/` is a platform CODEOWNERS path.
- `.gitignore` already ignores `.env`, `.env.*` (except `.env.example`), `*.db`
  and `/data`. It doesn't ignore `*.tfvars`, `*.tfstate*`, `.terraform/` or
  plan files (P6 adds those).

**Agent environment:**

- Terraform 1.17.0 is installed.
- No Docker and no DigitalOcean credentials.
- So P1–P3 smoke scripts can run only on the owner's machine or a CI runner
  (this matters for Q7).

## Questions, options and recommendations

Prices are DigitalOcean and GitHub list prices checked on 2026-11-08, in USD
per month, before VAT. DigitalOcean adds Finnish VAT for an EU consumer
account. Sources are in the evidence bundle.

### Q1 — How the image reaches the droplet (platform-owner)

The release build compiles actix-web, sqlx, argon2, handlebars and tracing.
On a 1 GB droplet a cold `cargo build --release` of a tree this size is
likely to hit the OOM killer, or to need a swap file and take tens of
minutes. Either way it competes with the running app for memory, on the same
host.

**Option A — GitHub Actions builds, pushes to GHCR (recommended)**

- How it works:
  - a workflow on push to `main` (and on tags) builds the multi-stage image
    on `ubuntu-latest` (amd64, which matches the droplet);
  - it pushes `ghcr.io/jurzo/kauppa-rs:sha-<commit>` plus a moving `:main`
    tag;
  - Compose on the droplet pins one tag or digest; the operator updates by
    changing it and running `docker compose pull && up -d`.
- Pros:
  - public repo → public package → no storage or transfer charge (GitHub
    Packages billing: free for public packages);
  - the droplet pulls anonymously, so no registry credential lives on the
    host;
  - the same workflow can run the P1–P3 smoke scripts, which the agent can't
    run locally (Q7);
  - builds are reproducible from a commit, and the droplet never holds the
    source or a toolchain.
- Cons:
  - a `.github/` change (platform CODEOWNERS);
  - the job's `GITHUB_TOKEN` needs `packages: write`;
  - the image is public. It holds nothing that isn't already in the public
    repo, and no secret is baked in (P1 forbids `.env` in the image).
- Risk: low. The supply-chain surface is GitHub Actions itself plus the
  pinned base images.
- Cost: $0. A CI build with `Swatinem/rust-cache` or a BuildKit cache takes
  minutes, inside the free Actions minutes for a public repo.

**Option B — GitHub Actions builds, pushes to DigitalOcean Container Registry**

- Pros:
  - the registry sits next to the droplet;
  - DOCR integrates with the DO API.
- Cons:
  - the free Starter plan holds one repository and 500 MiB, with no overage
    allowed. A Rust runtime image (debian-slim plus binary, templates,
    assets) is about 40–90 MB, so a handful of tags fit, but garbage
    collection becomes an operator chore;
  - the private registry needs a DO API token in GitHub secrets (push) and
    registry credentials on the droplet (pull). The provider's
    `container_registry_docker_credentials` resource would put those in
    Terraform state (Q3).
- Risk: low to medium. There are more credentials to hold.
- Cost: $0 Starter, or $5 for Basic (5 repos, 5 GiB).

**Option C — Build on the droplet**

- Pros: no registry, no CI change.
- Cons:
  - needs a 2–4 GB droplet ($12–$24) or a swap file and long builds;
  - the toolchain and source live on the internet-facing host;
  - a build failure or OOM can take the running app down;
  - builds can't be reproduced off the host.
- Risk: medium.
- Cost: +$6 to +$18 per month in droplet size.

**Fallback until a CI job exists (under A):** the owner builds locally
(`docker buildx build --platform linux/amd64`) and pushes to GHCR with a
personal token, or pipes `docker save | ssh … docker load`. The README
documents one of them, so the first deploy doesn't wait on the workflow PR.

**Recommendation: A.**

### Q2 — Terraform state backend and locking (platform-owner)

With Q3's recommendation, state holds no secret:

- the DO token comes from `DIGITALOCEAN_TOKEN` and isn't stored;
- the provider stores `user_data` as a hash;
- no `digitalocean_spaces_key` or `random_password` resources.

State still holds the resource IDs, the reserved IP and the firewall rules.
Losing it means `terraform import` of roughly six resources (droplet, volume
+ attachment, reserved IP + assignment, firewall, SSH key).

**Option A — Local state file on the owner's machine**

- Pros: zero setup and zero cost; nothing third-party.
- Cons:
  - lost with the laptop;
  - no locking, which is fine for one operator but breaks if CI or a second
    machine ever runs `plan`;
  - the file must stay out of git (P6 covers that).
- Risk: low for one operator; recovery is the imports.
- Cost: $0.

**Option B — DigitalOcean Spaces S3 backend with `use_lockfile = true` (recommended)**

- How it works:
  - `backend "s3"` with the Spaces endpoint and the `skip_*` flags from DO's
    guide;
  - `use_lockfile = true` writes a `.tflock` object next to the state with S3
    conditional writes;
  - native S3 locking arrived in Terraform 1.10 and DO's guide uses it with
    `~> 1.11`. The owner has 1.17, so no DynamoDB is needed.
  - Credentials: `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` env vars, as
    DO's guide recommends. Passing them via `-backend-config` can leave them
    in `.terraform/` and in plan files.
- Pros:
  - state survives the laptop;
  - locking is there if CI `plan` is ever added;
  - the bucket shares the Spaces subscription with Q4's backups, so it adds no
    cost;
  - versioning on the bucket gives state history.
- Cons:
  - bootstrap ordering: the state bucket must exist before `terraform init`,
    so it's created by hand in the console (one step in the README);
  - one more key: a limited-access Spaces key scoped to the state bucket only,
    held by the operator.
- Risk: low.
- Cost: $0 marginal ($5 Spaces subscription, shared with Q4).

**Option C — HCP Terraform (Terraform Cloud), free tier**

- Facts:
  - the legacy free plan ended on 2026-03-31;
  - the current free tier covers 500 managed resources and one concurrent
    run, far more than this stack needs.
- Pros: managed state, locking and history UI.
- Cons:
  - a third-party account;
  - with remote execution, the DO token is stored in HCP. With local
    execution, it's only a state store with an extra login;
  - free-tier terms have changed once already.
- Risk: low to medium (vendor terms).
- Cost: $0 today.

**Recommendation: B.** State and backups go in separate buckets with
separate limited keys (Q3), both in AMS3 (Q6). Versioning is on for the state
bucket.

### Q3 — How secrets reach the droplet; may any pass through state (security)

Secrets in play:

| Secret | Who holds it | Needed on droplet? |
| --- | --- | --- |
| `KAUPPA_SESSION_SECRET` (64+ bytes) | app container | yes |
| Spaces key for backups (Q4) | backup container | yes |
| DO API token | operator's shell (`DIGITALOCEAN_TOKEN`) | **no** |
| Spaces key for TF state (Q2) | operator's shell (`AWS_*`) | **no** |
| SSH private key | operator's machine | no (public key via Terraform) |

**Option A — Generate the session secret on the droplet; `scp` the backup key (recommended)**

- **Session secret:**
  - cloud-init (no secret in it) writes `/mnt/<volume>/secrets/app.env`,
    mode 0600, owned by root, only if the file doesn't exist yet. The value
    comes from `openssl rand -base64 64 | tr -d '\n' | cut -c1-64`, as in
    `.env.example`.
  - The file sits on the data volume, so a replaced droplet keeps the secret
    and nobody is logged out.
  - Compose reads it with `env_file`.
  - Rotation: delete or regenerate the file, then `docker compose up -d`
    (README, P7).
- **Backup Spaces key:**
  - the owner creates a limited-access key in the DO console, scoped to the
    backup bucket;
  - then `scp`s a `backup.env` (mode 0600) next to `app.env`.
  - It originates in the console, so it never touches Terraform.
- **State:** no secret ever passes through Terraform.
  - P4's "every secret variable is `sensitive`, no default" still holds,
    because there are no secret variables.
  - `check_terraform.sh` asserts that any variable named
    `*token*|*secret*|*key*` is sensitive and has no default, so one added
    later is caught.
- Pros:
  - the session secret never leaves the host and never appears in a terminal,
    plan or state;
  - rotation needs no Terraform run;
  - the droplet holds no DO API token.
- Cons:
  - the secret lives in a file on the volume; a volume snapshot contains it;
  - one manual `scp` step at first deploy and on key rotation.
- Risk: low.

**Option B — Operator generates both locally and `scp`s one env file**

- Same as A, but the session secret is generated on the owner's machine.
- Pros: one uniform mechanism; the owner has a copy.
- Cons:
  - the secret exists on the laptop too (shell history, temp files);
  - it isn't needed anywhere but the droplet.
- Risk: low.

**Option C — Pass secrets as cloud-init `user_data` from sensitive Terraform variables**

- Facts:
  - the DO provider stores `user_data` as a hash in state
    (`resource_droplet.go`: `StateFunc: util.HashStringStateFunc()`), so
    state alone doesn't leak it;
  - but saved plan files (`plan -out`) contain it in plain text;
  - DO keeps it as droplet metadata, readable through the API and the control
    panel and by any process on the droplet at `169.254.169.254`. That
    includes containers, because Docker doesn't block link-local metadata by
    default;
  - `user_data` is `ForceNew`, so rotating a secret **replaces the droplet**.
- Variants that render the secret into a data source (`cloudinit_config`,
  `templatefile` into a `local_file`) or create it with `random_password` or
  `digitalocean_spaces_key` put the plain value in state.
- Risk: medium to high. **Not recommended.**

**Recommendation: A**, plus one constraint for the RR: *no secret value
passes through Terraform variables, resources, data sources, `user_data` or
plan files; secrets on the droplet live only in 0600 files under the volume's
`secrets/` directory.*

### Q4 — Backup target, schedule and retention (data-owner)

Facts that shape the choice:

- DO **droplet backups don't include volumes**, and the database lives on the
  volume. So droplet backups don't protect the data.
- Volume snapshots ($0.06/GiB-month) are crash-consistent images, not SQLite
  snapshots. P5 and the brief's "never a file copy of a live database"
  exclude them as the backup of record.
- **MB-0025** (WAL, busy timeout, `synchronous=NORMAL`) is in progress.
  - Before it lands, the rollback journal means a snapshot holds a read lock
    briefly. The database is small, so that's milliseconds.
  - After it lands, `.backup` and `VACUUM INTO` read a consistent snapshot
    without blocking the writer. The backup container must mount `/data`
    read-write, because a WAL reader needs the `-shm` file.
- Data volume estimate: a household app with a few users, kilobytes per list
  and recipe, so a database well under 50 MB for years. Compressed snapshots
  are smaller still.

**Option A — Scheduled snapshot container to Spaces (recommended)**

- A small pinned image (sqlite3 CLI plus an S3 client such as `rclone` or
  `aws-cli`) in the Compose stack, with a scheduler (`supercronic` or a
  sleep loop).
- Each run:
  - `sqlite3 /data/kauppa.db "VACUUM INTO '/tmp/kauppa-<ts>.db'"`, or
    `.backup`;
  - then `PRAGMA integrity_check` on the copy;
  - gzip, upload to `s3://<backup-bucket>/<prefix>/`, delete the local copy.
  - On failure it exits non-zero and logs, and Docker's restart policy plus
    `docker compose ps` show it.
- Proposed schedule and retention, implemented as Spaces lifecycle rules by
  prefix rather than prune scripts:
  - `6h/`: every 6 hours, kept **14 days**;
  - `daily/`: the 03:00 UTC run is also written here, kept **90 days**;
  - `monthly/`: the first daily of each month, kept **12 months**.

  That's about 56 + 90 + 12 ≈ 160 objects. At 10 MB each that's about
  1.6 GB, far inside 250 GiB.
- Worst-case loss (RPO): **6 hours.**
- Pros:
  - exactly what P5 describes;
  - simple, inspectable files, and restore is "download, gunzip, place on
    volume";
  - works before and after MB-0025;
  - the drill (`backup_restore_drill.sh`) is straightforward.
- Cons:
  - RPO measured in hours, not seconds;
  - a limited Spaces key has Read or Read/Write/Delete only, so a
    compromised droplet could delete backups. Mitigations: versioning on the
    bucket, an occasional local pull by the owner, or both.
- Risk: low.
- Cost: $0 marginal (the shared $5 Spaces subscription). Transfer counts
  against the droplet's 1,000 GiB allowance and is negligible.

**Option B — Litestream continuous replication to Spaces**

- Status:
  - 0.5.x is active (v0.5.18, 2026-11-03; Apache-2.0);
  - 0.5.0 replaced the WAL-segment format with LTX after a two-year pause,
    and early 0.5.0 adopters reported problems; 18 patch releases have
    followed.
- Needs WAL mode, so it depends on MB-0025 merging. It also takes over
  checkpointing, which interacts with MB-0025's settings.
- Pros: RPO of seconds; point-in-time restore.
- Cons:
  - one more long-running process holding a Spaces key;
  - the restore and drill are more involved;
  - its own format and version-upgrade path;
  - can't start until MB-0025 lands.
- Risk: medium.
- Cost: $0 marginal.

**Option C — DO droplet backups or volume snapshots**

- Droplet backups: +20% (weekly, $1.20) or +30% (daily, $1.80) of the
  droplet price, and they **don't cover the volume**.
- Volume snapshots: $0.06/GiB-month, crash-consistent, same provider. They
  aren't "a consistent SQLite snapshot" (P5).
- Useful at most as a second layer for the host, which cloud-init rebuilds
  anyway.
- **Not recommended** as the backup.

**Recommendation: A now**, with a follow-up brief for Litestream (B) once
MB-0025 has run in production for a while, if a 6-hour RPO is too much.

Sub-question: is 6-hourly / 14 d + daily / 90 d + monthly / 12 mo right, or
should the RPO be tighter (hourly is the same cost)?

Second sub-question: may Terraform manage the **backup bucket** with its
lifecycle rules and `prevent_destroy`? The provider's Spaces credentials would
come from `SPACES_ACCESS_KEY_ID` and `SPACES_SECRET_ACCESS_KEY` env vars, not
state.

- Recommendation: yes. Retention becomes code, and `prevent_destroy` plus
  the provider's default `force_destroy = false` stop `terraform destroy` from
  deleting backups.
- The access key itself stays a console step (Q3).
- The alternative is creating the bucket by hand with the rules documented in
  the README.

### Q5 — DNS in Terraform or outside (owner)

**Option A — Outside Terraform: one A (and AAAA) record at the current DNS provider (recommended)**

- The reserved IP is stable across droplet replacement, so the record is a
  one-time manual step.
- Pros:
  - no nameserver move;
  - other records at the registrar (mail, etc.) stay where they are;
  - pointing DNS stays a **deliberate human action** after MB-0023, MB-0025
    and MB-0026 merge, which is the gate the brief sets.
- Cons:
  - one manual step;
  - Caddy can't issue a certificate until the record resolves. The README
    orders it: apply → start the stack → add the record → Caddy obtains the
    certificate over HTTP-01.
- Cost: $0.

**Option B — DigitalOcean DNS managed by Terraform (`digitalocean_domain` + `digitalocean_record`)**

- Pros: everything in code; the record follows the reserved IP.
- Cons:
  - the domain's nameservers must move to DigitalOcean, so the whole zone,
    including any mail records, is then managed there;
  - a `terraform apply` can publish the site before the app-side gate, unless
    the record sits behind a variable defaulting to off.
- Cost: $0 (DO DNS is free).

**Recommendation: A**, unless the domain is new and dedicated to kauppa, in
which case B with an `enable_dns = false` default is fine.

### Q6 — Droplet size and region defaults (owner)

The brief lets the agent pick these as overridable variables. They're listed
because cost and data location follow from them.

- **Region.** DO has no Nordic region; the EU options are FRA1 (Frankfurt)
  and AMS3 (Amsterdam), plus LON1 (UK). Both FRA1 and AMS3 have Spaces.
  - Recommended defaults: **droplet `fra1`; Spaces buckets (state and
    backups) `ams3`.** Both are in the EU (data stays under GDPR
    jurisdiction).
  - Backups sit in a different datacenter from the droplet at no extra cost.
  - Latency from Finland is a few tens of ms to either.
- **Size.** With Q1 = A (no compiling on the host):
  - the app (tens of MB RSS), Caddy and the Docker daemon fit in **1 GB**.
    Default: `s-1vcpu-1gb`, $6, 25 GiB disk, 1,000 GiB transfer;
  - cloud-init adds a 1 GiB swap file as headroom;
  - `s-1vcpu-2gb` ($12) is the step up if memory is tight; Q1 = C would need
    it.
- **Volume:** 5 GiB ($0.50). Plenty of headroom; it can grow online.
- **Reserved IP:** free while assigned to a droplet. DO bills reserved IPs
  left unassigned.

**Monthly total with all recommendations:**

| Item | $/mo |
| --- | --- |
| Droplet `s-1vcpu-1gb` (fra1) | 6.00 |
| Volume 5 GiB | 0.50 |
| Spaces subscription (state + backups, ams3) | 5.00 |
| Reserved IP (assigned), DO firewall, GHCR (public), DNS at current provider | 0.00 |
| **Total (before VAT)** | **≈ 11.50** |

Variants:

- Q1 = C: +$6.
- Q2 = A saves nothing, because Q4 uses the same Spaces subscription.
- Daily droplet backups: +$1.80, not recommended.

### Q7 — CI jobs under `.github/` (platform-owner)

The agent has no Docker. Without a CI job, P1–P3 run only on the owner's
machine, and every iteration on the Dockerfile or Compose file needs a
round-trip.

**Option A — One new workflow, three jobs (recommended):**

1. **`image`** (on PRs touching `deploy/**`, `crates/**`, `templates/**`,
   `assets/**`, `migrations/**`, `Cargo.*`):
   - build the image;
   - run `scripts/deploy/smoke_image.sh` and `smoke_compose.sh` (local CA)
     on the runner.
   - On push to `main`, additionally push to GHCR.
   - Permissions: `contents: read` by default, `packages: write` only in the
     push step's job.
2. **`terraform`** (on PRs touching `deploy/terraform/**`):
   `scripts/deploy/check_terraform.sh`, which runs `fmt -check`,
   `init -backend=false` and `validate`, plus the static firewall and
   variable checks. It needs no credentials.
3. **`secrets`** (every PR): `scripts/deploy/check_no_secrets.sh` (P6).

- Third-party actions: `docker/setup-buildx-action`, `docker/login-action`,
  `docker/build-push-action`, `hashicorp/setup-terraform`, pinned like the
  existing ones (major tag), or by commit SHA if you prefer that for the new
  workflow.
- No deploy job: CI never holds an SSH key or DO token. The operator pulls.
- Pros: P1–P4 and P6 become green or red on every PR, for agent and owner
  alike; GHCR images come from reviewed `main` commits only.
- Cons: a CODEOWNERS path; some Actions minutes (free for public repos).

**Option B — Terraform validate and secrets check only; images built and
pushed by hand.** Smaller `.github/` change. P1–P3 stay owner-run.

**Option C — No CI change in this mission.** Everything runs by hand. That's
viable, but P1–P3 evidence for the Merge-Readiness Pack then has to come from
the owner's machine.

**Recommendation: A**, in a separate PR from the `deploy/` files so the
`.github/` review is isolated.

## Recommendation and question

Recommended set:

| Q | Recommendation |
| --- | --- |
| Q1 | GitHub Actions → public GHCR image, pinned by tag or digest in Compose; local build + push as the first-deploy fallback |
| Q2 | Spaces S3 backend, `use_lockfile = true`, versioned state bucket created by hand, creds via `AWS_*` env vars |
| Q3 | Session secret generated on the droplet into a 0600 file on the volume; backup key `scp`'d; **no secret through Terraform** (constraint) |
| Q4 | `VACUUM INTO`/`.backup` + integrity check + gzip → Spaces every 6 h; lifecycle 14 d / 90 d daily / 12 mo monthly; Terraform manages the bucket with `prevent_destroy`; Litestream as a later brief |
| Q5 | DNS outside Terraform: one A/AAAA record by hand, after MB-0023/0025/0026 merge |
| Q6 | `fra1` droplet `s-1vcpu-1gb` + 1 GiB swap, 5 GiB volume, Spaces in `ams3`; ≈ $11.50/mo before VAT |
| Q7 | One new workflow: image build + smoke (+ push on `main`), `check_terraform.sh`, `check_no_secrets.sh`; separate PR |

**Crisp question to answer:** Approve Q1–Q7 as recommended, or name the
deviating option per question, plus Q4's retention numbers and whether
Terraform may own the backup bucket.

## Blast radius and rollback

- **What can break:**
  - Q1: a bad `:main` image. The operator pins by tag or digest and rolls
    back by pinning the previous one.
  - Q2: a corrupt or lost state file. Restore from bucket versioning, or
    re-import the roughly six resources. Nothing in state is needed to keep
    the site running.
  - Q3: a lost `secrets/` directory logs everyone out (session secret) and
    stops backups (key). Regenerate it or `scp` it again; no data is lost.
  - Q4: silently failing backups. Mitigations: the drill (P5), the job's
    non-zero exit, and a README step to list the bucket.
  - Q7: a red CI job blocks PRs that touch `deploy/`. Revert the workflow
    PR.
- **Who is affected:** the owner, and later the site's users. Nothing in this
  pack touches the Rust code, schema or sessions.
- **Rollback plan:**
  - every choice is in `deploy/` files, `.github/workflows/<new>.yml` or the
    README. Reverting the PR removes it;
  - `terraform destroy` is guarded by `prevent_destroy` on the volume and
    the backup bucket;
  - the DNS record (Q5 = A) is removed by hand.

## Minimal context

- MB-0022 goal, P1–P7, autonomy (forbidden: `apply`/`destroy`, committing
  secrets or state, SSH open to 0.0.0.0/0, publishing the app port past
  Caddy, copying the live DB file, Rust, `Cargo.toml` or `migrations/`
  changes).
- Order (owner, 2026-11-08): MB-0023, MB-0025 and MB-0026 merge before
  public DNS. MB-0024 and MB-0027 any time.
- Sibling interactions:
  - MB-0024 removes the `templates/` and `assets/` copies from the image
    (owner escalation when it lands);
  - MB-0025 changes how Q4's backup job reads the database (WAL, `-shm`);
  - MB-0026 may put rate limiting in Caddy (custom build, security
    escalation). That would change Caddy's image, not these choices.

## Evidence bundle (Hardened)

Checked 2026-11-08.

- **Droplet prices.** DigitalOcean Droplet pricing,
  <https://www.digitalocean.com/pricing/droplets>: Basic 512 MiB $4, 1 GiB
  $6 (1 vCPU, 25 GiB SSD, 1,000 GiB transfer), 2 GiB $12, 2 GiB/2 vCPU $18,
  4 GiB $24.
- **Volumes.** <https://docs.digitalocean.com/products/volumes/details/pricing/>:
  $0.10/GiB-month, 1 GiB–16 TiB.
- **Spaces.** <https://docs.digitalocean.com/products/spaces/details/pricing/>:
  $5/month subscription, 250 GiB storage and 1,024 GiB outbound transfer
  included, then $0.02/GiB and $0.01/GiB.
- **Spaces access.** <https://docs.digitalocean.com/products/spaces/how-to/manage-access/>:
  limited-access keys can be scoped per bucket to Read or
  Read/Write/Delete.
- **Spaces regions.** <https://docs.digitalocean.com/products/spaces/details/availability/>:
  includes AMS3, FRA1, LON1.
- **Spaces as a Terraform backend.**
  <https://docs.digitalocean.com/products/spaces/reference/terraform-backend/>:
  - the `backend "s3"` block with `endpoints.s3`, the `skip_*` flags and
    `region = "us-east-1"`;
  - `use_lockfile = true` for a lock file in the same bucket (example pins
    `~> 1.11`);
  - credentials via `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY`, not
    `-backend-config`.
- **Native S3 locking.** S3 lockfile locking arrived in Terraform 1.10
  (conditional writes): <https://devopscube.com/terraform-state-locking-with-s3/>.
- **DOCR.** <https://www.digitalocean.com/pricing/container-registry>,
  <https://docs.digitalocean.com/products/container-registry/details/pricing>:
  - Starter: free, 1 repo, 500 MiB, no overage;
  - Basic: $5, 5 repos, 5 GiB, $0.02/GiB overage;
  - Professional: $20.
- **GHCR.** GitHub Packages is free for public packages:
  <https://docs.github.com/en/billing/managing-billing-for-your-products/managing-billing-for-github-packages/about-billing-for-github-packages>.
  - The GHCR billing discussion for private images is ongoing
    (<https://github.com/orgs/community/discussions/183054>).
  - Repo visibility: `gh repo view Jurzo/kauppa-rs --json visibility` →
    `PUBLIC`.
- **HCP Terraform.** <https://www.hashicorp.com/en/blog/continuing-hcp-terraform-s-enhanced-free-tier-experience>:
  the legacy free plan ended on 2026-03-31; the free tier has 500 managed
  resources.
- **Droplet backups.**
  - <https://docs.digitalocean.com/products/backups/details/pricing/>:
    weekly +20%, daily +30% on Basic plans.
  - <https://docs.digitalocean.com/products/backups/details/features/>:
    automated backups don't include volumes.
  - <https://docs.digitalocean.com/products/snapshots/details/pricing/>:
    volume snapshots $0.06/GiB-month.
- **Litestream.**
  - `gh api repos/benbjohnson/litestream/releases`: v0.5.18 (2026-11-03),
    v0.5.17 (2026-08-31), v0.5.16, v0.5.15; not archived; Apache-2.0.
  - 0.5 series overview: <https://www.x-cmd.com/blog/251016>.
  - Early-0.5.0 caution: <https://ecosistemastartup.com/?p=47637>.
- **DO provider and `user_data`.** `digitalocean/terraform-provider-digitalocean`
  `digitalocean/droplet/resource_droplet.go`: `user_data` is `ForceNew` with
  `StateFunc: util.HashStringStateFunc()`. Latest release v2.107.0
  (2026-11-04). The provider has a `spaces_key` resource, whose secret would
  land in state; Q3 avoids it.
- **Repo state.** `.github/workflows/{ci,agentic-gate}.yml` (no Docker or
  Terraform); `.github/CODEOWNERS` (`/.github/` platform); `.gitignore` (no
  Terraform patterns yet); `config.rs` (64-byte minimum, no dev-secret
  rejection); `repo/sqlite/mod.rs` (no WAL pragmas).
