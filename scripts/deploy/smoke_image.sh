#!/usr/bin/env bash
# smoke_image.sh: MB-0022 P1 and P2 against the Docker image; MB-0032 P4.
#
#   P1  the image builds (multi-stage, release); runs as a non-root user;
#       contains the binary and no templates/, assets/ or migrations/
#       directory (embedded since MB-0024; MB-0032, RR-0020), source tree,
#       toolchain or .env; serves the login page, /assets/app.css and a
#       registered user's lists from a database on the /data volume.
#   P2  the database lives on the volume: after the container is replaced by
#       one from a rebuilt image, the user can still log in and see their
#       lists; the app refuses to start with no, the dev, or a short session
#       secret (container entrypoint, RR-0014).
#
# Needs Docker (with BuildKit) and curl. Usage, from anywhere:
#
#   scripts/deploy/smoke_image.sh
#
# Environment:
#   KAUPPA_SMOKE_IMAGE  test this image instead of building deploy/Dockerfile
#   KAUPPA_SMOKE_PORT   host port for the app (default 18081, on 127.0.0.1)
#   KAUPPA_SMOKE_KEEP=1 keep the container and volume for inspection

set -euo pipefail
cd "$(dirname "$0")/../.."
# shellcheck source=scripts/deploy/lib.sh
source scripts/deploy/lib.sh

require_cmd docker curl

PORT=${KAUPPA_SMOKE_PORT:-18081}
RUN_ID="kauppa-smoke-$(rand_id)"
CTR="$RUN_ID-app"
VOLUME="$RUN_ID-data"
IMAGE=${KAUPPA_SMOKE_IMAGE:-kauppa-rs:smoke}
REBUILT="kauppa-rs:$RUN_ID-rebuilt"
SMOKE_TMP=$(mktemp -d)
export SMOKE_TMP

DEV_SECRET='dev-only-dev-only-dev-only-dev-only-dev-only-dev-only-dev-only-01'
SECRET=$(rand_secret)
BASE="http://127.0.0.1:$PORT"
ORIGIN="$BASE"
EMAIL="smoke-$RUN_ID@example.test"
PASSWORD="smoke-password-$(rand_id)-long-enough"
LIST="Smoke list $RUN_ID"
JAR="$SMOKE_TMP/cookies"

cleanup() {
    if [[ "${KAUPPA_SMOKE_KEEP:-0}" == "1" ]]; then
        log "kept: container $CTR, volume $VOLUME, image $REBUILT"
    else
        docker rm -f "$CTR" >/dev/null 2>&1 || true
        docker volume rm "$VOLUME" >/dev/null 2>&1 || true
        docker image rm "$REBUILT" >/dev/null 2>&1 || true
    fi
    rm -rf "$SMOKE_TMP"
}
trap cleanup EXIT

run_app() {
    local image=$1
    docker run -d --name "$CTR" \
        -p "127.0.0.1:$PORT:8080" \
        -v "$VOLUME:/data" \
        -e "KAUPPA_SESSION_SECRET=$SECRET" \
        -e "KAUPPA_PUBLIC_ORIGIN=$ORIGIN" \
        -e "KAUPPA_COOKIE_SECURE=false" \
        --health-interval 2s --health-start-period 30s \
        "$image" >/dev/null
}

# ---------------------------------------------------------------- build ----
section "build"
if [[ -n "${KAUPPA_SMOKE_IMAGE:-}" ]]; then
    log "using prebuilt image $IMAGE"
else
    build_app_image "$IMAGE" "smoke-1"
fi
docker image inspect "$IMAGE" >/dev/null || die "image $IMAGE not found"
info "image size: $(docker image inspect -f '{{.Size}}' "$IMAGE") bytes"

# ------------------------------------------------------------ P1: image ----
section "P1: image contents and user"

image_user() { docker image inspect -f '{{.Config.User}}' "$IMAGE"; }
user_not_root() {
    local u
    u=$(image_user)
    log "    image user: '$u'"
    [[ -n "$u" && "$u" != "root" && "$u" != 0 && "$u" != 0:* && "$u" != root:* ]]
}
check P1 "image declares a non-root USER" -- user_not_root

runtime_uid_not_zero() {
    local uid
    uid=$(docker run --rm --entrypoint /usr/bin/id "$IMAGE" -u)
    log "    runtime uid: $uid"
    [[ "$uid" =~ ^[0-9]+$ && "$uid" -ne 0 ]]
}
check P1 "container process runs as a non-zero uid" -- runtime_uid_not_zero

has_healthcheck() {
    docker image inspect -f '{{json .Config.Healthcheck}}' "$IMAGE" | qgrep '/usr/local/bin/healthcheck'
}
check P1 "image has a HEALTHCHECK (GET /healthz)" -- has_healthcheck

has_data_volume() {
    docker image inspect -f '{{json .Config.Volumes}}' "$IMAGE" | qgrep '"/data"'
}
check P1 "image declares the /data volume" -- has_data_volume

