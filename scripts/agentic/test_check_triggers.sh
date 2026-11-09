#!/usr/bin/env bash
# test_check_triggers: check_triggers.py keeps a clone's history and reports
# the same changed files and trigger categories (MB-0033 P1, P2).
#
# Everything runs in throwaway repositories under a temporary directory: a
# base repo (the clones' `origin`) with `main` and a `feature` branch, a full
# clone and a `--depth=1` clone of `feature`. No fetch ever reaches this
# repository or GitHub. Never run check_triggers.py itself in a working copy
# to test it: before MB-0033 it made a full clone shallow.
#
# Usage: scripts/agentic/test_check_triggers.sh   (PYTHON=... to pick the
# interpreter; it needs PyYAML, like check_triggers.py)

set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
CHECK="$SCRIPT_DIR/check_triggers.py"
PYTHON="${PYTHON:-python3}"

# Don't let a caller's git environment point the throwaway repos elsewhere.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

g() {
    git -c user.name=test -c user.email=test@example.invalid \
        -c commit.gpgsign=false -c core.hooksPath=/dev/null "$@"
}

commit_file() { # repo path content message
    mkdir -p "$(dirname "$1/$2")"
    printf '%s\n' "$3" >"$1/$2"
    g -C "$1" add -- "$2"
    g -C "$1" commit -q -m "$4"
}

failures=0
pass() { echo "ok   $1"; }
fail() { echo "FAIL $1"; failures=$((failures + 1)); }

# --- Base repo -------------------------------------------------------------
# main: a few commits, then `feature` branches off with three commits
# (migrations/, routes/auth/, a non-trigger file); main then moves on with a
# trigger-path change of its own (Cargo.toml) that must NOT be reported.
BASE="$TMP/base"
g init -q -b main "$BASE"
commit_file "$BASE" README.md "kauppa" "m1"
commit_file "$BASE" crates/kauppa-web/src/lib.rs "// lib" "m2"
commit_file "$BASE" docs/a.md "a" "m3"
g -C "$BASE" switch -q -c feature
commit_file "$BASE" migrations/0099_x.sql "-- x" "f1: migration"
commit_file "$BASE" crates/kauppa-web/src/routes/auth/mod.rs "// auth" "f2: auth"
commit_file "$BASE" docs/b.md "b" "f3: docs"
g -C "$BASE" switch -q main
commit_file "$BASE" Cargo.toml "[workspace]" "m4: deps on main"

expected_files=$(g -C "$BASE" diff --name-only main...feature)
expected_triggers="['schema', 'security:auth']"

# run_check <clone> <label>: runs check_triggers.py in the clone with a PR
# body that references no RR, so it must fail and list the triggers.
run_check() {
    local clone=$1 label=$2 out rc=0
    out=$(cd "$clone" && PR_BASE=main PR_BODY="no resolution record here" \
        "$PYTHON" "$CHECK" 2>&1) || rc=$?
    echo "--- check_triggers.py output ($label, exit $rc):"
    printf '%s\n' "$out" | sed 's/^/    /'
    if [[ $rc -eq 1 ]] && grep -qF "triggers fired: $expected_triggers" <<<"$out"; then
        pass "P2 $label: exit 1, triggers fired: $expected_triggers"
    else
        fail "P2 $label: expected exit 1 and triggers fired: $expected_triggers"
    fi
}

# changed_files <clone>: the script's own changed-file list.
changed_files() {
    (cd "$1" && "$PYTHON" -c '
import sys
sys.path.insert(0, sys.argv[1])
import check_triggers
print("\n".join(check_triggers.changed_files("main")))
' "$SCRIPT_DIR") 2>&1 || true
}

check_files() { # clone label
    local got
    got=$(changed_files "$1")
    if [[ "$got" == "$expected_files" ]]; then
        pass "P2 $2: changed files = merge-base diff ($(tr '\n' ' ' <<<"$got"))"
    else
        fail "P2 $2: changed files differ"
        echo "    expected: $(tr '\n' ' ' <<<"$expected_files")"
        echo "    got:      $(tr '\n' ' ' <<<"$got")"
    fi
}

# --- Full clone ------------------------------------------------------------
FULL="$TMP/full"
g clone -q --branch feature "file://$BASE" "$FULL"
run_check "$FULL" "full clone"
if [[ "$(git -C "$FULL" rev-parse --is-shallow-repository)" == "false" ]] &&
    [[ ! -e "$FULL/.git/shallow" ]]; then
    pass "P1 full clone: still not shallow, no .git/shallow"
else
    fail "P1 full clone: became shallow (.git/shallow: $(cat "$FULL/.git/shallow" 2>/dev/null | tr '\n' ' '))"
fi
check_files "$FULL" "full clone"

# --- Shallow clone ---------------------------------------------------------
SHALLOW="$TMP/shallow"
g clone -q --depth=1 --branch feature "file://$BASE" "$SHALLOW"
run_check "$SHALLOW" "shallow clone"
if mb=$(git -C "$SHALLOW" merge-base origin/main HEAD 2>/dev/null) &&
    [[ "$mb" == "$(g -C "$BASE" merge-base main feature)" ]]; then
    pass "P1 shallow clone: merge base found ($mb)"
else
    fail "P1 shallow clone: merge base not found"
fi
check_files "$SHALLOW" "shallow clone"

if [[ $failures -gt 0 ]]; then
    echo "test_check_triggers: $failures failure(s)"
    exit 1
fi
echo "test_check_triggers: all checks passed"
