#!/usr/bin/env bash
# check_terraform.sh: MB-0022 P4 for deploy/terraform/, with no credentials.
#
#   P4  The Terraform is formatted and validates. It declares a droplet, a
#       block-storage volume for /data (kept when the droplet is replaced), a
#       reserved IP, and a cloud firewall allowing inbound 80 and 443 from
#       anywhere and 22 only from the operator CIDRs in a variable. No secret
#       has a default value, and every secret variable is `sensitive`.
#
# Runs: terraform fmt -check, init -backend=false, validate, `terraform test`
# (plan-only, mocked provider: tests/plan.tftest.hcl), and static assertions
# for what a plan can't show (RR-0014 constraints 1 and 8).
#
# Needs terraform >= 1.10 and network access to the Terraform registry for
# the provider. Never touches state, a backend or the DigitalOcean API.

set -euo pipefail
cd "$(dirname "$0")/../.."
# shellcheck source=scripts/deploy/lib.sh
source scripts/deploy/lib.sh

require_cmd terraform awk grep
TF_DIR=deploy/terraform

# Never let a credential from the caller's shell reach these commands.
unset DIGITALOCEAN_TOKEN DIGITALOCEAN_ACCESS_TOKEN SPACES_ACCESS_KEY_ID SPACES_SECRET_ACCESS_KEY \
    AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY AWS_SESSION_TOKEN AWS_PROFILE
export TF_IN_AUTOMATION=1 TF_INPUT=0 CHECKPOINT_DISABLE=1

tf() { terraform -chdir="$TF_DIR" "$@"; }

section "terraform $(terraform version | head -1)"

fmt_ok() { tf fmt -check -recursive -diff; }
check P4 "terraform fmt -check" -- fmt_ok

init_ok() { tf init -backend=false -lockfile=readonly -no-color >/dev/null; }
check P4 "terraform init -backend=false (provider from the committed lock file)" -- init_ok

validate_ok() { tf validate -no-color; }
check P4 "terraform validate" -- validate_ok

test_ok() { tf test -no-color; }
check P4 "terraform test (firewall rules, SSH CIDR validation, defaults, retention)" -- test_ok

# ------------------------------------------------------------ static ----
section "static assertions"

