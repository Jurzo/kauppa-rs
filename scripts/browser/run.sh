#!/usr/bin/env bash
# run.sh: the browser-level test (MB-0047 P1, P2; RR-0041).
#
# Builds the release binary, creates an invitation in a throwaway database,
# serves the app on a free loopback port, runs the Playwright flows in
# tests/browser/ against it with the pinned Chromium, and stops the app.
# Nothing outside a temporary directory is written except the build and
# tests/browser/'s own output (test-results/, .auth/).
#
# Usage: scripts/browser/run.sh [playwright test args…]
#
# First time: (cd tests/browser && npm ci --ignore-scripts &&
#   npx playwright install chromium)   # add --with-deps on a fresh Linux host

set -euo pipefail

REPO=$(cd "$(dirname "$0")/../.." && pwd)
BROWSER_DIR="$REPO/tests/browser"

if [[ ! -x "$BROWSER_DIR/node_modules/.bin/playwright" ]]; then
    echo "run.sh: tests/browser has no node_modules; first run:" >&2
    echo "  (cd tests/browser && npm ci --ignore-scripts && npx playwright install chromium)" >&2
    exit 2
fi

cargo build --release --locked -p kauppa-web --manifest-path "$REPO/Cargo.toml"
BIN="$REPO/target/release/kauppa-web"

TMP=$(mktemp -d)
APP_PID=
cleanup() {
    if [[ -n "$APP_PID" ]] && kill -0 "$APP_PID" 2>/dev/null; then
        kill "$APP_PID"
        wait "$APP_PID" 2>/dev/null || true
    fi
    rm -rf "$TMP"
}
trap cleanup EXIT

# A free loopback port, picked by the OS.
PORT=$(python3 -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')

# Only these settings: the app runs from $TMP, so no .env is read (dotenvy
# looks in the working directory and its parents).
unset KAUPPA_PUBLIC_ORIGIN KAUPPA_TRUSTED_PROXIES
export KAUPPA_BIND="127.0.0.1:$PORT"
export KAUPPA_DATABASE_URL="sqlite://$TMP/kauppa.db"
export KAUPPA_COOKIE_SECURE=false
export KAUPPA_LOG_LEVEL="${KAUPPA_LOG_LEVEL:-warn}"
# A fresh 64-character secret per run (RR-0026: loopback, not secure cookies).
KAUPPA_SESSION_SECRET=$(head -c 48 /dev/urandom | base64 | tr -d '\n')
export KAUPPA_SESSION_SECRET

cd "$TMP"

# `admin invite` prints the link once; with no public origin it is a path.
INVITE_PATH=$("$BIN" admin invite --note "browser test" | grep -oE '/auth/register\?invite=[A-Za-z0-9_-]+' | head -1)
if [[ -z "$INVITE_PATH" ]]; then
    echo "run.sh: kauppa-web admin invite printed no invitation link" >&2
    exit 1
fi

"$BIN" >"$TMP/app.log" 2>&1 &
APP_PID=$!

BASE_URL="http://127.0.0.1:$PORT"
for _ in $(seq 1 100); do
    if curl -fsS "$BASE_URL/healthz" >/dev/null 2>&1; then
        break
    fi
    if ! kill -0 "$APP_PID" 2>/dev/null; then
        echo "run.sh: the app exited during start-up:" >&2
        cat "$TMP/app.log" >&2
        exit 1
    fi
    sleep 0.1
done
curl -fsS "$BASE_URL/healthz" >/dev/null

cd "$BROWSER_DIR"
status=0
KAUPPA_BASE_URL="$BASE_URL" KAUPPA_INVITE_PATH="$INVITE_PATH" \
    ./node_modules/.bin/playwright test "$@" || status=$?

if [[ "$status" -ne 0 ]]; then
    echo "--- app log ($KAUPPA_LOG_LEVEL) ---" >&2
    cat "$TMP/app.log" >&2
fi
exit "$status"
