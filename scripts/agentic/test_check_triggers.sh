#!/usr/bin/env bash
# test_check_triggers: check_triggers.py keeps a clone's history and reports
# the same changed files and trigger categories (MB-0033 P1, P2); the trigger
# paths are the same in check_triggers.py, the feature runbook's table and
# CODEOWNERS, CODEOWNERS names only allowed owners, and each trigger path
# trips (MB-0043 P1, P2; RR-0027).
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
# Importing check_triggers must not leave a __pycache__ in the repo.
export PYTHONDONTWRITEBYTECODE=1

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

# --- MB-0043: one list of trigger paths, real owners (RR-0027) --------------
# The rule: the set of trigger paths in check_triggers.py (TRIGGERS, plus
# CRATE_MANIFESTS for crates/*/Cargo.toml) EQUALS the paths in the feature
# runbook's "Escalation triggers" table, and is a SUBSET of the CODEOWNERS
# paths: CODEOWNERS may also own non-trigger paths (AGENTS.md, .github/,
# scripts/agentic/, .agentic/schemas/). Paths compare without a leading "/"
# and without a trailing "/" or "/**"; `{a,b}` expands. In a table cell, a
# token that doesn't start with a top-level directory (crates/, deploy/,
# migrations/, .agentic/, scripts/, .github/) is relative to the cell's
# first token's `crates/<crate>/src/` directory, if it has one (so
# `csp.rs` beside `crates/kauppa-web/src/csrf/` is
# crates/kauppa-web/src/csp.rs). Every CODEOWNERS owner is in OWNERS.
REPO=$(cd "$SCRIPT_DIR/../.." && pwd)

compare_lists() { # runbook codeowners: prints any drift, exit 1 on drift
    "$PYTHON" - "$SCRIPT_DIR" "$1" "$2" <<'PY'
import re
import sys

sys.path.insert(0, sys.argv[1])
import check_triggers as ct

ROOTS = ("crates/", "deploy/", "migrations/", ".agentic/", "scripts/", ".github/")


def norm(path):
    path = path.strip().lstrip("/")
    for suffix in ("/**", "/"):
        if path.endswith(suffix):
            path = path[: -len(suffix)]
    return path


def expand(token):
    m = re.search(r"\{([^}]*)\}", token)
    if not m:
        return [token]
    return [token[: m.start()] + alt + token[m.end():] for alt in m.group(1).split(",")]


def table_paths(text):
    section = text.split("## Escalation triggers", 1)[1].split("\n## ", 1)[0]
    found = set()
    for line in section.splitlines():
        if not line.startswith("| ") or line.startswith(("| Path", "| ---")):
            continue
        tokens = re.findall(r"`([^`]+)`", line.split("|")[1])
        m = re.match(r"crates/[^/]+/src/", tokens[0]) if tokens else None
        base = m.group(0) if m else ""
        for token in tokens:
            for path in expand(token):
                if base and not path.startswith(ROOTS):
                    path = base + path
                found.add(norm(path))
    return found


def codeowners(text):
    paths, owners = set(), set()
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        path, *who = line.split()
        paths.add(norm(path))
        owners.update(who)
    return paths, owners


triggers = {norm(prefix) for prefix, _ in ct.TRIGGERS} | {ct.CRATE_MANIFESTS}
table = table_paths(open(sys.argv[2], encoding="utf-8").read())
owned, owners = codeowners(open(sys.argv[3], encoding="utf-8").read())
drift = []
for path in sorted(triggers - table):
    drift.append(f"in check_triggers.py, not in the runbook table: {path}")
for path in sorted(table - triggers):
    drift.append(f"in the runbook table, not in check_triggers.py: {path}")
for path in sorted(triggers - owned):
    drift.append(f"a trigger path CODEOWNERS doesn't own: {path}")
for owner in sorted(owners - set(ct.OWNERS)):
    drift.append(f"a CODEOWNERS owner not in OWNERS {list(ct.OWNERS)}: {owner}")
print("\n".join(drift) if drift else f"{len(triggers)} trigger paths agree")
sys.exit(1 if drift else 0)
PY
}

