# Mission Brief — First deployment: DigitalOcean droplet, Docker, Caddy, Terraform

```yaml
---
id: MB-0022
owner: "@juuso-kalliomaki"
goal: "Make kauppa-rs deployable to the internet in a repeatable way: Terraform creates a DigitalOcean droplet with a persistent data volume and a firewall, Docker Compose runs the app behind Caddy (automatic HTTPS), the SQLite database survives redeploys and is backed up off the droplet, and an operator guide takes the owner from an empty DigitalOcean account to a working https:// site, and through updates and restores."
non_goals:
  - "Changes to the Rust code, routes, templates or `Cargo.toml`. The app-side gaps have their own briefs: invite-only registration (MB-0023), embedded templates and assets (MB-0024), SQLite WAL and busy timeout (MB-0025), login rate limiting (MB-0026), vendored htmx (MB-0027)."
  - More than one app instance, load balancing, or a database other than SQLite.
  - Kubernetes, DigitalOcean App Platform, or managed databases.
  - Monitoring, alerting and uptime checks beyond Docker's restart policy and the container health check.
  - Running `terraform apply` against the real account. The owner does that; the agent works with `validate` and `plan` output only.
acceptance:
  - id: P1
    property: "The Docker image builds from a clean checkout (multi-stage, release build). It runs as a non-root user, contains the binary, `templates/`, `assets/` and `migrations/` but no source tree, toolchain or `.env`, and serves the login page, `/assets/app.css` and a registered user's lists from a database on the `/data` volume."
    check: scripts/deploy/smoke_image.sh
  - id: P2
    property: "The database lives on the volume: after `docker compose down` and `up` with a rebuilt image, a user registered before can still log in and see their lists. Migrations run on start, and the app refuses to start with the dev session secret or a secret under 64 bytes (the dev-secret check is in the container entrypoint; RR-0014)."
    check: scripts/deploy/smoke_image.sh
  - id: P3
    property: "Through the Compose stack, Caddy is the only thing listening on public ports (80 and 443); the app is reachable only on the internal network. Caddy passes `Host` through and redirects HTTP to HTTPS. The app runs with `KAUPPA_COOKIE_SECURE=true` and `KAUPPA_PUBLIC_ORIGIN` set: a same-origin POST succeeds, a cross-site POST gets 403 (MB-0018), and the session cookie is `Secure; HttpOnly; SameSite=Lax`. Caddy writes no access log containing query strings (RR-0014 constraint 5)."
    check: scripts/deploy/smoke_compose.sh
  - id: P4
    property: "The Terraform in `deploy/terraform/` is formatted and validates. It declares a droplet, a block-storage volume mounted for `/data` (kept when the droplet is replaced), a reserved IP, and a cloud firewall allowing inbound 80 and 443 from anywhere and 22 only from the operator CIDRs in a variable. No secret has a default value, and every secret variable is `sensitive`."
    check: scripts/deploy/check_terraform.sh
  - id: P5
    property: "A backup is a consistent SQLite snapshot (online `.backup` or `VACUUM INTO`, never a file copy of a live database) copied off the droplet on a schedule (`VACUUM INTO` every 6 hours to Spaces; retention per RR-0014). With WAL (MB-0025) recent commits may still be only in `kauppa.db-wal`, so the snapshot must come from SQLite, not from the files. A restore drill on a scratch stack, taken while the app is writing, brings back the users and lists that existed when the backup was taken."
    check: scripts/deploy/backup_restore_drill.sh
  - id: P6
    property: "No secret is committed: production `.env`, `*.tfvars`, Terraform state and the DigitalOcean token are ignored by git, and the operator guide says where each secret lives. The check fails if any tracked file matches those patterns."
    check: scripts/deploy/check_no_secrets.sh
  - id: P7
    property: "`deploy/README.md` covers first deploy from an empty account (token, SSH key, DNS, `terraform apply`, secrets, first start), updating to a new version, backup and restore, rotating `KAUPPA_SESSION_SECRET`, and tearing down without losing the volume. Its database section (MB-0025) says: the database is three files (`kauppa.db`, `kauppa.db-wal`, `kauppa.db-shm`) that stay together on the data volume; never copy, move or delete one of them while the app runs, and never copy `kauppa.db` alone as a backup; a restore stops the app, replaces `kauppa.db`, removes any stale `-wal` and `-shm`, then starts it; `synchronous=NORMAL` means a power or OS failure can lose the last few seconds of commits but not corrupt the database; and only one app container may open the file (the backup job is a short-lived reader on the same host; RR-0014). It also warns that rolling back to a binary older than the database's newest migration fails at startup, so a rollback after a migration means restoring the pre-update backup. The owner follows it for the first real deploy, and the resulting https:// site passes the P3 checks against the real domain."
    check: "manual: owner's first deploy, recorded in MB-0022's Continuity Pack (scripts/deploy/smoke_compose.sh pointed at the live URL)"
context:
  - crates/kauppa-web/src/main.rs
  - crates/kauppa-web/src/config.rs
  - crates/kauppa-web/src/templates.rs
  - crates/kauppa-core/src/repo/sqlite/mod.rs
  - .env.example
  - .gitignore
  - .agentic/mentorship/security.md
  - .agentic/resolutions/RR-0010-csrf-mechanism.md
autonomy:
  may_decide:
    - "the base images (pinned by digest or exact tag), within: a Rust builder stage and a slim runtime stage"
    - "the layout under `deploy/` (e.g. `deploy/Dockerfile`, `deploy/compose.yaml`, `deploy/Caddyfile`, `deploy/terraform/`, `deploy/README.md`)"
    - "droplet size and region defaults, as Terraform variables the operator can override"
    - "the container health check (the existing `GET /healthz`)"
    - "how the smoke scripts drive the app (curl), and their names under `scripts/deploy/`"
  escalate:
    - { when: "choosing how the image reaches the droplet (built in GitHub Actions and pushed to GHCR or DigitalOcean Container Registry, or built on the droplet)", to: platform-owner }
    - { when: "choosing the Terraform state backend (local file, DigitalOcean Spaces, Terraform Cloud) or how state is locked", to: platform-owner }
    - { when: "choosing how secrets reach the droplet (session secret, Spaces keys) and whether any secret may pass through Terraform state", to: security }
    - { when: "choosing the backup target and schedule (Spaces with a sqlite3 job, Litestream, droplet backups), and the retention", to: data-owner }
    - { when: "whether the domain's DNS is managed by Terraform (DigitalOcean DNS) or outside it", to: owner }
    - { when: "pointing public DNS at the droplet before MB-0023 (invite-only registration) is merged, or before MB-0025 and MB-0026", to: owner }
    - { when: "MB-0026's RR chooses rate limiting in Caddy (custom Caddy build with a plugin) instead of in the app", to: security }
    - { when: "MB-0024 merges first and the image no longer needs `templates/` and `assets/` copied", to: owner }
    - { when: "adding or changing anything under `.github/` (e.g. an image build or `terraform validate` job)", to: platform-owner }
    - { when: "a deploy/infra runbook is needed because `feature.md`'s gates don't fit", to: owner }
    - { when: "the deployment needs any change to the Rust code, `Cargo.toml` or `migrations/`", to: owner }
  forbidden:
    - "run `terraform apply` or `destroy`, or call the DigitalOcean API with real credentials"
    - commit secrets, tokens, `*.tfvars` with real values, or Terraform state
    - "open SSH (22) to 0.0.0.0/0 in the firewall, or publish the app's port past Caddy"
    - "back up by copying the live database file"
    - change Rust code, `Cargo.toml`, `migrations/` or session and cookie settings
    - lower lint strictness
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/security.md@v1
  - .agentic/mentorship/persistence.md@v1
resolutions: [RR-0014]
---
```

