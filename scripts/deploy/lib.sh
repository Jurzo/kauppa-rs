#!/usr/bin/env bash
# Shared helpers for scripts/deploy/*.sh (MB-0022). Sourced, not run.
#
# The checks drive the app with curl the way a browser would: every POST
# carries a same-origin `Origin` header (RR-0010).

# shellcheck disable=SC2034 # variables here are used by the sourcing scripts

FAILURES=0
CHECKS=0

log() { printf '%s\n' "$*" >&2; }

# qgrep ARGS...: `grep -q ARGS` that reads all of stdin first. A plain
# `producer | grep -q` lets grep exit at the first match, the producer then
# dies of SIGPIPE, and under `set -o pipefail` the pipeline fails even though
# grep matched (and `! producer | grep -q` wrongly passes).
qgrep() {
    local input
    input=$(cat)
    grep -q "$@" <<<"$input"
}
section() { printf '\n== %s\n' "$*" >&2; }
die() {
    printf 'FATAL: %s\n' "$*" >&2
    exit 2
}

# check ID DESCRIPTION -- COMMAND...: runs COMMAND, records PASS or FAIL.
check() {
    local id=$1 desc=$2
    shift 2
    [[ "${1:-}" == "--" ]] && shift
    CHECKS=$((CHECKS + 1))
    if "$@"; then
        printf 'PASS %-4s %s\n' "$id" "$desc"
    else
        printf 'FAIL %-4s %s\n' "$id" "$desc"
        FAILURES=$((FAILURES + 1))
    fi
}

# info LINE: an observation that is not a pass/fail check.
info() { printf 'INFO      %s\n' "$*"; }

summary() {
    printf '\n%d checks, %d failed\n' "$CHECKS" "$FAILURES"
    [[ "$FAILURES" -eq 0 ]]
}

require_cmd() {
    local c
    for c in "$@"; do
        command -v "$c" >/dev/null 2>&1 || die "'$c' is required but not installed"
    done
}

# A 64-byte random session secret (base64 characters only).
rand_secret() {
    head -c 48 /dev/urandom | base64 | tr -d '\n' | cut -c1-64
}

rand_id() {
    head -c 6 /dev/urandom | od -An -tx1 | tr -d ' \n'
}

# --- HTTP ------------------------------------------------------------------
#
# CURL_OPTS (array) holds extra options for every request, e.g. --cacert and
# --resolve for the local Caddy stack.
CURL_OPTS=()
HTTP_STATUS=""
HTTP_HEADERS=""
HTTP_BODY=""
HTTP_LOCATION=""

# http METHOD URL [curl args...]: sets HTTP_STATUS, HTTP_HEADERS (file),
# HTTP_BODY (file) and HTTP_LOCATION. Never follows redirects.
http() {
    local method=$1 url=$2
    shift 2
    : "${SMOKE_TMP:?SMOKE_TMP must be set}"
    HTTP_HEADERS="$SMOKE_TMP/last.headers"
    HTTP_BODY="$SMOKE_TMP/last.body"
    HTTP_STATUS=$(curl -sS -X "$method" ${CURL_OPTS[@]+"${CURL_OPTS[@]}"} --max-time 20 \
        -D "$HTTP_HEADERS" -o "$HTTP_BODY" -w '%{http_code}' "$@" "$url" 2>"$SMOKE_TMP/last.err") \
        || HTTP_STATUS="curl-error:$(tr -d '\n' <"$SMOKE_TMP/last.err")"
    HTTP_LOCATION=$(grep -i '^location:' "$HTTP_HEADERS" 2>/dev/null | head -1 | cut -d' ' -f2- | tr -d '\r' || true)
}

status_is() { [[ "$HTTP_STATUS" == "$1" ]] || { log "    got status $HTTP_STATUS, want $1"; return 1; }; }
location_starts() { [[ "$HTTP_LOCATION" == "$1"* ]] || { log "    got Location '$HTTP_LOCATION', want '$1...'"; return 1; }; }
body_has() { grep -qF -- "$1" "$HTTP_BODY" || { log "    body lacks '$1'"; return 1; }; }
header_has() { grep -qi -- "$1" "$HTTP_HEADERS" || { log "    headers lack '$1'"; return 1; }; }

# The session cookie's Set-Cookie line from the last response.
session_cookie_line() {
    grep -i '^set-cookie: kauppa_sid=' "$HTTP_HEADERS" | head -1 | tr -d '\r'
}

