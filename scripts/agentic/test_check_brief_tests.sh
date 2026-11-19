#!/usr/bin/env bash
# Self-test for check_brief_tests.sh (MB-0067 P3, P4): fixture briefs and
# packs and canned listings, so it needs no Rust build.
set -euo pipefail

cd "$(dirname "$0")/../.."
script="$PWD/scripts/agentic/check_brief_tests.sh"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

passed=0
failed=0

# A canned listing for `cargo test <args>`, as `-- --list` prints it.
listing() {
    local name="cargo test $1"
    shift
    printf '%s\n' "$@" >"$work/listings/${name// /_}.list"
}

# A brief MB-<id> whose `check:` lines are the remaining arguments; with
# `not-started`, its pack says so.
brief() {
    local id="$1" state="$2"
    shift 2
    {
        echo '```yaml'
        echo "id: $id"
        echo 'acceptance:'
        for check in "$@"; do
            echo '  - id: P1'
            echo "    check: $check"
        done
        echo '```'
    } >"$work/root/.agentic/missions/$id-fixture.md"
    if [[ "$state" == not-started ]]; then
        echo "- In flight: implementation not started" >"$work/root/.agentic/continuity/$id.md"
    fi
}

reset() {
    rm -rf "$work/root" "$work/listings"
    mkdir -p "$work/root/.agentic/missions" "$work/root/.agentic/continuity" "$work/listings"
    listing "-p core" \
        "    Running unittests src/lib.rs" \
        "repo::a::one: test" "repo::a::one_more: test" "repo::b::two: test" \
        "domain::three: test" "src/lib.rs - doc (line 3): test" "repo::bench: benchmark"
    listing "-p core --test compat" "verifies: test" "rejects: test"
    listing "-p web" "routes::x::tests::page: test" "routes::y::tests::form: test"
    listing "-p web --all-targets" "routes::x::tests::page: test" "routes::y::tests::form: test"
    listing "--workspace --all-targets" "repo::a::one: test" "routes::x::tests::page: test"
}

# expect <name> <exit code> <pattern>...: the script's exit code, and each
# pattern (a fixed string) in its output.
expect() {
    local name="$1" code="$2"
    shift 2
    local out status=0
    out=$(BRIEF_CHECK_ROOT="$work/root" BRIEF_CHECK_LISTINGS="$work/listings" "$script" 2>&1) || status=$?
    local ok=1
    [[ "$status" -eq "$code" ]] || ok=0
    for pattern in "$@"; do
        grep -qF -- "$pattern" <<<"$out" || ok=0
    done
    if ((ok)); then
        echo "ok   $name"
        passed=$((passed + 1))
    else
        echo "FAIL $name (exit $status, wanted $code)"
        sed 's/^/     | /' <<<"$out"
        failed=$((failed + 1))
    fi
}

# P4: every shape, matched as libtest filters (substring; no filter is all).
reset
brief MB-0001 started \
    "cargo test -p core repo::a::one" \
    "\"cargo test -p core repo::\"" \
    "cargo test -p core" \
    "cargo test -p core --test compat" \
    "cargo test -p core --test compat rejects" \
    "cargo test -p web routes::x::tests::page" \
    "cargo test -p web --all-targets" \
    "cargo test --workspace --all-targets" \
    "scripts/agentic/other.sh"
expect "every shape, substring matching" 0 \
    "ok   MB-0001: 'cargo test -p core repo::a::one' (2)" \
    "ok   MB-0001: 'cargo test -p core repo::' (3)" \
    "ok   MB-0001: 'cargo test -p core' (5)" \
    "ok   MB-0001: 'cargo test -p core --test compat' (2)" \
    "ok   MB-0001: 'cargo test -p core --test compat rejects' (1)" \
    "ok   MB-0001: 'cargo test -p web routes::x::tests::page' (1)" \
    "ok   MB-0001: 'cargo test -p web --all-targets' (2)" \
    "ok   MB-0001: 'cargo test --workspace --all-targets' (2)" \
    "checked 8 brief checks, 0 selecting no tests" \
    "listed 5 commands"

