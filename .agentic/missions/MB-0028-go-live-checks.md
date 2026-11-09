# Mission Brief — Go-live checks before public DNS

```yaml
---
id: MB-0028
owner: "@juuso-kalliomaki"
goal: "Make the droplet ready for public DNS: it runs an image with MB-0026's rate limiter, keyed by the real client address behind Caddy, a backup has been restored in a drill, and the live site passes MB-0022's P3 checks."
non_goals:
  - "Changes to the Rust code, `Cargo.toml` or `migrations/`."
  - "DNS records in Terraform (RR-0014: DNS stays outside it)."
  - "Monitoring, alerting or uptime checks."
  - "Running `terraform apply` or `destroy` (the owner does)."
acceptance:
  - id: P1
    property: "`deploy/compose.yaml` sets `KAUPPA_TRUSTED_PROXIES` to exactly the `internal` network's pinned subnet, and the Caddyfile sets no `trusted_proxies`. Through the local Compose stack, 11 failed logins for one account, each with a different spoofed `X-Forwarded-For`, end in 429 (the spoofed header doesn't change the limiter key), and the app logs no \"forwarded-for missing from a trusted proxy\" WARN (the setting matches the network). RR-0013 constraint 8."
    check: scripts/deploy/smoke_compose.sh
  - id: P2
    property: "The backup restore drill passes: a backup taken while the app is writing, restored into a scratch stack, brings back the users and lists that existed when it was taken (MB-0022 P5, never run so far)."
    check: scripts/deploy/backup_restore_drill.sh
  - id: P3
    property: "The droplet runs an image built from `main` at or after a359fda (MB-0026), started with `KAUPPA_TRUSTED_PROXIES` set, after a pre-update backup per `deploy/README.md`'s update procedure; `/healthz` answers 200 through Caddy."
    check: "manual: owner, on the droplet (`docker compose ps --format '{{.Name}} {{.Image}}'`, `kauppa-backup list`, `curl -fsS https://<domain>/healthz` or via the reserved IP), recorded in MB-0028's Continuity Pack"
  - id: P4
    property: "After public DNS points at the reserved IP, the live site passes `smoke_compose.sh`'s live-mode checks (HTTP→HTTPS, cookie flags, same-origin 303, cross-site 403), which closes MB-0022 P7."
    check: "manual: owner, `KAUPPA_SMOKE_EMAIL=… KAUPPA_SMOKE_PASSWORD=… scripts/deploy/smoke_compose.sh https://<domain>`, output in MB-0028's Continuity Pack"
  - id: P5
    property: "`terraform plan` from the owner's account shows no changes against the deployed infrastructure (no drift since the first apply; the state backend in fra1 works)."
    check: "manual: owner, `terraform plan` output (no secrets) in MB-0028's Continuity Pack"
context:
  - deploy/compose.yaml
  - deploy/Caddyfile
  - deploy/README.md
  - scripts/deploy/smoke_compose.sh
  - scripts/deploy/backup_restore_drill.sh
  - .agentic/continuity/MB-0022.md
  - .agentic/resolutions/RR-0013-auth-rate-limiting.md
  - .agentic/resolutions/RR-0014-deployment-platform-choices.md
autonomy:
  may_decide:
    - "how `smoke_compose.sh` drives the spoofed-header logins (names, accounts, ordering), in local mode only"
    - "the wording of `deploy/README.md` §3.9 once the setting is on by default"
  escalate:
    - { when: "running the restore drill (or any other Docker check) in CI, which changes `.github/`", to: platform-owner }
    - { when: "the drill or the live smoke fails in a way that needs a change to the app, the image or RR-0014's choices", to: owner }
    - { when: "`terraform plan` shows changes, above all any replacement of the volume, the backup bucket or the reserved IP", to: owner }
    - { when: "the live smoke would need to send failing logins to the real site (it would lock out the smoke account)", to: security }
  forbidden:
    - "run `terraform apply` or `destroy`, or call the DigitalOcean API with real credentials"
    - "commit secrets, tokens, `*.tfvars` with real values, or Terraform state"
    - "set `trusted_proxies` in the Caddyfile, or trust any range wider than the `internal` subnet"
    - "change Rust code, `Cargo.toml` or `migrations/`"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/security.md@v1
  - .agentic/mentorship/persistence.md@v1
resolutions: [RR-0013, RR-0014]
---
```

## Context

MB-0022 closed on 2026-11-09 with the stack running on the droplet but not
yet public. Its first-deploy record (MB-0022's pack → Evidence pointers →
P7) shows only Caddy publishing 80/443, nothing on 8080, no query strings
in Caddy's logs and a fresh backup. What it left open:

- The droplet runs `ghcr.io/jurzo/kauppa-rs:sha-23faf4a` (the #42 merge),
  which predates MB-0026 (#43, a359fda). RR-0014 constraint 6: public DNS
  only once MB-0023, MB-0025 and MB-0026 are in the running image.
- `KAUPPA_TRUSTED_PROXIES: 172.30.53.0/24` is commented out in
  `compose.yaml`; the `internal` subnet is already pinned. Without the
  setting every client shares one login bucket behind Caddy (fails safe,
  but one attacker can slow logins for everyone). RR-0013 constraint 8 asks
  for the setting, a Caddyfile without `trusted_proxies`, a smoke check that
  a spoofed `X-Forwarded-For` doesn't change the key, and the shared-bucket
  fallback in the operator guide (README §3.9 already describes it).
- `smoke_compose.sh` already proves Caddy replaces a client-sent
  `X-Forwarded-For` (echo upstream). P1 adds the end-to-end check against
  the app's limiter.
- `backup_restore_drill.sh` (MB-0022 P5) has never run: it needs Docker,
  which the agent's environment lacks.
- MB-0022 P7's check (`smoke_compose.sh https://<domain>`) and the owner's
  `terraform plan` were not recorded. The state backend moved to fra1 at
  the first deploy (owner's change in `versions.tf`).

The agent has no Docker and no DigitalOcean credentials: it changes
`compose.yaml`, the README and the smoke script and runs what it can
(`bash -n`, static checks); CI's `deploy` workflow runs `smoke_compose.sh`
on the PR; the owner runs P2–P5.

## Conceptual plan

1. `compose.yaml`: uncomment the setting; a static assertion in
   `smoke_compose.sh` (or `check_terraform.sh`'s style) that it equals the
   subnet. README §3.9: on by default, keep the fallback warning.
2. `smoke_compose.sh` (local mode): the spoofed-header login burst and the
   WARN check. CI runs it on the PR.
3. Owner: drill (P2), pre-update backup, pull the new image and
   `up -d` (P3), DNS, live smoke (P4), `terraform plan` (P5). Each result
   goes in this mission's pack.

Slow-mode triggers: the drill fails; `terraform plan` wants to replace
anything; the live site answers differently from the local stack.

## Validation plan

- P1: `smoke_compose.sh` on the PR (CI `deploy` workflow) and locally where
  Docker exists.
- P2: `backup_restore_drill.sh` on the owner's machine (or CI, if the
  platform owner agrees), output in the pack.
- P3–P5: the owner's commands and outputs, pasted into the pack.
