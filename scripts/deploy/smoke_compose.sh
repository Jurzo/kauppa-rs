#!/usr/bin/env bash
# smoke_compose.sh: MB-0022 P3 (and P2's down/up) against the Compose stack.
#
#   P3  Caddy is the only thing listening on public ports (80, 443); the app
#       is reachable only on the internal network. Caddy redirects HTTP to
#       HTTPS and passes Host through. With KAUPPA_COOKIE_SECURE=true and
#       KAUPPA_PUBLIC_ORIGIN set, a same-origin POST succeeds, a cross-site
#       POST gets 403, and the session cookie is Secure; HttpOnly;
#       SameSite=Lax. Caddy writes no access log containing query strings
#       (RR-0014 constraint 5). Also RR-0013's follow-up: a client-sent
#       X-Forwarded-For never reaches the app.
#
# Two modes:
#
#   scripts/deploy/smoke_compose.sh
#       Local: starts deploy/compose.yaml (app + Caddy) on a scratch data
#       directory, with Caddy's internal CA for a fake domain (kauppa.test),
#       on ports 18080/18443. Needs Docker and curl.
#
#   KAUPPA_SMOKE_EMAIL=... KAUPPA_SMOKE_PASSWORD=... \
#       scripts/deploy/smoke_compose.sh https://kauppa.example
#       Live (MB-0022 P7): the deployed site, with an existing account. Only
#       HTTP checks; run `docker compose ...` checks on the droplet by hand
#       (deploy/README.md).
#
# Environment (local mode):
#   KAUPPA_SMOKE_IMAGE    test this image instead of building deploy/Dockerfile
#   KAUPPA_SMOKE_HTTP_PORT / KAUPPA_SMOKE_HTTPS_PORT  (default 18080 / 18443)
#   KAUPPA_SMOKE_KEEP=1   leave the stack running

set -euo pipefail
cd "$(dirname "$0")/../.."
# shellcheck source=scripts/deploy/lib.sh
source scripts/deploy/lib.sh

require_cmd curl
SMOKE_TMP=$(mktemp -d)
export SMOKE_TMP
EVIL_ORIGIN="https://evil.example"
SPOOFED_IP="203.0.113.77"
QUERY_MARKER="smokequerytoken$(rand_id)"