# P4: `-p kauppa-web` checks, with or without --all-targets, are matched
# against kauppa-web's tests in the workspace build (one listing for both);
# a `--test` check of it is still listed by its own command.
reset
listing "-p kauppa-web" "routes::stale::from_own_build: test"
listing "-p kauppa-web --test smoke" "boots: test"
printf '%s\n' "routes::x::tests::page: test" "routes::y::tests::form: test" \
    >"$work/listings/workspace_build:_kauppa-web.list"
brief MB-0001 started \
    "cargo test -p kauppa-web routes::x" \
    "cargo test -p kauppa-web --all-targets" \
    "cargo test -p kauppa-web routes::stale" \
    "cargo test -p kauppa-web --test smoke"
expect "kauppa-web from the workspace build" 1 \
    "ok   MB-0001: 'cargo test -p kauppa-web routes::x' (1)" \
    "ok   MB-0001: 'cargo test -p kauppa-web --all-targets' (2)" \
    "FAIL MB-0001: 'cargo test -p kauppa-web routes::stale' selects no tests" \
    "ok   MB-0001: 'cargo test -p kauppa-web --test smoke' (1)" \
    "listed 2 commands"

# P2: a command is listed once however many checks share it.
reset
brief MB-0001 started "cargo test -p core repo::a" "cargo test -p core repo::b" "cargo test -p core domain"
expect "one listing per command" 0 "listed 1 commands" "checked 3 brief checks, 0 selecting no tests"

# P3: a filter that selects nothing.
reset
brief MB-0001 started "cargo test -p core repo::gone" "cargo test -p core repo::b"
expect "filter selecting nothing" 1 \
    "FAIL MB-0001: 'cargo test -p core repo::gone' selects no tests" \
    "checked 2 brief checks, 1 selecting no tests"

# P3: `--test` keeps only that target: a name from another target fails.
reset
brief MB-0001 started "cargo test -p core --test compat repo::a::one"
expect "--test selecting nothing in its target" 1 \
    "FAIL MB-0001: 'cargo test -p core --test compat repo::a::one' selects no tests"

# P3: shapes the script doesn't know.
reset
brief MB-0001 started \
    "cargo test -p core --lib repo::a" \
    "cargo test -p core repo::a repo::b" \
    "cargo test repo::a" \
    "cargo test -p core --test"
expect "unknown shapes" 1 \
    "FAIL MB-0001: 'cargo test -p core --lib repo::a' is not a check this script knows: unrecognised option '--lib'" \
    "FAIL MB-0001: 'cargo test -p core repo::a repo::b' is not a check this script knows: more than one filter" \
    "FAIL MB-0001: 'cargo test repo::a' is not a check this script knows: names no package" \
    "FAIL MB-0001: 'cargo test -p core --test' is not a check this script knows: '--test' needs a value" \
    "checked 4 brief checks, 4 selecting no tests"

# P3: a listing that can't be produced (an unknown package) fails its checks.
reset
brief MB-0001 started "cargo test -p nosuch repo::a" "cargo test -p core repo::a"
expect "failed listing" 1 \
    "FAIL MB-0001: 'cargo test -p nosuch repo::a' could not be listed" \
    "ok   MB-0001: 'cargo test -p core repo::a' (2)" \
    "checked 2 brief checks, 1 selecting no tests"

# P3: an empty listing selects nothing, even without a filter.
reset
listing "-p empty"
brief MB-0001 started "cargo test -p empty"
expect "empty listing" 1 "FAIL MB-0001: 'cargo test -p empty' selects no tests"

# P4: a brief whose pack says "not started" is skipped.
reset
brief MB-0001 started "cargo test -p core repo::a"
brief MB-0002 not-started "cargo test -p core repo::not_yet"
expect "not started is skipped" 0 \
    "skipped (not started): MB-0002" \
    "checked 1 brief checks, 0 selecting no tests"

echo "test_check_brief_tests: $passed passed, $failed failed"
[[ "$failed" -eq 0 ]]
