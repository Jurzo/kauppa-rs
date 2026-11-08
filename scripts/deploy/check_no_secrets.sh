#!/usr/bin/env bash
# check_no_secrets.sh: MB-0022 P6.
#
#   P6  No secret is committed: production .env, *.tfvars, Terraform state
#       and the DigitalOcean token are ignored by git. The check fails if any
#       tracked file matches those patterns.
#
# Three parts:
#   1. no tracked file has a secret-bearing name (.env, *.tfvars, state,
#      plans, .terraform/, secrets/app.env, backup.env, private keys);
#   2. git ignores sample paths of each kind, and does not ignore the
#      committed examples and the provider lock file;
#   3. no tracked file contains a DigitalOcean token or a private key.
#
# Needs git. Runs in a second or two; no network.

set -euo pipefail
cd "$(dirname "$0")/../.."
# shellcheck source=scripts/deploy/lib.sh
source scripts/deploy/lib.sh

require_cmd git grep

# Tracked file names that must never exist. Extended regex on the path.
FORBIDDEN_NAMES=(
    '(^|/)\.env$'                      # any real .env (deploy/.env, root .env)
    '(^|/)\.env\.[^/]*$'               # .env.production etc. (allowed: .env.example, below)
    '\.tfvars$'                        # Terraform variable files
    '\.tfvars\.json$'
    '\.tfstate$'                       # Terraform state and its backups
    '\.tfstate\.'
    '(^|/)\.terraform/'                # provider cache, backend config
    '\.tfplan$'                        # saved plans hold variable values
    '(^|/)terraform\.tfplan'
    '(^|/)crash(\.[0-9]+)?\.log$'      # Terraform crash logs
    '(^|/)secrets/'                    # the data volume's secrets/ directory
    '(^|/)app\.env$'
    '(^|/)backup\.env$'
    '(^|/)id_(rsa|ecdsa|ed25519)$'     # SSH private keys
    '\.pem$'
    '\.db$'                            # databases (may hold password hashes)
    '\.db-(wal|shm|journal)$'
)
ALLOWED_NAMES=(
    '(^|/)\.env\.example$'
)

tracked_forbidden() {
    local files bad="" f pat allowed
    files=$(git ls-files)
    while IFS= read -r f; do
        [[ -z "$f" ]] && continue
        for pat in "${FORBIDDEN_NAMES[@]}"; do
            if [[ "$f" =~ $pat ]]; then
                allowed=0
                for pat2 in "${ALLOWED_NAMES[@]}"; do
                    [[ "$f" =~ $pat2 ]] && allowed=1
                done
                [[ "$allowed" -eq 0 ]] && bad+="$f"$'\n'
                break
            fi
        done
    done <<<"$files"
    [[ -z "$bad" ]] || { log "    tracked secret-bearing files:"$'\n'"$(sed 's/^/      /' <<<"$bad")"; return 1; }
}
check P6 "no tracked file has a secret-bearing name" -- tracked_forbidden

# Sample paths git must ignore, one per kind of secret the guide mentions.
MUST_IGNORE=(
    .env
    deploy/.env
    .env.production
    deploy/terraform/terraform.tfvars
    deploy/terraform/prod.auto.tfvars
    deploy/terraform/secrets.tfvars.json
    deploy/terraform/terraform.tfstate
    deploy/terraform/terraform.tfstate.backup
    deploy/terraform/.terraform/terraform.tfstate
    deploy/terraform/.terraform/providers/x
    deploy/terraform/plan.tfplan
    deploy/terraform/crash.log
    deploy/secrets/app.env
    deploy/secrets/backup.env
    data/kauppa.db
    data/kauppa.db-wal
    kauppa.db-shm
)
MUST_NOT_IGNORE=(
    .env.example
    deploy/.env.example
    deploy/terraform/terraform.tfvars.example
    deploy/terraform/.terraform.lock.hcl
    deploy/terraform/main.tf
    deploy/compose.yaml
)

ignored_ok() {
    local p ok=0
    for p in "${MUST_IGNORE[@]}"; do
        git check-ignore -q --no-index "$p" || { log "    not ignored: $p"; ok=1; }
    done
    return "$ok"
}
check P6 "git ignores .env, tfvars, Terraform state/plans/.terraform, secrets/, databases" -- ignored_ok

not_ignored_ok() {
    local p ok=0
    for p in "${MUST_NOT_IGNORE[@]}"; do
        ! git check-ignore -q --no-index "$p" || { log "    wrongly ignored: $p"; ok=1; }
    done
    return "$ok"
}
check P6 "git does not ignore the examples and the provider lock file" -- not_ignored_ok

# Secret-shaped content in tracked files. The patterns are written so they
# don't match themselves.
CONTENT_PATTERNS=(
    'do[opr]_v1_[0-9a-f]{64}'                     # DigitalOcean API / OAuth / refresh tokens
    '-----BEGIN [A-Z ]*PRIVATE KEY-----'          # PEM / OpenSSH private keys
    'KAUPPA_SESSION_SECRET=[A-Za-z0-9+/]{64}'      # a real-looking session secret
    '(SPACES|AWS|BACKUP_SPACES)_SECRET_ACCESS_KEY=[A-Za-z0-9+/]{30,}'
)

content_ok() {
    local pat hits ok=0
    for pat in "${CONTENT_PATTERNS[@]}"; do
        hits=$(git grep -lIE -e "$pat" -- . || true)
        if [[ -n "$hits" ]]; then
            log "    pattern '$pat' found in: $(tr '\n' ' ' <<<"$hits")"
            ok=1
        fi
    done
    return "$ok"
}
check P6 "no tracked file contains a DigitalOcean token, private key or secret value" -- content_ok

# The operator guide says where every secret lives (P6, P7).
guide_ok() {
    local s ok=0
    for s in DIGITALOCEAN_TOKEN AWS_ACCESS_KEY_ID SPACES_ACCESS_KEY_ID secrets/app.env secrets/backup.env \
        KAUPPA_SESSION_SECRET terraform.tfvars; do
        grep -qF "$s" deploy/README.md || { log "    deploy/README.md doesn't mention $s"; ok=1; }
    done
    return "$ok"
}
check P6 "deploy/README.md says where each secret lives" -- guide_ok

summary
