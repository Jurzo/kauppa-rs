# Deploying kauppa-rs

The operator guide for running kauppa-rs on the internet: one DigitalOcean
droplet, Docker Compose with Caddy in front, Terraform for the
infrastructure. It covers the first deploy from an empty account, updates,
backups and restores, secret rotation and teardown.

Decisions behind it: Mission Brief MB-0022 and Resolution Record RR-0014
(`.agentic/`). Commands marked **(laptop)** run on your machine; commands
marked **(droplet)** run on the droplet as root.

---

## 1. What runs where

```text
            internet
               │  80, 443 (TCP)             22 (TCP), only from ssh_allowed_cidrs
               ▼                             │
   ┌──── DigitalOcean cloud firewall ────────┴─────────────┐
   │ droplet (fra1, Ubuntu 24.04, reserved IPv4)           │
   │                                                       │
   │   caddy ──(internal network, 172.30.53.0/24)── kauppa │
   │   ports 80/443        no published port, no egress    │
   │                                                       │
   │   backup (VACUUM INTO every 6 h) ──► Spaces (ams3)    │
   │                                                       │
   │   /mnt/kauppa-data   ← block-storage volume           │
   │     db/        kauppa.db, kauppa.db-wal, kauppa.db-shm│
   │     secrets/   app.env, backup.env  (0600, root)      │
   └───────────────────────────────────────────────────────┘
```

| Piece | Where | Made by |
| --- | --- | --- |
| Droplet, volume, reserved IP, firewall, SSH key, backup bucket | DigitalOcean | `deploy/terraform/` |
| Terraform state | a Spaces bucket you create by hand (versioned) | you, once |
| App image | `ghcr.io/jurzo/kauppa-rs:sha-<commit>` (public) | GitHub Actions, or you (§3.6) |
| Backup image | built on the droplet from `deploy/backup/` | `docker compose build backup` |
| Caddy | `caddy:2.11.9-alpine`, pinned by digest | Docker Hub |
| Database | `/mnt/kauppa-data/db/` on the volume | the app (migrations on start) |
| DNS record | your DNS provider, by hand | you (§3.10), never Terraform |

The volume and the backup bucket have `prevent_destroy`: no Terraform plan can
delete or replace them (RR-0014 constraint 8). Replacing the droplet keeps the
volume, so the database and the session secret survive it.

## 2. Where each secret lives

No secret passes through Terraform variables, resources, `user_data` or plan
files, and none is committed to git (RR-0014 constraint 1;
`.agentic/mentorship/security.md` → Deployment secrets).
`scripts/deploy/check_no_secrets.sh` checks git.

