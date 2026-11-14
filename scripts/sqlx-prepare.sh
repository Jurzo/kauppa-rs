#!/usr/bin/env bash
# sqlx-prepare: regenerate, or check, the sqlx offline query cache `.sqlx/`
# (MB-0049, RR-0037 sub-answer 2).
#
#   scripts/sqlx-prepare.sh          rewrite .sqlx/ from the current queries
#   scripts/sqlx-prepare.sh --check  fail if .sqlx/ isn't what they give
#
# The query macros (`sqlx::query!` and friends) in kauppa-core read `.sqlx/`
# when no DATABASE_URL is set or SQLX_OFFLINE=true, so builds need no
# database. This script makes a throwaway SQLite database from
# migrations/*.sql (Python's sqlite3), rebuilds kauppa-core online against
# it with SQLX_OFFLINE_DIR pointing at a temporary folder, and compares or
# replaces `.sqlx/` with what the macros wrote. No sqlx-cli.
#
# Run it after changing a query in the macro-checked modules or adding a
# migration (RR-0037 constraint 1). CI runs --check.

set -euo pipefail

cd "$(dirname "$0")/.."

mode=write
case "${1:-}" in
    "") ;;
    --check) mode=check ;;
    *) echo "usage: $0 [--check]" >&2; exit 2 ;;
esac

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

python3 - "$tmp/schema.db" migrations/*.sql <<'PY'
import sqlite3
import sys

db = sqlite3.connect(sys.argv[1])
for path in sorted(sys.argv[2:]):
    with open(path, encoding="utf-8") as f:
        db.executescript(f.read())
db.commit()
db.close()
PY

mkdir "$tmp/sqlx"

# Recompile kauppa-core so every macro expands again with the online
# settings: cargo doesn't rebuild a crate when only the environment changes.
touch crates/kauppa-core/src/lib.rs
DATABASE_URL="sqlite://$tmp/schema.db" \
    SQLX_OFFLINE=false \
    SQLX_OFFLINE_DIR="$tmp/sqlx" \
    cargo check --quiet -p kauppa-core --lib
# Recompile once more offline, so the next build doesn't reuse artifacts
# made against the throwaway database.
touch crates/kauppa-core/src/lib.rs

if [[ "$mode" == check ]]; then
    # A missing .sqlx/ counts as empty (git keeps no empty folders).
    mkdir "$tmp/committed"
    if [[ -d .sqlx ]]; then
        cp -r .sqlx/. "$tmp/committed/"
    fi
    if ! diff -r "$tmp/committed" "$tmp/sqlx" >"$tmp/diff"; then
        echo "sqlx-prepare: .sqlx/ is stale; run scripts/sqlx-prepare.sh and commit .sqlx/" >&2
        if [[ -s "$tmp/diff" ]]; then
            head -n 40 "$tmp/diff" >&2
        fi
        exit 1
    fi
    echo "sqlx-prepare: .sqlx/ is current ($(find .sqlx -name 'query-*.json' | wc -l) queries)"
else
    rm -rf .sqlx
    cp -r "$tmp/sqlx" .sqlx
    echo "sqlx-prepare: wrote .sqlx/ ($(find .sqlx -name 'query-*.json' | wc -l) queries)"
fi
