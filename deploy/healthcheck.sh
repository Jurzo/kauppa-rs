#!/bin/bash
# Container health check: GET /healthz on the app's own port, healthy only on
# a 200. Uses bash's /dev/tcp so the runtime image needs no curl or wget.

set -euo pipefail

port="${KAUPPA_BIND##*:}"

exec 3<>"/dev/tcp/127.0.0.1/${port}"
printf 'GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n' >&3
IFS=' ' read -r -t 4 _version status _reason <&3
[[ "$status" == "200" ]]