| Secret | Lives in | Never in |
| --- | --- | --- |
| `KAUPPA_SESSION_SECRET` | `/mnt/kauppa-data/secrets/app.env` on the droplet (0600, root). Generated there by cloud-init on first boot; never leaves the host. | your laptop, git, Terraform |
| Backup Spaces key (`BACKUP_SPACES_ACCESS_KEY_ID`, `BACKUP_SPACES_SECRET_ACCESS_KEY`) | `/mnt/kauppa-data/secrets/backup.env` on the droplet (0600, root). Limited-access key, scoped to the backup bucket. | git, Terraform |
| DigitalOcean API token (`DIGITALOCEAN_TOKEN`) | your shell / password manager only | the droplet, git, CI |
| State bucket key (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`) | your shell only. Limited-access key, scoped to the state bucket. Never `-backend-config`. | the droplet, `.terraform/`, git |
| Spaces key for Terraform (`SPACES_ACCESS_KEY_ID`, `SPACES_SECRET_ACCESS_KEY`) | your shell only (Terraform manages the backup bucket with it) | the droplet, git |
| GHCR token (`write:packages`, fallback image push only) | your machine only | the droplet (it pulls anonymously), git |
| SSH private key | your machine (`~/.ssh/`) | anywhere else; Terraform only gets the public key |

Git-ignored on purpose: `.env`, `deploy/.env`, `terraform.tfvars`, `*.tfstate*`,
`.terraform/`, `*.tfplan`, `secrets/`, `app.env`, `backup.env`, `*.db*`.
`deploy/.env` and `terraform.tfvars` hold no secret either; they are ignored
because they are yours.

## 3. First deploy from an empty account

### 3.1 Before you start

- **The app-side gate.** Public DNS points at the droplet only after
  **MB-0023** (invite-only registration), **MB-0025** (SQLite WAL) and
  **MB-0026** (login rate limiting) have merged *and the image you run contains
  them* (RR-0014 constraint 6). Everything up to §3.9 can happen before that.
- Tools on your laptop: Terraform ≥ 1.10, Docker with buildx, `ssh`, `scp`,
  `openssl`.
- A domain whose DNS you control, e.g. `kauppa.example`.
- Cost (RR-0014 Q6): droplet $6, volume $0.50, Spaces $5 (state and backups
  share it) ≈ **$11.50/month** before VAT.

### 3.2 DigitalOcean API token

1. Control panel → API → Tokens → *Generate New Token*. Give it an expiry and
   write access (Full Access, or Custom Scopes covering droplets, block
   storage, firewalls, reserved IPs, SSH keys and their reads).
2. Keep it in your password manager. **(laptop)**, per shell session:

   ```bash
   export DIGITALOCEAN_TOKEN=...      # never in a file in the repo
   ```

### 3.3 Spaces: state bucket and keys

1. Control panel → Spaces Object Storage → enable the subscription.
2. **State bucket**, by hand: create a bucket in **fra1** (the region of the
   backend endpoint in `terraform/versions.tf`), e.g. `kauppa-tfstate-<you>`,
   file listing restricted, then turn **versioning on** for it (bucket
   settings). It must not be the backup bucket.
3. **State key**: Spaces → Access Keys → *Create Access Key* → *Limited
   Access*, scoped to the state bucket only, Read/Write/Delete.
4. **Terraform's Spaces key**: Terraform creates the backup bucket and its
   lifecycle rules, so it needs a key that can create buckets (Full Access).
   It stays in your shell; you can delete it after `apply` and create a new
   one when you next change the bucket.
5. **(laptop)**:

   ```bash
   export AWS_ACCESS_KEY_ID=...          # state key (3)
   export AWS_SECRET_ACCESS_KEY=...
   export SPACES_ACCESS_KEY_ID=...       # Terraform's Spaces key (4)
   export SPACES_SECRET_ACCESS_KEY=...
   ```

### 3.4 SSH key and your address

```bash
# (laptop) if you don't have one yet
ssh-keygen -t ed25519 -f ~/.ssh/kauppa_ed25519
curl -4 https://ifconfig.co         # your public IPv4 → 203.0.113.7/32
```

SSH is open only to `ssh_allowed_cidrs`. Never use `0.0.0.0/0` or `::/0`:
the variable's validation rejects them, and the brief forbids it. If your
address changes, update `terraform.tfvars` and `apply` (only the firewall
changes).

### 3.5 Terraform: init, plan, apply

```bash
# (laptop)
cd deploy/terraform
cp terraform.tfvars.example terraform.tfvars   # git-ignored; edit it:
#   backup_bucket_name = "kauppa-backups-<you>"
#   ssh_public_key     = "<contents of ~/.ssh/kauppa_ed25519.pub>"
#   ssh_allowed_cidrs  = ["203.0.113.7/32"]

terraform init -backend-config="bucket=kauppa-tfstate-<you>"   # bucket name only, never keys
terraform plan
```

Review the plan before applying. On the first apply it creates 7 resources:
SSH key, volume, droplet, reserved IP and its assignment, firewall, backup
bucket. **Stop if any later plan wants to destroy or replace
`digitalocean_volume.data` or `digitalocean_spaces_bucket.backups`**
(`prevent_destroy` makes such a plan fail anyway). Then:

```bash
terraform apply
terraform output       # reserved_ip, volume_mount, backup_bucket, ssh
```

Avoid `terraform plan -out=...` files; if you save one, it is git-ignored and
should be deleted after use.

cloud-init then, on the droplet: installs Docker and Compose, adds 1 GiB of
swap, mounts the volume at `/mnt/kauppa-data`, creates `db/` (owned by the
container user, uid 10001) and `secrets/` (root, 0700), generates
`secrets/app.env` with a fresh 64-byte `KAUPPA_SESSION_SECRET` if it doesn't
exist, and turns off SSH password logins.

### 3.6 The image

The droplet pulls `ghcr.io/jurzo/kauppa-rs:sha-<commit>` anonymously.

- **With the CI workflow** (RR-0014 Q7, its own PR): every push to `main`
  builds and pushes `sha-<7-char commit>` and `:main`. Use the `sha-` tag of
  the commit you want, never `:main`.
- **Until the workflow has merged** (RR-0014 Q1 fallback), build on your
  laptop and push with a personal access token scoped to `write:packages`,
  kept on your machine only:

  ```bash
  # (laptop) from the repository root, at the commit to deploy
  echo "$GHCR_TOKEN" | docker login ghcr.io -u <github-user> --password-stdin
  tag=ghcr.io/jurzo/kauppa-rs:sha-$(git rev-parse --short=7 HEAD)
  docker buildx build --platform linux/amd64 -f deploy/Dockerfile \
      --build-arg KAUPPA_REVISION="$(git rev-parse HEAD)" -t "$tag" --push .
  ```

  The first push creates a **private** package: on GitHub → Packages →
  `kauppa-rs` → Package settings → change visibility to **public** (the droplet
  holds no registry credential).

Before pushing, the Docker checks can run locally:
`scripts/deploy/smoke_image.sh`, `scripts/deploy/smoke_compose.sh`,
`scripts/deploy/backup_restore_drill.sh` (§9).

### 3.7 The backup key

1. Spaces → Access Keys → *Limited Access*, scoped to the **backup bucket**
   (`terraform output backup_bucket`), Read/Write/Delete.
2. **(laptop)** write it to a 0600 file, copy it to the volume, delete the
   local copy:

   ```bash
   umask 077
   cat > /tmp/backup.env <<'EOF'
   BACKUP_SPACES_ACCESS_KEY_ID=...
   BACKUP_SPACES_SECRET_ACCESS_KEY=...
   EOF
   scp /tmp/backup.env root@<reserved_ip>:/mnt/kauppa-data/secrets/backup.env
   ssh root@<reserved_ip> chmod 0600 /mnt/kauppa-data/secrets/backup.env
   rm /tmp/backup.env
   ```

A key that can write can also delete; bucket versioning keeps deleted or
overwritten backups for 30 days, and §6.4's local copy covers the rest.

### 3.8 First start

```bash
ssh root@<reserved_ip>                         # (laptop)

# (droplet)
cloud-init status --wait                       # "status: done"
findmnt /mnt/kauppa-data                       # the volume is mounted
ls -l /mnt/kauppa-data/secrets/                # app.env and backup.env, both -rw-------

git clone https://github.com/Jurzo/kauppa-rs.git /opt/kauppa
cd /opt/kauppa/deploy
cp .env.example .env                           # git-ignored; edit it:
#   KAUPPA_IMAGE=ghcr.io/jurzo/kauppa-rs:sha-<commit>
#   KAUPPA_DATA_ROOT=/mnt/kauppa-data
#   KAUPPA_DOMAIN=kauppa.example
#   KAUPPA_PUBLIC_ORIGIN=https://kauppa.example
#   KAUPPA_TLS=you@example.com                 # Let's Encrypt contact
#   KAUPPA_BACKUP_BUCKET=kauppa-backups-<you>

docker compose config --quiet                  # every required setting is there
docker compose build backup
docker compose pull
docker compose up -d
docker compose ps                              # kauppa healthy; caddy, backup up
docker compose logs kauppa | tail              # "listening"; migrations ran
docker compose exec backup kauppa-backup pre-update   # a first backup works
docker compose exec backup kauppa-backup list
```

Until the DNS record exists Caddy can't get a certificate and logs ACME
errors; that is expected. All commands below assume `cd /opt/kauppa/deploy`.

### 3.9 Settings from the sibling missions

These land with their own missions; set them when the image contains them.

- **MB-0026 (rate limiting): on by default since MB-0028.** `compose.yaml`
  sets `KAUPPA_TRUSTED_PROXIES: 172.30.53.0/24`, which must equal the
  `internal` network's subnet; there is nothing to set. The app keys login
  limits by the real client address that Caddy puts in `X-Forwarded-For`;
  Caddy has no `trusted_proxies`, so it overwrites any address a client
  sends, and `smoke_compose.sh` checks both (§9). The image must contain
  MB-0026 (built from `main` at or after a359fda); an older image ignores
  the setting. A droplet started before MB-0028 gets the setting with the
  next update (§8.1: pre-update backup, `git pull`, new image,
  `docker compose up -d`).

  **Shared-bucket fallback.** If you change the `internal` subnet, change
  `KAUPPA_TRUSTED_PROXIES` with it, and never widen it beyond that subnet.
  With the setting removed or not matching, the app sees Caddy as the
  client, so every visitor shares one login bucket: safe, but one attacker
  can slow logins for everyone (RR-0013). A trusted proxy that sends no
  usable `X-Forwarded-For` is limited the same way, as one client, and the
  app logs one WARN `forwarded-for missing from a trusted proxy` per
  process (§13).
- **After MB-0023 (invite-only registration):** bootstrap the first admin
  from the droplet (RR-0012):

  ```bash
  docker compose exec kauppa kauppa-web admin invite --note "owner"
  # open the printed link, register your account, then:
  docker compose exec kauppa kauppa-web admin grant you@example.com
  ```

  The invitation link carries a token in `?invite=`; Caddy has no access log
  and strips query strings from its own (error) logs, and the app logs paths
  only after MB-0023, so it isn't written to a log.
- **Image contents:** the runtime image holds only the binary (MB-0032,
  RR-0020).
- **After MB-0027 (vendored htmx):** nothing to do here; until then browsers
  load htmx from unpkg, and the site works without it, more slowly.

### 3.10 DNS (the deliberate step)

Only when §3.1's gate holds (MB-0023, MB-0025 and MB-0026 merged and in the
running image), at your DNS provider:

```text
kauppa.example.   A   <reserved_ip>     (TTL 300 to start)
```

No AAAA record: the droplet has no IPv6 by default (`enable_ipv6`), and a
droplet's IPv6 address changes when it is replaced. Then **(droplet)**
`docker compose logs -f caddy` until the certificate is obtained (HTTP-01;
port 80 must stay open).

### 3.11 Verify and record (MB-0022 P7)

```bash
# (laptop) with an existing account
KAUPPA_SMOKE_EMAIL=you@example.com KAUPPA_SMOKE_PASSWORD='...' \
    scripts/deploy/smoke_compose.sh https://kauppa.example
```

Then §4's droplet checks. Paste both outputs into
`.agentic/continuity/MB-0028.md` (Evidence pointers; MB-0022 closed and
moved this check to MB-0028's P4). Live mode sends no failing logins, so
it never throttles the account you test with.

## 4. Checks after a deploy

```bash
# (droplet)
docker compose ps                     # only caddy shows ports (0.0.0.0:80, :443)
docker compose ps --format '{{.Name}} {{.Ports}}'
ss -tlnp | grep -E ':(80|443|8080)\b' # 80 and 443 by docker-proxy; nothing on 8080
docker compose logs caddy | grep -c 'invite='      # 0: no query strings in Caddy's logs
docker compose exec backup kauppa-backup list 6h/  # newest object < 6 h old
```

## 5. The database (SQLite, WAL)

From MB-0025 the app opens SQLite in WAL mode (`synchronous=NORMAL`, a 10 s
busy timeout). What that means for the operator:

- **The database is three files**: `kauppa.db`, `kauppa.db-wal` and
  `kauppa.db-shm`, in `/mnt/kauppa-data/db/`. They stay together on the data
  volume. Recent commits can live only in `kauppa.db-wal` until a checkpoint.
- **Never copy, move or delete one of them while the app runs**, and **never
  copy `kauppa.db` alone as a backup**: it can be missing commits or be
  inconsistent. Backups come from SQLite (`VACUUM INTO`, §6), never from the
  files (RR-0014 constraint 3).
- **A restore** stops the app, replaces `kauppa.db`, removes any stale
  `kauppa.db-wal` and `kauppa.db-shm`, then starts it (§7; the restore command
  moves the old set aside rather than deleting it).
- **`synchronous=NORMAL`**: a power or OS failure can lose the last few
  seconds of commits, but cannot corrupt the database. An app crash loses
  nothing.
- **Only one app container may open the file.** Never `--scale kauppa=2`,
  never run a second stack against the same `db/` directory, never put it on a
  network share. The backup job is the exception: a short-lived SQLite reader
  on the same host and volume, which WAL supports (RR-0014).
- **Migrations are one-way.** The app runs migrations on start. A binary
  older than the database's newest migration **refuses to start**, so rolling
  back across a migration means restoring the pre-update backup (§8.2). Never
  edit `_sqlx_migrations` by hand (RR-0014 constraint 4).

## 6. Backups

### 6.1 What runs

The `backup` service runs `supercronic` with `deploy/backup/crontab`: at
03:00, 09:00, 15:00 and 21:00 UTC it runs `kauppa-backup scheduled`, which:

1. runs `VACUUM INTO '/tmp/kauppa-<ts>.db'` on `/data/kauppa.db` with a 10 s
   busy timeout. The snapshot comes from SQLite, so commits still only in the
   WAL are included;
2. runs `PRAGMA integrity_check` on the copy; anything but `ok` fails the run;
3. gzips it, uploads it, deletes the local copy. A failure exits non-zero and
   is logged (`docker compose logs backup`).

Worst-case data loss (RPO): 6 hours.

### 6.2 Retention (Spaces lifecycle rules, `deploy/terraform/backups.tf`)

| Prefix | Written | Kept |
| --- | --- | --- |
| `6h/` | every run | 14 days |
| `daily/` | the 03:00 UTC run | 90 days |
| `monthly/` | the 03:00 UTC run on the 1st | 12 months (365 days) |
| `pre-update/` | on demand, before every update (§8) | 90 days |

Versioning is on: overwritten or deleted objects stay as noncurrent versions
for 30 more days.

### 6.3 Routine (weekly)

```bash
# (droplet)
docker compose exec backup kauppa-backup list 6h/      # newest < 6 h old
docker compose logs --since 168h backup | grep -E 'FAILED|backup OK' | tail
```

A backup job that fails silently is a P5 failure: look at the list, not only
at the logs.

### 6.4 An off-provider copy (monthly)

The droplet's key can delete objects. Pull a recent backup to a machine you
control now and then, e.g. download `daily/<newest>.db.gz` from the Spaces
console, or with any S3 client and a read-only key.

### 6.5 On-demand backup

```bash
docker compose exec backup kauppa-backup pre-update     # into pre-update/
docker compose exec backup kauppa-backup run manual     # any prefix (no lifecycle rule: kept until deleted)
```

## 7. Restore

```bash
# (droplet)
docker compose exec backup kauppa-backup list daily/          # pick an object
docker compose stop kauppa backup                             # the app must be stopped
docker compose run --rm --no-deps backup restore --app-stopped daily/kauppa-<ts>.db.gz
docker compose start kauppa backup
docker compose ps                                             # kauppa healthy
```

`restore` downloads the object, checks `integrity_check`, moves the current
`kauppa.db` with its `-wal`/`-shm` (as a set) into
`db/pre-restore-<ts>/`, and puts the restored `kauppa.db` in place with no
stale `-wal` or `-shm`. Delete `pre-restore-*` directories once you are sure
(`rm -r /mnt/kauppa-data/db/pre-restore-<ts>`).

The restored database must not be newer (in migrations) than the image you
start; restore before an update's migration together with the old image
(§8.2).

To rehearse it without touching production: `scripts/deploy/backup_restore_drill.sh`
on any machine with Docker (§9).

## 8. Updating

### 8.1 To a new version

```bash
# (droplet)
cd /opt/kauppa/deploy
grep KAUPPA_IMAGE .env                                # note the current tag (rollback)
docker compose exec backup kauppa-backup pre-update   # ALWAYS first (RR-0014 constraint 4)
git pull                                              # compose/Caddy/backup changes, if any; read the diff
sed -i 's|^KAUPPA_IMAGE=.*|KAUPPA_IMAGE=ghcr.io/jurzo/kauppa-rs:sha-<new>|' .env
docker compose build backup                           # only if deploy/backup changed
docker compose pull
docker compose up -d
docker compose ps; docker compose logs --tail 20 kauppa
```

Then §4's checks, and `smoke_compose.sh https://<domain>` from your laptop.
Expect a few seconds of downtime while the app container is replaced.

**Before the first image with migration `0010_email_nocase.sql`**
(MB-0044, RR-0028): that migration makes email addresses unique
regardless of ASCII case, and fails if two accounts already differ only by
case. Check first; the query prints nothing when it is safe:

```bash
docker compose exec backup sqlite3 -readonly /data/kauppa.db \
  "SELECT lower(email), count(*) FROM users GROUP BY 1 HAVING count(*) > 1;"
```

Resolve any address it prints by hand before updating (the app never
merges or deletes accounts). If the migration fails anyway, the new image
won't start and nothing in the database has changed: go back to the
previous tag as in §8.2's first case.

### 8.2 Rolling back

- **No migration in between** (the new version added no file under
  `migrations/`): set `KAUPPA_IMAGE` back to the previous tag and
  `docker compose up -d`.
- **The update ran a migration**: the old binary refuses to start on the
  migrated database. Restore the `pre-update/` backup taken in §8.1 *and*
  start the old image:

  ```bash
  sed -i 's|^KAUPPA_IMAGE=.*|KAUPPA_IMAGE=ghcr.io/jurzo/kauppa-rs:sha-<previous>|' .env
  docker compose stop kauppa backup
  docker compose run --rm --no-deps backup restore --app-stopped pre-update/kauppa-<ts>.db.gz
  docker compose up -d
  ```

  Writes made between the update and the rollback are lost. This applies
  to `0010_email_nocase.sql` too: it only adds an index, but the older
  image refuses a database with a migration it doesn't know. Never edit
  `_sqlx_migrations` by hand unless an RR says so.

## 9. Testing the deployment locally

With Docker (BuildKit) and curl, from the repository root:

| Script | Checks |
| --- | --- |
| `scripts/deploy/smoke_image.sh` | P1, P2: image contents and user, serving from `/data`, secret refusals, data kept across a replaced container |
| `scripts/deploy/smoke_compose.sh` | P3 (+ P2 down/up): only Caddy publishes ports, HTTP→HTTPS, cookie flags, same-origin 303, cross-site 403, no query strings in Caddy's logs, spoofed `X-Forwarded-For` dropped; MB-0028 P1: `KAUPPA_TRUSTED_PROXIES` equals the `internal` subnet, failed logins with spoofed `X-Forwarded-For` are limited under the real client's key, no trusted-proxy fallback WARN |
| `scripts/deploy/smoke_compose.sh https://<domain>` | P3 against the live site (§3.11) |
| `scripts/deploy/backup_restore_drill.sh` | P5: back up while writing, restore into a fresh stack, log in, compare |
| `scripts/deploy/check_terraform.sh` | P4: fmt, validate, plan tests with a mocked provider, static rules (needs Terraform, no credentials) |
| `scripts/deploy/check_no_secrets.sh` | P6: nothing secret tracked by git |

`KAUPPA_SMOKE_IMAGE=<image>` reuses a built image; `KAUPPA_SMOKE_KEEP=1` keeps
the containers for inspection.

## 10. Rotating secrets

### 10.1 `KAUPPA_SESSION_SECRET`

Logs everyone out; loses nothing else.

```bash
# (droplet)
umask 077
f=/mnt/kauppa-data/secrets/app.env
printf 'KAUPPA_SESSION_SECRET=%s\n' "$(openssl rand -base64 64 | tr -d '\n' | cut -c1-64)" > "$f.new"
chmod 0600 "$f.new" && mv "$f.new" "$f"
docker compose up -d --force-recreate kauppa
```

The container refuses to start with the `.env.example` dev secret or anything
under 64 bytes (RR-0014 constraint 2). The app itself also refuses the dev
secret unless it binds to a loopback address with
`KAUPPA_COOKIE_SECURE=false` (RR-0026).

### 10.1a Sign-in lifetime

A sign-in ends 30 days after the user's last visit and 90 days after they
signed in, whichever comes first (MB-0042, RR-0026). The limits are
constants in the app, not settings. There is no server-side "sign out
everywhere": rotating the session secret (10.1) is the way to end every
session at once. The first deploy of an image with these limits signs every
user out once, because older sessions carry no sign-in time.

### 10.2 Other credentials

- **Backup key**: create a new limited key (§3.7), replace `secrets/backup.env`,
  `docker compose up -d --force-recreate backup`, run
  `docker compose exec backup kauppa-backup run manual`, then delete the old
  key in the console.
- **DO token, state key, Terraform's Spaces key**: they only live in your
  shell; create new ones in the console, delete the old ones.
- **SSH key**: change `ssh_public_key` and `apply`. This **replaces the
  droplet** (the volume, database and session secret are kept); then redo
  §3.8 from `git clone`, with the same `.env` values.

## 11. Replacing the droplet

`terraform apply -replace=digitalocean_droplet.app` (or a change that forces
it) destroys the droplet and creates a new one with the same volume and
reserved IP. Take a `pre-update` backup first. On the new droplet: §3.8 from
`git clone` (cloud-init finds the existing `secrets/app.env` and keeps it).
Caddy obtains a new certificate on start.

## 12. Tearing down without losing the volume

1. Final backup and a local copy (§6.5, §6.4).
2. Destroy only the compute and network resources. `prevent_destroy` makes a
   plain `terraform destroy` fail, which is the point:

   ```bash
   # (laptop) deploy/terraform
   terraform destroy \
       -target=digitalocean_firewall.app \
       -target=digitalocean_reserved_ip_assignment.app \
       -target=digitalocean_droplet.app
   ```

   The volume (with the database and `secrets/`), the backup bucket and the
   reserved IP remain. They keep costing money: an unattached volume is billed,
   and so is an unassigned reserved IP. To also release the IP, add
   `-target=digitalocean_reserved_ip.app` and remove the DNS record first.
3. To come back: `terraform apply` creates a new droplet with the same
   volume; then §3.8.

Deleting the volume or the backup bucket for good is a separate, deliberate
change: remove its `prevent_destroy` in a reviewed commit, apply, and keep a
local copy of the last backup. It is not part of a normal teardown.

## 13. Troubleshooting

| Symptom | Look at |
| --- | --- |
| `kauppa` restarting, logs say `refusing to start` | `secrets/app.env` missing, the dev secret, or under 64 bytes (§10.1) |
| `kauppa` unhealthy after an update, migration error | an older image on a migrated database (§8.2) |
| Caddy logs ACME errors | DNS not pointing at the reserved IP yet, or port 80 blocked |
| 403 on every form POST | `KAUPPA_PUBLIC_ORIGIN` doesn't match the URL in the browser (scheme, host, port) |
| `kauppa-backup: FAILED` | `docker compose logs backup`; key in `secrets/backup.env`, bucket name in `.env` |
| `docker compose config` errors about a variable | a required setting missing from `deploy/.env` (§3.8) |
| `kauppa` logs `forwarded-for missing from a trusted proxy`, or logins throttled for everyone at once | `KAUPPA_TRUSTED_PROXIES` doesn't match the `internal` subnet, or a proxy in front sends no `X-Forwarded-For`: everyone shares one login bucket (§3.9) |