# ============================================================== live mode ==
if [[ $# -ge 1 ]]; then
    BASE=${1%/}
    [[ "$BASE" == https://* ]] || die "the live URL must be https://..."
    : "${KAUPPA_SMOKE_EMAIL:?set KAUPPA_SMOKE_EMAIL to an existing account}"
    : "${KAUPPA_SMOKE_PASSWORD:?set KAUPPA_SMOKE_PASSWORD}"
    trap 'rm -rf "$SMOKE_TMP"' EXIT
    ORIGIN=$BASE
    HOST=${BASE#https://}
    HOST=${HOST%%/*}
    JAR="$SMOKE_TMP/cookies"

    section "P3 (live): $BASE"
    redirect_ok() {
        http GET "http://$HOST/auth/login"
        [[ "$HTTP_STATUS" == 301 || "$HTTP_STATUS" == 308 ]] || { log "    got $HTTP_STATUS"; return 1; }
        location_starts "https://$HOST/"
    }
    check P3 "HTTP redirects to HTTPS" -- redirect_ok

    healthz_ok() { http GET "$BASE/healthz" && status_is 200; }
    check P3 "GET /healthz over HTTPS is 200" -- healthz_ok

    app_port_closed() {
        ! curl -sS --max-time 5 -o /dev/null "http://$HOST:8080/healthz" 2>/dev/null
    }
    check P3 "the app port (8080) is not reachable from outside" -- app_port_closed

    cross_site_login() {
        http POST "$BASE/auth/login" -H "Origin: $EVIL_ORIGIN" \
            --data-urlencode "email=$KAUPPA_SMOKE_EMAIL" --data-urlencode "password=$KAUPPA_SMOKE_PASSWORD"
        status_is 403
    }
    check P3 "cross-site POST (Origin: $EVIL_ORIGIN) gets 403" -- cross_site_login

    check P3 "same-origin POST (login) succeeds with 303" -- \
        login "$BASE" "$ORIGIN" "$JAR" "$KAUPPA_SMOKE_EMAIL" "$KAUPPA_SMOKE_PASSWORD"
    check P3 "session cookie is Secure; HttpOnly; SameSite=Lax" -- cookie_flags_ok
    lists_ok() { http GET "$BASE/lists" -b "$JAR" && status_is 200; }
    check P3 "GET /lists with the session is 200" -- lists_ok

    logout_ok() { http POST "$BASE/auth/logout" -H "Origin: $ORIGIN" -b "$JAR" -c "$JAR" && status_is 303; }
    check P3 "same-origin logout succeeds" -- logout_ok

    info "on the droplet, also check: no ports but Caddy's are published (docker compose ps),"
    info "and no query strings in Caddy's logs (deploy/README.md, 'Checks after a deploy')."
    summary
    exit $?
fi

# ============================================================= local mode ==
require_cmd docker
HTTP_PORT=${KAUPPA_SMOKE_HTTP_PORT:-18080}
HTTPS_PORT=${KAUPPA_SMOKE_HTTPS_PORT:-18443}
DOMAIN=kauppa.test
PROJECT="kauppa-smoke-$(rand_id)"
IMAGE=${KAUPPA_SMOKE_IMAGE:-kauppa-rs:smoke}
ROOT="$SMOKE_TMP/root"
BASE="https://$DOMAIN:$HTTPS_PORT"
ORIGIN="$BASE"
EMAIL="smoke-$PROJECT@example.test"
PASSWORD="smoke-password-$(rand_id)-long-enough"
LIST="Compose smoke list $PROJECT"
JAR="$SMOKE_TMP/cookies"

mkdir -p "$ROOT/db" "$ROOT/secrets"
# The app runs as uid 10001; let it write the scratch db directory.
chmod 0777 "$ROOT/db"
printf 'KAUPPA_SESSION_SECRET=%s\n' "$(rand_secret)" >"$ROOT/secrets/app.env"
: >"$ROOT/secrets/backup.env"
chmod 0600 "$ROOT/secrets/app.env" "$ROOT/secrets/backup.env"

cat >"$SMOKE_TMP/compose.env" <<EOF
KAUPPA_IMAGE=$IMAGE
KAUPPA_DATA_ROOT=$ROOT
KAUPPA_DOMAIN=$DOMAIN
KAUPPA_PUBLIC_ORIGIN=$ORIGIN
KAUPPA_TLS=internal
KAUPPA_HTTP_PORT=$HTTP_PORT
KAUPPA_HTTPS_PORT=$HTTPS_PORT
KAUPPA_BACKUP_BUCKET=smoke-unused
EOF

compose() {
    docker compose -p "$PROJECT" -f deploy/compose.yaml --env-file "$SMOKE_TMP/compose.env" "$@"
}

cleanup() {
    if [[ "${KAUPPA_SMOKE_KEEP:-0}" == "1" ]]; then
        log "kept: compose project $PROJECT (data in $ROOT)"
        return
    fi
    docker rm -f "$PROJECT-echo" >/dev/null 2>&1 || true
    compose down -v --remove-orphans >/dev/null 2>&1 || true
    remove_tree_as_root "$IMAGE" "$SMOKE_TMP"
}
trap cleanup EXIT

section "build and start (project $PROJECT)"
if [[ -z "${KAUPPA_SMOKE_IMAGE:-}" ]]; then
    build_app_image "$IMAGE" "smoke-compose"
fi
compose config --quiet || die "compose config is invalid"
compose up -d --wait --wait-timeout 180 kauppa caddy >&2 || {
    compose logs --tail 50 >&2 || true
    die "the stack did not start"
}

# Caddy's local CA root, so curl verifies the internal certificate.
CA="$SMOKE_TMP/caddy-root.crt"
for _ in $(seq 1 30); do
    if compose exec -T caddy cat /data/caddy/pki/authorities/local/root.crt >"$CA" 2>/dev/null && [[ -s "$CA" ]]; then
        break
    fi
    sleep 1
done
[[ -s "$CA" ]] || die "Caddy's internal CA root not found"
CURL_OPTS=(--cacert "$CA" --resolve "$DOMAIN:$HTTPS_PORT:127.0.0.1" --resolve "$DOMAIN:$HTTP_PORT:127.0.0.1")

# ------------------------------------------------------------- ports ----
section "P3: only Caddy publishes ports; the app is internal"

published_ports() { docker port "$(compose ps -q "$1")" 2>/dev/null | sort; }

app_unpublished() {
    local p
    p=$(published_ports kauppa)
    [[ -z "$p" ]] || { log "    the app publishes: $p"; return 1; }
}
check P3 "the app container publishes no port" -- app_unpublished

caddy_ports_only() {
    local p bad
    p=$(published_ports caddy)
    log "    caddy publishes: $(tr '\n' ' ' <<<"$p")"
    bad=$(grep -vE '^(80|443)/tcp -> ' <<<"$p" || true)
    [[ -z "$bad" ]] && grep -q '^80/tcp' <<<"$p" && grep -q '^443/tcp' <<<"$p"
}
check P3 "Caddy publishes exactly 80/tcp and 443/tcp" -- caddy_ports_only

only_caddy_publishes() {
    local svc
    for svc in $(compose ps --services); do
        [[ "$svc" == caddy ]] && continue
        [[ -z "$(published_ports "$svc")" ]] || { log "    $svc publishes ports"; return 1; }
    done
}
check P3 "no other service in the stack publishes a port" -- only_caddy_publishes

internal_network() {
    [[ "$(docker network inspect -f '{{.Internal}}' "${PROJECT}_internal")" == "true" ]]
}
check P3 "the app's network is internal (no route out, not reachable from the host)" -- internal_network

app_only_internal() {
    local nets
    nets=$(docker inspect -f '{{range $k, $v := .NetworkSettings.Networks}}{{$k}} {{end}}' "$(compose ps -q kauppa)")
    [[ "$(xargs <<<"$nets")" == "${PROJECT}_internal" ]] || { log "    app networks: $nets"; return 1; }
}
check P3 "the app is attached only to the internal network" -- app_only_internal

# ------------------------------------------------------- HTTP and HTTPS ----
section "P3: HTTP to HTTPS, Host, cookies, cross-site check"

redirect_ok() {
    http GET "http://$DOMAIN:$HTTP_PORT/auth/login"
    [[ "$HTTP_STATUS" == 301 || "$HTTP_STATUS" == 308 ]] || { log "    got $HTTP_STATUS"; return 1; }
    location_starts "https://$DOMAIN"
}
check P3 "HTTP redirects to HTTPS" -- redirect_ok

healthz_ok() { http GET "$BASE/healthz" && status_is 200 && body_has ok; }
check P3 "GET /healthz through Caddy over HTTPS is 200" -- healthz_ok

no_host_rewrite() {
    ! grep -vE '^\s*#' deploy/Caddyfile | grep -qiE 'header_up\s+Host'
}
check P3 "Caddyfile passes Host through (no header_up Host)" -- no_host_rewrite

public_origin_set() {
    compose exec -T kauppa /bin/bash -c '[ "$KAUPPA_COOKIE_SECURE" = true ] && [ -n "$KAUPPA_PUBLIC_ORIGIN" ]'
}
check P3 "the app runs with KAUPPA_COOKIE_SECURE=true and KAUPPA_PUBLIC_ORIGIN set" -- public_origin_set

check P3 "same-origin POST: register" -- \
    register "$BASE" "$ORIGIN" "$JAR" "$EMAIL" "$PASSWORD" compose exec -T kauppa kauppa-web admin invite
check P3 "session cookie is Secure; HttpOnly; SameSite=Lax" -- cookie_flags_ok
check P3 "same-origin POST: create a list (303)" -- create_list "$BASE" "$ORIGIN" "$JAR" "$LIST"

cross_site_post() {
    http POST "$BASE/lists" -H "Origin: $EVIL_ORIGIN" -b "$JAR" --data-urlencode "name=cross-site $PROJECT"
    status_is 403
}
check P3 "cross-site POST (Origin: $EVIL_ORIGIN) gets 403" -- cross_site_post

fetch_site_post() {
    http POST "$BASE/lists" -H "Origin: $ORIGIN" -H "Sec-Fetch-Site: cross-site" -b "$JAR" \
        --data-urlencode "name=cross-site-2 $PROJECT"
    status_is 403
}
check P3 "POST with Sec-Fetch-Site: cross-site gets 403" -- fetch_site_post

not_created() {
    http GET "$BASE/lists" -b "$JAR"
    status_is 200 && ! grep -qF -e "cross-site $PROJECT" -e "cross-site-2 $PROJECT" "$HTTP_BODY"
}
check P3 "the cross-site POSTs created nothing" -- not_created

# ------------------------------------------------------------- logging ----
section "P3: logs (RR-0014 constraint 5; RR-0013 follow-up)"

# A query string (an invitation link's shape) that must never reach Caddy's
# logs during normal traffic.
http GET "$BASE/auth/register?invite=$QUERY_MARKER"
http POST "$BASE/auth/login" -H "Origin: $ORIGIN" \
    --data-urlencode "email=nobody-$PROJECT@example.test" --data-urlencode "password=wrong-password-123"
sleep 1

# The only `log` directive allowed is the global `log default` filter;
# a site-level `log` would be an access log (RR-0014 constraint 5).
no_caddy_access_log() {
    ! grep -vE '^[[:space:]]*#' deploy/Caddyfile \
        | grep -E '^[[:space:]]*log([[:space:]]|$|\{)' \
        | grep -qvE '^[[:space:]]*log[[:space:]]+default[[:space:]]*\{'
}
check P3 "Caddyfile has no access log (only the global log filter)" -- no_caddy_access_log

caddy_logs_clean() {
    ! compose logs --no-color caddy 2>&1 | grep -qF "$QUERY_MARKER"
}
check P3 "Caddy logs no query string for normal traffic" -- caddy_logs_clean

# ------------------------------------- what Caddy sends to the upstream ----
section "P3: what reaches the app (Host; RR-0013 X-Forwarded-For)"

# The app doesn't echo request headers, so for this part a throwaway echo
# server (the same pinned Caddy image) takes the app's place on the internal
# network, under the alias `kauppa`, and reports what Caddy forwarded.
CADDY_IMAGE=$(docker inspect -f '{{.Config.Image}}' "$(compose ps -q caddy)")
ECHO="$PROJECT-echo"
compose stop kauppa >&2

# Upstream down: Caddy answers 502 and writes an error log entry with the
# request URI. The global `log default` filter must strip its query string
# (RR-0014 constraint 5, owner-approved 2026-11-08).
QUERY_MARKER_502="smokeupstreamdown$(rand_id)"
http GET "$BASE/auth/register?invite=$QUERY_MARKER_502"
sleep 1
info "UPSTREAM-DOWN: the request got $HTTP_STATUS"
caddy_error_log_clean() {
    ! compose logs --no-color caddy 2>&1 | grep -qF "$QUERY_MARKER_502"
}
check P3 "Caddy's error log has no query string while the app is down" -- caddy_error_log_clean

docker run -d --rm --name "$ECHO" --network "${PROJECT}_internal" --network-alias kauppa \
    --entrypoint /bin/sh "$CADDY_IMAGE" -c \
    'printf "{\n\tadmin off\n\tauto_https off\n}\n:8080 {\n\trespond \"xff=[{header.X-Forwarded-For}] host=[{hostport}]\"\n}\n" >/tmp/Caddyfile && exec caddy run --adapter caddyfile --config /tmp/Caddyfile' \
    >/dev/null
for _ in $(seq 1 30); do
    http GET "$BASE/echo" -H "X-Forwarded-For: $SPOOFED_IP"
    [[ "$HTTP_STATUS" == 200 ]] && grep -q '^xff=' "$HTTP_BODY" && break
    sleep 1
done
ECHOED=$(cat "$HTTP_BODY" 2>/dev/null || true)
info "upstream saw: $ECHOED"

host_passed() { [[ "$ECHOED" == *"host=[$DOMAIN:$HTTPS_PORT]"* ]] || { log "    want host=[$DOMAIN:$HTTPS_PORT]"; return 1; }; }
check P3 "Caddy passes Host through unchanged" -- host_passed

xff_replaced() {
    [[ "$ECHOED" == xff=\[* && "$ECHOED" != *"$SPOOFED_IP"* && "$ECHOED" != "xff=[]"* ]] \
        || { log "    the client-sent X-Forwarded-For reached the upstream, or none was set"; return 1; }
}
check P3 "a client-sent X-Forwarded-For is replaced by the peer address (RR-0013)" -- xff_replaced

docker rm -f "$ECHO" >/dev/null 2>&1 || true
compose start kauppa >&2
check P3 "the app is back and healthy" -- wait_healthy "$(compose ps -q kauppa)" 90

# ------------------------------------------------- P2: down and up again ----
section "P2: docker compose down, up: the data stays on the volume"
compose down >&2
compose up -d --wait --wait-timeout 180 kauppa caddy >&2 || die "the stack did not restart"
JAR2="$SMOKE_TMP/cookies2"
check P2 "after down/up, the user logs in" -- login "$BASE" "$ORIGIN" "$JAR2" "$EMAIL" "$PASSWORD"
check P2 "and sees their list" -- lists_page_has "$BASE" "$JAR2" "$LIST"

summary