# All .tf files, comments stripped (`#` and `//` lines).
tf_src() { cat "$TF_DIR"/*.tf | grep -vE '^[[:space:]]*(#|//)'; }

resource_present() {
    local type=$1 name=$2
    tf_src | grep -qE "^resource \"$type\" \"$name\" \{"
}
declares_all() {
    local r ok=0
    for r in "digitalocean_droplet app" "digitalocean_volume data" "digitalocean_reserved_ip app" \
        "digitalocean_reserved_ip_assignment app" "digitalocean_firewall app" "digitalocean_ssh_key operator" \
        "digitalocean_spaces_bucket backups"; do
        # shellcheck disable=SC2086 # two words on purpose
        resource_present $r || { log "    missing resource: $r"; ok=1; }
    done
    return "$ok"
}
check P4 "declares droplet, volume, reserved IP (+ assignment), firewall, SSH key, backup bucket" -- declares_all

# block_of TYPE NAME: the body of one resource block (terraform fmt layout).
block_of() {
    tf_src | awk -v head="resource \"$1\" \"$2\" {" '
        $0 == head { inside = 1; depth = 0 }
        inside {
            print
            depth += gsub(/\{/, "{") - gsub(/\}/, "}")
            if (depth == 0) exit
        }'
}

prevent_destroy_on() {
    block_of "$1" "$2" | grep -qE '^[[:space:]]*prevent_destroy[[:space:]]*=[[:space:]]*true' \
        || { log "    $1.$2 lacks lifecycle { prevent_destroy = true }"; return 1; }
}
check P4 "the data volume has prevent_destroy (RR-0014 constraint 8)" -- prevent_destroy_on digitalocean_volume data
check P4 "the backup bucket has prevent_destroy (RR-0014 constraint 8)" -- prevent_destroy_on digitalocean_spaces_bucket backups

volume_separate() {
    # The volume is its own resource, attached by id: replacing the droplet
    # doesn't replace it. Nothing may make it depend on the droplet.
    block_of digitalocean_droplet app | grep -qE 'volume_ids[[:space:]]*=[[:space:]]*\[digitalocean_volume\.data\.id\]' \
        && ! block_of digitalocean_volume data | grep -q 'digitalocean_droplet'
}
check P4 "the volume is independent of the droplet and attached by id" -- volume_separate

ssh_rule_ok() {
    local fw rule
    fw=$(block_of digitalocean_firewall app)
    # The inbound rule for port 22, as one line per attribute.
    rule=$(awk '/inbound_rule \{/ { r = "" ; inside = 1 } inside { r = r $0 "\n" } inside && /^[[:space:]]*\}/ { if (r ~ /port_range *= *"22"/) printf "%s", r; inside = 0 }' <<<"$fw")
    [[ -n "$rule" ]] || { log "    no inbound rule for port 22"; return 1; }
    grep -qE 'source_addresses[[:space:]]*=[[:space:]]*var\.ssh_allowed_cidrs[[:space:]]*$' <<<"$rule" \
        || { log "    port 22 must take its sources from var.ssh_allowed_cidrs only: $rule"; return 1; }
}
check P4 "SSH (22) is open only to var.ssh_allowed_cidrs" -- ssh_rule_ok

no_anywhere_ssh_default() {
    # No default for the CIDRs, and validation rejects 0.0.0.0/0 and ::/0
    # (exercised by terraform test above).
    local v
    v=$(awk '/^variable "ssh_allowed_cidrs" \{/ { inside = 1 } inside { print } inside && /^\}/ { exit }' "$TF_DIR/variables.tf")
    ! grep -qE '^[[:space:]]*default[[:space:]]*=' <<<"$v" || { log "    ssh_allowed_cidrs has a default"; return 1; }
    ! grep -rqE '"(0\.0\.0\.0/0|::/0)"' "$TF_DIR"/*.tfvars* 2>/dev/null || { log "    a tfvars file opens SSH to anywhere"; return 1; }
}
check P4 "ssh_allowed_cidrs has no default; no tfvars opens SSH to anywhere" -- no_anywhere_ssh_default

inbound_ports_ok() {
    local ports
    ports=$(block_of digitalocean_firewall app \
        | awk '/inbound_rule \{/ { inside = 1 } inside && /port_range/ { gsub(/[^0-9-]/, ""); print } inside && /^[[:space:]]*\}/ { inside = 0 }' \
        | sort -n | tr '\n' ' ')
    [[ "$ports" == "22 80 443 " ]] || { log "    inbound ports: $ports"; return 1; }
}
check P4 "inbound rules are exactly 22, 80 and 443" -- inbound_ports_ok

# RR-0014 constraint 1: any variable named like a secret must be sensitive and
# have no default. The one name-based exception is the SSH *public* key.
SECRET_NAME_EXCEPTIONS=("ssh_public_key")
secret_vars_ok() {
    local name body ok=0 exc skip
    while read -r name; do
        skip=0
        for exc in "${SECRET_NAME_EXCEPTIONS[@]}"; do
            [[ "$name" == "$exc" ]] && skip=1
        done
        [[ "$skip" -eq 1 ]] && continue
        if [[ "$name" =~ (token|secret|key|password|credential) ]]; then
            body=$(tf_src | awk -v head="variable \"$name\" {" '$0 == head { inside = 1 } inside { print } inside && /^\}/ { exit }')
            grep -qE '^[[:space:]]*sensitive[[:space:]]*=[[:space:]]*true' <<<"$body" || { log "    variable $name is not sensitive"; ok=1; }
            ! grep -qE '^[[:space:]]*default[[:space:]]*=' <<<"$body" || { log "    variable $name has a default"; ok=1; }
        fi
    done < <(tf_src | sed -nE 's/^variable "([^"]+)" \{/\1/p')
    return "$ok"
}
check P4 "every secret-named variable is sensitive with no default (exception: ssh_public_key)" -- secret_vars_ok

forbidden_resources() {
    ! tf_src | grep -qE '(resource|data) "(random_password|random_string|digitalocean_spaces_key|digitalocean_container_registry_docker_credentials|local_file|local_sensitive_file)"' \
        || { log "    a resource or data source that would put a secret in state"; return 1; }
}
check P4 "no resource that puts a secret in state (RR-0014 constraint 1)" -- forbidden_resources

backend_no_credentials() {
    ! awk '/backend "s3" \{/ { inside = 1 } inside { print } inside && /^  \}/ { exit }' "$TF_DIR/versions.tf" \
        | grep -qE '^[[:space:]]*(access_key|secret_key|token)[[:space:]]*='
}
check P4 "the S3 backend block holds no credentials" -- backend_no_credentials

lockfile_backend() {
    awk '/backend "s3" \{/ { inside = 1 } inside { print } inside && /^  \}/ { exit }' "$TF_DIR/versions.tf" \
        | grep -qE '^[[:space:]]*use_lockfile[[:space:]]*=[[:space:]]*true'
}
check P4 "the backend locks with use_lockfile (RR-0014 Q2)" -- lockfile_backend

provider_pinned() {
    tf_src | grep -qE '^[[:space:]]*version[[:space:]]*=[[:space:]]*"[0-9]+\.[0-9]+\.[0-9]+"' \
        && ! tf_src | grep -qE '^[[:space:]]*version[[:space:]]*=[[:space:]]*"[~><=!]'
}
check P4 "the DigitalOcean provider is pinned to an exact version" -- provider_pinned

cloud_init_no_secrets() {
    local f="$TF_DIR/cloud-init.yaml.tftpl"
    ! grep -qE '(DIGITALOCEAN_TOKEN|AWS_SECRET|SPACES_SECRET|BACKUP_SPACES_SECRET|dop_v1_)' "$f" \
        && ! grep -qE 'KAUPPA_SESSION_SECRET=[A-Za-z0-9+/]{16,}' "$f"
}
check P4 "cloud-init carries no secret value (the session secret is generated on the host)" -- cloud_init_no_secrets

summary
