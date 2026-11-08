#!/usr/bin/env bash
# backup_restore_drill.sh: MB-0022 P5, the restore drill.
#
#   P5  A backup is a consistent SQLite snapshot (VACUUM INTO, never a file
#       copy). A restore drill on a scratch stack, taken while the app is
#       writing, brings back the users and lists that existed when the
#       backup was taken.
#
# What it does, with the production compose file and backup image:
#   1. Stack A: app + Caddy + backup job, on a scratch data directory; the
#      backup target is a local directory (compose.drill.yaml), not Spaces.
#   2. Registers a user and creates a list; then a writer keeps creating
#      numbered lists through Caddy (same-origin POSTs).
#   3. While it writes, runs `kauppa-backup run drill` in the backup
#      container: VACUUM INTO, integrity_check, gzip, "upload".
#   4. Stack B: a fresh data directory. Starts, stops the app, runs
#      `kauppa-backup restore --app-stopped drill/<file>`, starts the app.
#   5. Logs in as the user and checks the lists: every list acknowledged
#      before the backup started is back, the numbered lists form an
#      unbroken prefix (a consistent snapshot), and none acknowledged after
#      the backup finished is there.
#
# Needs Docker and curl. Usage: scripts/deploy/backup_restore_drill.sh
#
# Environment:
#   KAUPPA_SMOKE_IMAGE   test this app image instead of building it
#   KAUPPA_DRILL_HTTP_PORT / KAUPPA_DRILL_HTTPS_PORT  (default 28080 / 28443)
#   KAUPPA_SMOKE_KEEP=1  keep the stacks and directories

set -euo pipefail
cd "$(dirname "$0")/../.."
# shellcheck source=scripts/deploy/lib.sh
source scripts/deploy/lib.sh

require_cmd docker curl

HTTP_PORT=${KAUPPA_DRILL_HTTP_PORT:-28080}
HTTPS_PORT=${KAUPPA_DRILL_HTTPS_PORT:-28443}
DOMAIN=kauppa.test
RUN="kauppa-drill-$(rand_id)"
IMAGE=${KAUPPA_SMOKE_IMAGE:-kauppa-rs:smoke}
SMOKE_TMP=$(mktemp -d)
export SMOKE_TMP
BACKUPS="$SMOKE_TMP/backups"
BASE="https://$DOMAIN:$HTTPS_PORT"
ORIGIN="$BASE"
EMAIL="drill-$RUN@example.test"
PASSWORD="drill-password-$(rand_id)-long-enough"
JAR="$SMOKE_TMP/cookies"
WRITER_LOG="$SMOKE_TMP/writer.log"
WRITER_STOP="$SMOKE_TMP/writer.stop"
WRITER_PID=""
CURRENT=""

mkdir -p "$BACKUPS"
chmod 0777 "$BACKUPS"

# new_root NAME: a scratch KAUPPA_DATA_ROOT with db/ and secrets/.
new_root() {
    local root="$SMOKE_TMP/$1"
    mkdir -p "$root/db" "$root/secrets"
    chmod 0777 "$root/db"
    printf 'KAUPPA_SESSION_SECRET=%s\n' "$(rand_secret)" >"$root/secrets/app.env"
    : >"$root/secrets/backup.env"
    chmod 0600 "$root/secrets/app.env" "$root/secrets/backup.env"
    cat >"$root/compose.env" <<EOF
KAUPPA_IMAGE=$IMAGE
KAUPPA_DATA_ROOT=$root
KAUPPA_DOMAIN=$DOMAIN
KAUPPA_PUBLIC_ORIGIN=$ORIGIN
KAUPPA_TLS=internal
KAUPPA_HTTP_PORT=$HTTP_PORT
KAUPPA_HTTPS_PORT=$HTTPS_PORT
KAUPPA_BACKUP_BUCKET=drill-unused
KAUPPA_DRILL_BACKUP_DIR=$BACKUPS
EOF
    printf '%s\n' "$root"
}

# stack ROOT PROJECT ARGS...: docker compose for one scratch stack.
stack() {
    local root=$1 project=$2
    shift 2
    docker compose -p "$project" -f deploy/compose.yaml -f scripts/deploy/compose.drill.yaml \
        --env-file "$root/compose.env" "$@"
}

