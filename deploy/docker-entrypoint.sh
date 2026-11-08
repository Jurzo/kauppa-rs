#!/bin/bash
# Container entrypoint: refuse a missing, dev or short session secret, then
# exec the app (MB-0022 P2; RR-0014 Q3, constraint 2).
#
# config.rs falls back to the dev secret when KAUPPA_SESSION_SECRET is unset
# and rejects only secrets under 64 bytes; the dev value is 65 bytes. This
# check runs first, outside Rust; config.rs's own check stays as the second
# layer. Removing or bypassing it needs an RR.
#
# The secret's value is never printed.

set -euo pipefail

# Count bytes, not characters: actix-session needs 64 bytes of key material.
export LC_ALL=C

readonly DEV_SECRET='dev-only-dev-only-dev-only-dev-only-dev-only-dev-only-dev-only-01'
readonly MIN_BYTES=64

refuse() {
    printf 'docker-entrypoint: refusing to start: %s\n' "$1" >&2
    printf 'docker-entrypoint: generate one with: openssl rand -base64 64 | tr -d %s | cut -c1-64\n' "'\\n'" >&2
    exit 78 # EX_CONFIG
}

secret="${KAUPPA_SESSION_SECRET-}"

if [[ -z "$secret" ]]; then
    refuse "KAUPPA_SESSION_SECRET is not set (the app would fall back to the dev secret)"
fi
if [[ "$secret" == "$DEV_SECRET" ]]; then
    refuse "KAUPPA_SESSION_SECRET is the development secret from .env.example"
fi
if ((${#secret} < MIN_BYTES)); then
    refuse "KAUPPA_SESSION_SECRET is ${#secret} bytes; it must be at least ${MIN_BYTES}"
fi

unset secret
exec /usr/local/bin/kauppa-web "$@"
