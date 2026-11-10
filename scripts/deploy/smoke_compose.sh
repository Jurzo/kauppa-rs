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
#   MB-0028 P1 (RR-0013 constraint 8), local mode only: compose.yaml sets
#       KAUPPA_TRUSTED_PROXIES to exactly the `internal` network's subnet and
#       the Caddyfile sets no trusted_proxies; failed logins through Caddy
#       with spoofed X-Forwarded-For headers are limited under the real
#       client's key, not the spoofed one; the app logs no "forwarded-for
#       missing from a trusted proxy" WARN. Live mode sends no failing
#       logins: they would lock the smoke account out of the real site.
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

# ------------------------------------ MB-0028 P1: static, no Docker needed ----
section "MB-0028 P1: KAUPPA_TRUSTED_PROXIES and the Caddyfile (static)"

# compose.yaml's uncommented KAUPPA_TRUSTED_PROXIES lines, value only.
compose_trusted_proxies() {
    { grep -E '^[[:space:]]+KAUPPA_TRUSTED_PROXIES:' deploy/compose.yaml || true; } \
        | sed -E 's/^[[:space:]]*KAUPPA_TRUSTED_PROXIES:[[:space:]]*//; s/[[:space:]]+#.*$//; s/[[:space:]]+$//' \
        | tr -d "\"'"
}