ROOT_A="" ROOT_B=""
PROJECT_A="$RUN-a"
PROJECT_B="$RUN-b"

stop_writer() {
    if [[ -n "$WRITER_PID" ]]; then
        touch "$WRITER_STOP"
        wait "$WRITER_PID" 2>/dev/null || true
        WRITER_PID=""
    fi
}

cleanup() {
    stop_writer
    if [[ "${KAUPPA_SMOKE_KEEP:-0}" == "1" ]]; then
        log "kept: projects $PROJECT_A, $PROJECT_B; files in $SMOKE_TMP"
        return
    fi
    [[ -n "$ROOT_A" ]] && stack "$ROOT_A" "$PROJECT_A" down -v --remove-orphans >/dev/null 2>&1 || true
    [[ -n "$ROOT_B" ]] && stack "$ROOT_B" "$PROJECT_B" down -v --remove-orphans >/dev/null 2>&1 || true
    remove_tree_as_root "$IMAGE" "$SMOKE_TMP"
}
trap cleanup EXIT

# use_ca ROOT PROJECT: trust this stack's Caddy internal CA.
use_ca() {
    local root=$1 project=$2 ca="$SMOKE_TMP/$2-root.crt"
    for _ in $(seq 1 30); do
        if stack "$root" "$project" exec -T caddy cat /data/caddy/pki/authorities/local/root.crt >"$ca" 2>/dev/null \
            && [[ -s "$ca" ]]; then
            break
        fi
        sleep 1
    done
    [[ -s "$ca" ]] || die "Caddy's internal CA root not found ($project)"
    CURL_OPTS=(--cacert "$ca" --resolve "$DOMAIN:$HTTPS_PORT:127.0.0.1")
}

# The writer: numbered lists, one at a time; a number is logged only after
# the app acknowledged it (303).
writer() {
    local n=0 status
    while [[ ! -e "$WRITER_STOP" ]]; do
        n=$((n + 1))
        status=$(curl -sS "${CURL_OPTS[@]}" --max-time 10 -o /dev/null -w '%{http_code}' \
            -X POST -H "Origin: $ORIGIN" -b "$SMOKE_TMP/writer.cookies" \
            --data-urlencode "name=drill-w-$n" "$BASE/lists" 2>/dev/null || echo err)
        if [[ "$status" == 303 ]]; then
            echo "$n" >>"$WRITER_LOG"
        else
            echo "writer: list $n got $status" >&2
            return 1
        fi
    done
}

acked() { [[ -s "$WRITER_LOG" ]] && tail -1 "$WRITER_LOG" || echo 0; }

# ============================================================ stack A ====
section "build and start stack A ($PROJECT_A)"
if [[ -z "${KAUPPA_SMOKE_IMAGE:-}" ]]; then
    build_app_image "$IMAGE" "drill"
fi
ROOT_A=$(new_root a)
stack "$ROOT_A" "$PROJECT_A" build backup >&2
stack "$ROOT_A" "$PROJECT_A" up -d --wait --wait-timeout 180 >&2 || {
    stack "$ROOT_A" "$PROJECT_A" logs --tail 50 >&2 || true
    die "stack A did not start"
}
use_ca "$ROOT_A" "$PROJECT_A"

crontab_ok() {
    stack "$ROOT_A" "$PROJECT_A" exec -T backup supercronic -test /etc/kauppa-backup/crontab >&2 \
        && stack "$ROOT_A" "$PROJECT_A" exec -T backup grep -q '^0 3,9,15,21 \* \* \* /usr/local/bin/kauppa-backup scheduled$' \
            /etc/kauppa-backup/crontab
}
check P5 "the backup job's scheduler is running with the 6-hourly crontab" -- crontab_ok

check P5 "register the drill user" -- \
    register "$BASE" "$ORIGIN" "$JAR" "$EMAIL" "$PASSWORD" stack "$ROOT_A" "$PROJECT_A" exec -T kauppa kauppa-web admin invite
check P5 "create list 'drill-before'" -- create_list "$BASE" "$ORIGIN" "$JAR" "drill-before"
cp "$JAR" "$SMOKE_TMP/writer.cookies"