expect_lists() { # want(pass|fail) label runbook codeowners [text the output must name]
    local out rc=0
    out=$(compare_lists "$3" "$4" 2>&1) || rc=$?
    printf '%s\n' "$out" | sed 's/^/    /'
    if [[ $1 == pass && $rc -eq 0 ]] ||
        [[ $1 == fail && $rc -eq 1 && ( -z "${5:-}" || "$out" == *"$5"* ) ]]; then
        pass "MB-0043 $2"
    else
        fail "MB-0043 $2 (exit $rc)"
    fi
}

RUNBOOK="$REPO/.agentic/runbooks/feature.md"
OWNERS_FILE="$REPO/.github/CODEOWNERS"
expect_lists pass "P2 the three lists agree, CODEOWNERS names allowed owners (P1)" \
    "$RUNBOOK" "$OWNERS_FILE"

# Drift is caught: a table row dropped, a trigger CODEOWNERS no longer owns,
# an owner that isn't allowed.
grep -v '^| Deployment:' "$RUNBOOK" >"$TMP/feature.md"
expect_lists fail "P2 drift: a runbook row removed" "$TMP/feature.md" "$OWNERS_FILE" \
    "not in the runbook table: deploy/Dockerfile"
grep -v '^/crates/kauppa-web/src/csp.rs' "$OWNERS_FILE" >"$TMP/CODEOWNERS.nocsp"
expect_lists fail "P2 drift: CODEOWNERS misses a trigger" "$RUNBOOK" "$TMP/CODEOWNERS.nocsp" \
    "doesn't own: crates/kauppa-web/src/csp.rs"
sed 's/@Jurzo/@juuso-kalliomaki/' "$OWNERS_FILE" >"$TMP/CODEOWNERS.wrong"
expect_lists fail "P1 an owner outside OWNERS" "$RUNBOOK" "$TMP/CODEOWNERS.wrong" \
    "not in OWNERS"

# Each trigger path trips, and a non-trigger file beside them doesn't.
labels() { # files... -> the labels check_triggers.py would fire
    "$PYTHON" -c '
import sys
sys.path.insert(0, sys.argv[1])
import check_triggers
print(" ".join(check_triggers.tripped_labels(sys.argv[2:])))
' "$SCRIPT_DIR" "$@"
}
expect_labels() { # file expected-labels
    local got
    got=$(labels "$1")
    if [[ "$got" == "$2" ]]; then
        pass "MB-0043 P2 $1 -> ${2:-no trigger}"
    else
        fail "MB-0043 P2 $1: expected '${2:-}', got '$got'"
    fi
}
expect_labels crates/kauppa-web/src/csp.rs security:headers
expect_labels crates/kauppa-web/src/csrf/mod.rs security:csrf
expect_labels crates/kauppa-web/src/csrf/tests/coverage.rs security:csrf
expect_labels crates/kauppa-web/src/config.rs security:config
expect_labels crates/kauppa-web/src/i18n/request.rs security:language-cookie
expect_labels crates/kauppa-web/src/routes/language.rs security:language-cookie
expect_labels crates/kauppa-web/src/routes/lists/items.rs security:shared-lists
expect_labels crates/kauppa-core/src/repo/sqlite/list.rs security:shared-lists
expect_labels crates/kauppa-core/src/repo/sqlite/list_access.rs security:shared-lists
expect_labels deploy/Dockerfile platform:deploy
expect_labels deploy/compose.yaml platform:deploy
expect_labels deploy/Caddyfile platform:deploy
expect_labels deploy/docker-entrypoint.sh platform:deploy
expect_labels deploy/terraform/main.tf platform:deploy
expect_labels deploy/backup/kauppa-backup platform:deploy
expect_labels crates/kauppa-web/Cargo.toml "platform:deps(workspace)"
expect_labels deploy/README.md ""
expect_labels crates/kauppa-web/src/i18n.rs ""
expect_labels crates/kauppa-core/src/repo/sqlite/list_rows.rs ""
expect_labels crates/kauppa-web/src/routes/recipes/mod.rs ""

if [[ $failures -gt 0 ]]; then
    echo "test_check_triggers: $failures failure(s)"
    exit 1
fi
echo "test_check_triggers: all checks passed"