# The subnet(s) pinned under the top-level `networks:` -> `internal:` block.
compose_internal_subnet() {
    awk '
        /^[^[:space:]#]/ { top = $1; net = "" }
        top == "networks:" && /^  [^[:space:]#]/ { net = $1 }
        top == "networks:" && net == "internal:" && /^[[:space:]]+(-[[:space:]]+)?subnet:/ {
            sub(/^[[:space:]]+(-[[:space:]]+)?subnet:[[:space:]]*/, "")
            sub(/[[:space:]]+#.*$/, "")
            sub(/[[:space:]]+$/, "")
            print
        }
    ' deploy/compose.yaml | tr -d "\"'"
}

trusted_equals_subnet() {
    local trusted subnet
    trusted=$(compose_trusted_proxies)
    subnet=$(compose_internal_subnet)
    log "    KAUPPA_TRUSTED_PROXIES=[$(xargs <<<"$trusted")] internal subnet=[$(xargs <<<"$subnet")]"
    # One setting, one subnet, the same string: "exactly" (no wider range,
    # no second entry, no second definition).
    [[ -n "$subnet" && "$subnet" != *$'\n'* && "$trusted" == "$subnet" ]]
}
check P1 "compose.yaml: KAUPPA_TRUSTED_PROXIES equals the internal network's subnet" -- trusted_equals_subnet

no_caddy_trusted_proxies() {
    ! grep -vE '^[[:space:]]*#' deploy/Caddyfile | qgrep -E 'trusted_proxies'
}
check P1 "Caddyfile sets no trusted_proxies (Caddy overwrites X-Forwarded-For)" -- no_caddy_trusted_proxies

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
    ! grep -vE '^\s*#' deploy/Caddyfile | qgrep -iE 'header_up\s+Host'
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
        | qgrep -vE '^[[:space:]]*log[[:space:]]+default[[:space:]]*\{'
}
check P3 "Caddyfile has no access log (only the global log filter)" -- no_caddy_access_log

caddy_logs_clean() {
    ! compose logs --no-color caddy 2>&1 | qgrep -F "$QUERY_MARKER"
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
    ! compose logs --no-color caddy 2>&1 | qgrep -F "$QUERY_MARKER_502"
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

# ------------------------- MB-0028 P1: the limiter's key behind Caddy ----
section "MB-0028 P1: spoofed X-Forwarded-For through Caddy (RR-0013 constraint 8)"

# 172.30.53.0/24-style IPv4 containment, for the runtime check below.
ipv4_int() {
    local IFS=. a b c d
    read -r a b c d <<<"$1"
    echo $(((a << 24) | (b << 16) | (c << 8) | d))
}
ipv4_in_net() {
    local ip=$1 net=${2%/*} prefix=${2#*/} mask
    [[ "$ip" =~ ^[0-9]+(\.[0-9]+){3}$ && "$net" =~ ^[0-9]+(\.[0-9]+){3}$ && "$prefix" =~ ^[0-9]+$ ]] \
        || return 1
    mask=$(((0xFFFFFFFF << (32 - prefix)) & 0xFFFFFFFF))
    (((($(ipv4_int "$ip")) & mask) == (($(ipv4_int "$net")) & mask)))
}

# What the running stack does, beside the static check above: the app's
# setting equals the subnet Docker gave the `internal` network, and Caddy's
# address on it (the peer the app sees) is inside it.
runtime_trusted_matches() {
    local setting subnet caddy_ip
    setting=$(compose exec -T kauppa /bin/bash -c 'printf %s "${KAUPPA_TRUSTED_PROXIES:-}"')
    subnet=$(docker network inspect -f '{{range .IPAM.Config}}{{.Subnet}} {{end}}' "${PROJECT}_internal" | xargs)
    caddy_ip=$(docker inspect -f "{{(index .NetworkSettings.Networks \"${PROJECT}_internal\").IPAddress}}" \
        "$(compose ps -q caddy)")
    log "    app KAUPPA_TRUSTED_PROXIES=[$setting] network subnet=[$subnet] caddy=[$caddy_ip]"
    [[ -n "$setting" && "$setting" == "$subnet" ]] && ipv4_in_net "$caddy_ip" "$subnet"
}
check P1 "the app trusts exactly the internal network, and Caddy is on it" -- runtime_trusted_matches

# The limits (crates/kauppa-web/src/routes/auth/limiter/mod.rs): failed
# logins per client 10 and per account 5, both per 900 s fixed window. A
# login is checked (client, then account) before any work; a throttled
# request answers 429 and is NOT counted; only a 401 counts, against both
# the client and the account. The limiter is in memory, and `compose start
# kauppa` just above started a fresh process, so every window is empty and
# the counts below are exact. Every request carries a different spoofed
# X-Forwarded-For, each in its own /24 (the log shows keys as /24s).
spoofed_login() {
    http POST "$BASE/auth/login" -H "Origin: $ORIGIN" -H "X-Forwarded-For: $2" \
        --data-urlencode "email=$1" --data-urlencode "password=wrong-password-$(rand_id)"
}

# (a) The brief's burst: 11 failed logins for one account (no such user;
# known and unknown emails are keyed alike). The account limit answers
# from the 6th on: 401 x5, then 429 x6. This holds whatever the client key
# is, so it shows the spoofed headers don't get around the account limit,
# not yet what the client key is; (b) and the log check show that.
BURST_ACCOUNT="p1-burst-$PROJECT@example.test"
BURST_A=""
for i in $(seq 1 11); do
    spoofed_login "$BURST_ACCOUNT" "198.18.$i.7"
    BURST_A+="$HTTP_STATUS "
done
BURST_A=${BURST_A% }
burst_a_ok() {
    [[ "$BURST_A" == "401 401 401 401 401 429 429 429 429 429 429" ]] \
        || { log "    got: $BURST_A"; return 1; }
}
check P1 "11 failed logins, one account, 11 spoofed X-Forwarded-For: 401 x5, then 429 (the 11th is 429)" -- burst_a_ok
last_429_ok() {
    status_is 429 && header_has '^retry-after: *[0-9]' && body_has "Too many attempts. Try again in"
}
check P1 "the 429 carries Retry-After and the 'Too many attempts' message" -- last_429_ok

# (b) The client key: one failed login each for 6 fresh accounts, again
# each with its own spoofed header. (a) counted 5 failures (its 401s)
# against this client, so 5 more fill the client window to 10 and the 6th
# is 429. No account here has more than one failure, so only the per-client
# limit can answer 429: if a spoofed header changed the key, each request
# would start a fresh client window and all 6 would be 401.
BURST_B=""
for i in $(seq 1 6); do
    spoofed_login "p1-client-$i-$PROJECT@example.test" "198.18.$((100 + i)).7"
    BURST_B+="$HTTP_STATUS "
done
BURST_B=${BURST_B% }
burst_b_ok() {
    [[ "$BURST_B" == "401 401 401 401 401 429" ]] || { log "    got: $BURST_B"; return 1; }
}
check P1 "6 more accounts, new spoofed headers: the per-client limit answers 429 at the 11th failure" -- burst_b_ok

sleep 1
APP_LOGS=$(compose logs --no-color kauppa 2>&1 || true)

# The throttle WARN ("auth attempt throttled", limiter/mod.rs) has the
# limiter and key kind; release builds log JSON with the request span, whose
# `http.client_ip` is the limiter's client key cut to /24 (telemetry.rs).
# All 7 throttled requests above must carry one key: the address Caddy saw
# (what the echo upstream got in X-Forwarded-For), not a spoofed 198.18.x
# and not Caddy's own 172.30.53.x (that would mean Caddy isn't trusted, and
# everyone shares Caddy's bucket).
THROTTLE_LINES=$(grep -F 'auth attempt throttled' <<<"$APP_LOGS" || true)
CADDY_SAW=$(sed -nE 's/^xff=\[([^],]*)\].*/\1/p' <<<"$ECHOED")
throttle_logged() {
    qgrep -F '"key_kind":"account"' <<<"$THROTTLE_LINES" \
        && qgrep -F '"key_kind":"client"' <<<"$THROTTLE_LINES" \
        || { log "    throttle lines: ${THROTTLE_LINES:-none}"; return 1; }
}
check P1 "the app logged both throttles (key_kind account and client)" -- throttle_logged

throttle_key_is_real_client() {
    local keys want=""
    keys=$({ grep -oE '"http\.client_ip":"[^"]*"' <<<"$THROTTLE_LINES" || true; } \
        | sed -E 's/.*:"([^"]*)"/\1/' | sort -u)
    [[ "$CADDY_SAW" =~ ^[0-9]+(\.[0-9]+){3}$ ]] && want="${CADDY_SAW%.*}.0/24"
    log "    throttled client key(s): [$(xargs <<<"$keys")]; Caddy saw [$CADDY_SAW]"
    [[ -n "$keys" && "$keys" != *$'\n'* ]] || { log "    want exactly one key"; return 1; }
    [[ "$keys" != 198.18.* ]] || { log "    the key is a spoofed address"; return 1; }
    [[ "$keys" != "$(compose_internal_subnet)" ]] || { log "    the key is Caddy's own address"; return 1; }
    [[ -z "$want" || "$keys" == "$want" ]] || { log "    want $want"; return 1; }
}
check P1 "the limiter keyed every throttled request by the real client, not the spoofed header" -- throttle_key_is_real_client

# The shared-bucket fallback (README §3.9): logged once per process when a
# trusted peer sends no usable X-Forwarded-For. These logs span both app
# processes of this container (before and after the stop/start above).
no_fallback_warn() {
    [[ -n "$APP_LOGS" ]] || { log "    no app logs"; return 1; }
    ! qgrep -F 'forwarded-for missing from a trusted proxy' <<<"$APP_LOGS"
}
check P1 "the app logged no 'forwarded-for missing from a trusted proxy' WARN" -- no_fallback_warn

# ------------------------------------------------- P2: down and up again ----
section "P2: docker compose down, up: the data stays on the volume"
compose down >&2
compose up -d --wait --wait-timeout 180 kauppa caddy >&2 || die "the stack did not restart"
JAR2="$SMOKE_TMP/cookies2"
check P2 "after down/up, the user logs in" -- login "$BASE" "$ORIGIN" "$JAR2" "$EMAIL" "$PASSWORD"
check P2 "and sees their list" -- lists_page_has "$BASE" "$JAR2" "$LIST"

summary