section "P5: back up while the app is writing"
: >"$WRITER_LOG"
writer &
WRITER_PID=$!
for _ in $(seq 1 100); do
    [[ "$(acked)" -ge 20 ]] && break
    sleep 0.1
done
[[ "$(acked)" -ge 20 ]] || die "the writer did not get going (acked $(acked))"

BEFORE=$(acked)
backup_ok() { stack "$ROOT_A" "$PROJECT_A" exec -T backup kauppa-backup run drill >&2; }
check P5 "kauppa-backup run drill (VACUUM INTO + integrity_check) while writing" -- backup_ok
AFTER=$(acked)
sleep 1
stop_writer
TOTAL=$(acked)
info "writer: $BEFORE lists acknowledged before the backup, $AFTER after it, $TOTAL in total"

OBJECT=$(cd "$BACKUPS" && find drill -name 'kauppa-*.db.gz' | sed -n 1p)
one_object() { [[ -n "$OBJECT" && "$(find "$BACKUPS/drill" -name '*.db.gz' | wc -l)" -eq 1 ]]; }
check P5 "exactly one snapshot under drill/ ($OBJECT)" -- one_object

files_untouched() {
    # The live set is still in place: the job never moves or deletes it.
    [[ -f "$ROOT_A/db/kauppa.db" ]]
}
check P5 "the live database is still in place after the backup" -- files_untouched

stack "$ROOT_A" "$PROJECT_A" down -v >&2

# ============================================================ stack B ====
section "restore into a fresh stack B ($PROJECT_B)"
ROOT_B=$(new_root b)
stack "$ROOT_B" "$PROJECT_B" up -d --wait --wait-timeout 180 >&2 || die "stack B did not start"
use_ca "$ROOT_B" "$PROJECT_B"

stack "$ROOT_B" "$PROJECT_B" stop kauppa >&2
restore_ok() {
    stack "$ROOT_B" "$PROJECT_B" run --rm --no-deps backup restore --app-stopped "$OBJECT" >&2
}
check P5 "kauppa-backup restore --app-stopped $OBJECT" -- restore_ok

refuses_without_flag() {
    ! stack "$ROOT_B" "$PROJECT_B" run --rm --no-deps backup restore "$OBJECT" >/dev/null 2>&1
}
check P5 "restore refuses without --app-stopped" -- refuses_without_flag

aside_kept() { compgen -G "$ROOT_B/db/pre-restore-*/kauppa.db" >/dev/null; }
check P5 "the replaced database set was moved aside, not deleted" -- aside_kept

no_stale_sidecars() { [[ ! -e "$ROOT_B/db/kauppa.db-wal" && ! -e "$ROOT_B/db/kauppa.db-shm" ]]; }
check P5 "no stale -wal or -shm next to the restored kauppa.db" -- no_stale_sidecars

stack "$ROOT_B" "$PROJECT_B" start kauppa >&2
check P5 "the app starts on the restored database" -- \
    wait_healthy "$(stack "$ROOT_B" "$PROJECT_B" ps -q kauppa)" 90

JAR_B="$SMOKE_TMP/cookies-b"
check P5 "the drill user logs in on stack B" -- login "$BASE" "$ORIGIN" "$JAR_B" "$EMAIL" "$PASSWORD"

restored_lists() {
    http GET "$BASE/lists" -b "$JAR_B"
    status_is 200 || return 1
    body_has "drill-before" || return 1
    local nums count max
    nums=$(grep -oE 'drill-w-[0-9]+' "$HTTP_BODY" | sort -u | cut -d- -f3 | sort -n)
    count=$(grep -c . <<<"$nums" || true)
    max=$(tail -1 <<<"$nums")
    max=${max:-0}
    info "restored: drill-before + $count numbered lists (highest $max); acknowledged before backup: $BEFORE, after: $AFTER"
    [[ "$count" -eq "$max" ]] || { log "    numbered lists are not an unbroken 1..$max prefix"; return 1; }
    [[ "$max" -ge "$BEFORE" ]] || { log "    lists acknowledged before the backup are missing"; return 1; }
    [[ "$max" -le "$AFTER" ]] || { log "    lists acknowledged after the backup finished are present"; return 1; }
}
check P5 "restored lists = everything committed when the backup was taken" -- restored_lists

summary