## Context

Requested by the owner (2026-11-08): the app is feature-complete for a
first deployment (MB-0001..MB-0021 delivered; MB-0018's cross-site check
lifts RR-0008's localhost restriction). The owner chose a DigitalOcean VPS,
Docker with Caddy, and Terraform for the infrastructure.

What the app needs from its host, as of `main` at e63b4f1:

- **One process, one SQLite database, local disk.** `KAUPPA_DATABASE_URL`
  points at the file; migrations run on start. With MB-0025 the pool opens
  it in WAL mode (`synchronous=NORMAL`, 10 s busy timeout), so the database
  is three files (`kauppa.db`, `-wal`, `-shm`) that must share the volume,
  and copying `kauppa.db` alone is not a backup. WAL needs a local
  filesystem: the block-storage volume is fine, a network share isn't.
  Only one instance may run against the database.
- **Migrations are one-way.** sqlx refuses to start a binary that doesn't
  know a migration the database already has, so rolling back the image
  after an update with a migration needs the pre-update backup.
- **Files read from the working directory.** Templates load from
  `./templates` and static files from `./assets` (`templates.rs`,
  `main.rs`). The image must copy both and set the working directory; this
  brief doesn't change that.
- **Sessions in a signed cookie** (`CookieSessionStore`), keyed by
  `KAUPPA_SESSION_SECRET` (64+ bytes). Changing the secret logs everyone out.
- **Behind HTTPS:** `KAUPPA_COOKIE_SECURE=true` and
  `KAUPPA_PUBLIC_ORIGIN=https://<domain>` (RR-0010). Caddy passes `Host`
  through by default, but the public origin is set anyway so the cross-site
  check doesn't depend on proxy behaviour.
- **htmx loads from unpkg** at runtime until MB-0027; the site works
  without it, more slowly.

DigitalOcean notes. The smallest droplets (1 GB RAM) can't reliably compile
the release build, which favours building the image in CI or locally and
pulling it on the droplet (an escalation above). A block-storage volume
keeps `/data` when the droplet is rebuilt. Cloud firewalls apply before
traffic reaches the droplet, and Docker's published ports bypass `ufw`, so
the firewall belongs in Terraform, not on the host.

Order with the app-side briefs (owner decision, 2026-11-08). The public
site needs invite-only registration first: MB-0023 must be merged before
public DNS points at the droplet, and so should MB-0025 (WAL) and MB-0026
(rate limiting). MB-0024 (embedded files) and MB-0027 (vendored htmx) can
land before or after; the image copies `templates/` and `assets/` until
MB-0024 removes the need. Infrastructure work (P1–P6) doesn't wait for them.
The first deploy for P7 needs a bootstrapped admin (MB-0023 P5), which the
operator guide covers.

The agent has no DigitalOcean credentials and no Docker in its current
environment; the smoke scripts run where Docker is available (the owner's
machine or a CI runner), and the first real deploy is the owner's (P7).

## Conceptual plan

1. **Clarify (S1).** One Consultation Request Pack covering the escalations
   that block design: image delivery, state backend, secret delivery,
   backup target, and DNS. CI: one new workflow in a separate `.github/`
   PR (RR-0014 Q7). Answered by RR-0014.
2. **Container (P1, P2).** A multi-stage Dockerfile, `.dockerignore` (no
   `target/`, `.env`, `data/`, `.git`), a non-root user, `/data` as a
   volume, and a health check. `smoke_image.sh` builds, runs and drives it
   with curl, sending `Origin` on POSTs.
3. **Compose and Caddy (P3).** `compose.yaml` with `kauppa` (no published
   ports) and `caddy` (80, 443, a volume for certificates), plus a
   `Caddyfile` for one domain. Locally, `smoke_compose.sh` uses Caddy's
   internal CA, so it runs without a real domain.
4. **Terraform (P4, P6).** Provider pinned, droplet with cloud-init
   (Docker, Compose, mount the volume), volume, reserved IP, firewall,
   SSH key; outputs for the IP. `check_terraform.sh` runs `fmt -check`,
   `init -backend=false`, `validate`, and static checks on the firewall
   rules and variables.
5. **Backups (P5).** A scheduled `VACUUM INTO` job (RR-0014) taking an
   online snapshot and copying it off the droplet; the drill restores into a
   scratch stack and logs in. The update procedure takes a backup before
   pulling a new image, for the one-way-migration case.
6. **Operator guide (P7).** `deploy/README.md`; the owner's first deploy
   closes the mission.

Checkpoints: P1–P2 green before Compose; P3 green before Terraform; the
owner reviews the `terraform plan` output before the first apply.

Slow-mode triggers:
- the image can't serve the app without a code change (e.g. a path that
  isn't relative to the working directory) — stop; it's a separate brief;
- anything needs a secret in Terraform state or in git;
- the plan would destroy or replace the data volume.

## Validation plan

- **P1, P2:** `scripts/deploy/smoke_image.sh` — `docker build`, inspect the
  image (user, file list), run with a temp volume, curl the login page and
  an asset, register and add a list (with `Origin`), recreate the container
  and log in again; start with the dev secret and with a short one and
  expect a non-zero exit. Output in the Merge-Readiness Pack.
- **P3:** `scripts/deploy/smoke_compose.sh [base-url]` — against the local
  stack by default, or the live URL for P7: HTTP→HTTPS redirect, app port
  not reachable from the host, same-origin POST 303, cross-site POST 403,
  cookie flags.
- **P4:** `scripts/deploy/check_terraform.sh` — `terraform fmt -check`,
  `validate`, and assertions on the firewall's inbound rules and on
  `sensitive` secret variables. A `terraform plan` against the owner's
  account goes in the pack as evidence when the owner runs it.
- **P5:** `scripts/deploy/backup_restore_drill.sh` — take a backup while
  the app is writing, restore into a fresh stack, log in, compare lists.
- **P6:** `scripts/deploy/check_no_secrets.sh` — `git ls-files` against the
  secret patterns, plus `git check-ignore` on sample paths.
- **P7:** the owner's first deploy, following `deploy/README.md`, with
  `smoke_compose.sh https://<domain>` output in the Continuity Pack.