# Inspect the filesystem as root so nothing is hidden by permissions.
contents_ok() {
    docker run --rm --user 0 --entrypoint /bin/bash "$IMAGE" -c '
        rc=0
        need() { [ -e "$1" ] || { echo "    missing: $1" >&2; rc=1; }; }
        absent() { [ ! -e "$1" ] || { echo "    must not exist: $1" >&2; rc=1; }; }
        need /usr/local/bin/kauppa-web
        [ -x /usr/local/bin/kauppa-web ] || { echo "    binary not executable" >&2; rc=1; }
        for p in /app/templates /app/assets /app/migrations \
                 /app/crates /app/Cargo.toml /app/Cargo.lock /app/target /build \
                 /usr/local/cargo /usr/local/rustup /root/.cargo /app/.git /app/.env; do
            absent "$p"
        done
        for c in cargo rustc rustup gcc cc; do
            if command -v "$c" >/dev/null 2>&1; then echo "    toolchain on PATH: $c" >&2; rc=1; fi
        done
        found=$(find / -xdev \( -name "*.rs" -o -name ".env" -o -name ".env.*" -o -name "Cargo.toml" \) \
                -not -path "/proc/*" -not -path "/sys/*" 2>/dev/null | sed -n 1,5p)
        if [ -n "$found" ]; then echo "    source or env files in the image: $found" >&2; rc=1; fi
        exit $rc
    '
}
check P1 "image has the binary; no templates/, assets/, migrations/, source, toolchain or .env" -- contents_ok

app_files_read_only() {
    docker run --rm --entrypoint /bin/bash "$IMAGE" -c \
        '[ ! -w /usr/local/bin/kauppa-web ]'
}
check P1 "the binary is read-only to the app user" -- app_files_read_only

# ------------------------------------------------- P2: secret refusals ----
section "P2: the entrypoint refuses bad session secrets"

# refused DESCRIPTION [docker run args...]: the container must exit non-zero
# on its own, with the entrypoint's message, without starting the server.
refused() {
    local out rc=0
    out=$(timeout 60 docker run --rm "$@" "$IMAGE" 2>&1) || rc=$?
    if [[ "$rc" -eq 0 || "$rc" -eq 124 ]]; then
        log "    exit $rc (want a non-zero exit from the entrypoint)"
        return 1
    fi
    grep -q 'refusing to start' <<<"$out" || { log "    output: $out"; return 1; }
    log "    exit $rc: $(grep 'refusing to start' <<<"$out")"
}
check P2 "no KAUPPA_SESSION_SECRET: refused" -- refused
check P2 "the .env.example dev secret: refused" -- refused -e "KAUPPA_SESSION_SECRET=$DEV_SECRET"
check P2 "a 63-byte secret: refused" -- refused -e "KAUPPA_SESSION_SECRET=${SECRET:0:63}"

second_layer() {
    local rc=0 out
    out=$(timeout 60 docker run --rm --entrypoint /usr/local/bin/kauppa-web \
        -e "KAUPPA_SESSION_SECRET=${SECRET:0:63}" "$IMAGE" 2>&1) || rc=$?
    [[ "$rc" -ne 0 && "$rc" -ne 124 ]] && grep -q 'at least 64 bytes' <<<"$out" \
        || { log "    exit $rc: $out"; return 1; }
}
check P2 "config.rs still refuses a short secret with the entrypoint bypassed" -- second_layer

# -------------------------------------------------------- P1: serving ----
section "P1: serving from the /data volume"
docker volume create "$VOLUME" >/dev/null
run_app "$IMAGE"
check P1 "container becomes healthy (HEALTHCHECK on /healthz)" -- wait_healthy "$CTR" 90

healthz_ok() { http GET "$BASE/healthz" && status_is 200 && body_has ok; }
check P1 "GET /healthz is 200" -- healthz_ok

login_page_ok() { http GET "$BASE/auth/login" && status_is 200 && body_has '<form'; }
check P1 "GET /auth/login serves the login page" -- login_page_ok

css_ok() { http GET "$BASE/assets/app.css" && status_is 200 && header_has '^content-type: text/css'; }
check P1 "GET /assets/app.css is served as text/css" -- css_ok

check P1 "register a user (same-origin POST)" -- \
    register "$BASE" "$ORIGIN" "$JAR" "$EMAIL" "$PASSWORD" docker exec "$CTR" kauppa-web admin invite
check P1 "create a list" -- create_list "$BASE" "$ORIGIN" "$JAR" "$LIST"
check P1 "GET /lists shows the user's list" -- lists_page_has "$BASE" "$JAR" "$LIST"

db_on_volume() {
    docker run --rm --entrypoint /bin/ls -v "$VOLUME:/data" "$IMAGE" /data | qgrep -x 'kauppa.db'
}
check P1 "kauppa.db is on the /data volume" -- db_on_volume

migrations_ran() { docker logs "$CTR" 2>&1 | qgrep -i 'listening'; }
check P2 "migrations ran on start and the server is listening" -- migrations_ran

# ------------------------------------------------- P2: replace container ----
section "P2: replace the container with a rebuilt image, same volume"
docker rm -f "$CTR" >/dev/null
# A new image (new id) from the tested one: only a label differs, so this
# needs no second compile and works for a prebuilt KAUPPA_SMOKE_IMAGE too.
printf 'FROM %s\nLABEL org.opencontainers.image.revision="%s"\n' "$IMAGE" "$RUN_ID-rebuilt" \
    | docker build -q -t "$REBUILT" - >/dev/null
new_image_id() {
    [[ "$(docker image inspect -f '{{.Id}}' "$REBUILT")" != "$(docker image inspect -f '{{.Id}}' "$IMAGE")" ]]
}
check P2 "rebuilt image has a new id" -- new_image_id
run_app "$REBUILT"
check P2 "new container becomes healthy" -- wait_healthy "$CTR" 90

JAR2="$SMOKE_TMP/cookies2"
check P2 "the earlier user logs in (fresh session)" -- login "$BASE" "$ORIGIN" "$JAR2" "$EMAIL" "$PASSWORD"
check P2 "and sees the list created before the replacement" -- lists_page_has "$BASE" "$JAR2" "$LIST"

summary