cookie_flags_ok() {
    local line
    line=$(session_cookie_line)
    [[ -n "$line" ]] || { log "    no kauppa_sid Set-Cookie"; return 1; }
    local ok=0
    grep -qi '; *Secure' <<<"$line" || { log "    cookie lacks Secure: $line"; ok=1; }
    grep -qi '; *HttpOnly' <<<"$line" || { log "    cookie lacks HttpOnly: $line"; ok=1; }
    grep -qi '; *SameSite=Lax' <<<"$line" || { log "    cookie lacks SameSite=Lax: $line"; ok=1; }
    return "$ok"
}

# register BASE ORIGIN JAR EMAIL PASSWORD INVITE_CMD...
#
# Registers through the form. Until MB-0023 registration is open. After it,
# a registration without an invitation is refused; then INVITE_CMD (e.g.
# `docker exec <ctr> kauppa-web admin invite`, RR-0012) prints a link with
# `?invite=<token>`, and the form is posted again with the hidden `invite`
# field. Succeeds on 303 to /lists.
register() {
    local base=$1 origin=$2 jar=$3 email=$4 password=$5
    shift 5
    http POST "$base/auth/register" -H "Origin: $origin" -c "$jar" -b "$jar" \
        --data-urlencode "email=$email" --data-urlencode "display_name=Smoke" \
        --data-urlencode "password=$password"
    if [[ "$HTTP_STATUS" == "303" && "$HTTP_LOCATION" == "/lists"* ]]; then
        return 0
    fi
    log "    open registration refused ($HTTP_STATUS); trying an invitation (MB-0023)"
    local out token
    out=$("$@" 2>/dev/null) || { log "    invite command failed: $*"; return 1; }
    token=$(grep -oE 'invite=[A-Za-z0-9_-]+' <<<"$out" | head -1 | cut -d= -f2)
    [[ -n "$token" ]] || { log "    no invite token in the command's output"; return 1; }
    http POST "$base/auth/register" -H "Origin: $origin" -c "$jar" -b "$jar" \
        --data-urlencode "email=$email" --data-urlencode "display_name=Smoke" \
        --data-urlencode "password=$password" --data-urlencode "invite=$token"
    [[ "$HTTP_STATUS" == "303" && "$HTTP_LOCATION" == "/lists"* ]] \
        || { log "    invited registration got $HTTP_STATUS"; return 1; }
}

login() {
    local base=$1 origin=$2 jar=$3 email=$4 password=$5
    http POST "$base/auth/login" -H "Origin: $origin" -c "$jar" -b "$jar" \
        --data-urlencode "email=$email" --data-urlencode "password=$password"
    [[ "$HTTP_STATUS" == "303" && "$HTTP_LOCATION" == "/lists"* ]] \
        || { log "    login got $HTTP_STATUS (Location '$HTTP_LOCATION')"; return 1; }
}

create_list() {
    local base=$1 origin=$2 jar=$3 name=$4
    http POST "$base/lists" -H "Origin: $origin" -b "$jar" --data-urlencode "name=$name"
    [[ "$HTTP_STATUS" == "303" && "$HTTP_LOCATION" == /lists/* ]] \
        || { log "    create list got $HTTP_STATUS (Location '$HTTP_LOCATION')"; return 1; }
}

lists_page_has() {
    local base=$1 jar=$2 name=$3
    http GET "$base/lists" -b "$jar"
    status_is 200 && body_has "$name"
}

# --- Docker ----------------------------------------------------------------

# wait_healthy CONTAINER [SECONDS]: waits for Docker's health status.
wait_healthy() {
    local ctr=$1 limit=${2:-90} i status
    for ((i = 0; i < limit; i++)); do
        status=$(docker inspect -f '{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' "$ctr" 2>/dev/null || echo gone)
        case "$status" in
            healthy) return 0 ;;
            gone) log "    container $ctr is gone"; return 1 ;;
        esac
        sleep 1
    done
    log "    $ctr not healthy after ${limit}s (status: $status)"
    docker logs --tail 30 "$ctr" >&2 || true
    return 1
}

# remove_tree_as_root IMAGE DIR: removes DIR, whose contents may belong to
# the container user, with a throwaway root container.
remove_tree_as_root() {
    local image=$1 dir=$2
    [[ -d "$dir" ]] || return 0
    docker run --rm --user 0 --entrypoint /bin/sh -v "$dir:/w" "$image" \
        -c 'rm -rf /w/* /w/.[!.]* 2>/dev/null; true' >/dev/null 2>&1 || true
    rm -rf "$dir" 2>/dev/null || log "could not remove $dir"
}

# build_app_image TAG REVISION: builds deploy/Dockerfile from the repo root.
build_app_image() {
    local tag=$1 revision=$2
    log "building $tag (deploy/Dockerfile) ..."
    docker build -f deploy/Dockerfile --build-arg "KAUPPA_REVISION=$revision" -t "$tag" . >&2
}
