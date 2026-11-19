#!/usr/bin/env bash
# check_brief_tests: every `cargo test` check in a Mission Brief selects at
# least one test (MB-0019 P4, RR-0009 constraint 2).
#
# Brief checks name exact test paths, and `cargo test <filter>` matches by
# substring, so a moved or renamed test leaves its check selecting nothing,
# silently. This fails on any check that selects no tests. Briefs whose
# Continuity Pack says "In flight: … not started" are skipped: their tests
# don't exist yet.
#
# MB-0067: each distinct command without its filter (`cargo test -p
# kauppa-web`, `cargo test --workspace --all-targets`, …) is listed once with
# `-- --list`; every check is then matched against its command's listing as
# libtest filters: the tests whose name contains the filter. A check in a
# shape this script doesn't know, or whose listing cargo fails to produce,
# fails.
#
# For the self-test (`test_check_brief_tests.sh`):
# - BRIEF_CHECK_ROOT: the directory holding `.agentic/` (default: the repo).
# - BRIEF_CHECK_LISTINGS: a directory of canned listings, one file per
#   listing command, named as the command with spaces replaced by `_` and
#   `.list` appended; cargo is then not run, and a missing file is a failed
#   listing.

set -euo pipefail

repo="$(cd "$(dirname "$0")/../.." && pwd)"
root="${BRIEF_CHECK_ROOT:-$repo}"
listings="${BRIEF_CHECK_LISTINGS:-}"
cd "$repo"

failures=0
checked=0
skipped_briefs=()

# Every check, in brief order: its brief id, the command, and either the
# listing command plus filter, or why its shape is refused.
ids=()
cmds=()
keys=()
filters=()
refusals=()

# parse <cmd>: sets `key` (the command without its filter), `filter`, and
# `refusal` (empty when the shape is known).
parse() {
    local -a words
    read -ra words <<<"$1"
    key="cargo test"
    filter=""
    refusal=""
    local have_filter=0 have_pkg=0 i=2
    while ((i < ${#words[@]})); do
        local w="${words[i]}"
        case "$w" in
            -p | --package | --test)
                if ((i + 1 >= ${#words[@]})) || [[ "${words[i + 1]}" == -* ]]; then
                    refusal="'$w' needs a value"
                    return
                fi
                [[ "$w" != --test ]] && have_pkg=1
                key+=" $w ${words[i + 1]}"
                i=$((i + 2))
                ;;
            --workspace)
                have_pkg=1
                key+=" $w"
                i=$((i + 1))
                ;;
            --all-targets)
                key+=" $w"
                i=$((i + 1))
                ;;
            -*)
                refusal="unrecognised option '$w'"
                return
                ;;
            *)
                if ((have_filter)); then
                    refusal="more than one filter"
                    return
                fi
                have_filter=1
                filter="$w"
                i=$((i + 1))
                ;;
        esac
    done
    if ((!have_pkg)); then
        refusal="names no package (-p or --workspace)"
    fi
}

for brief in "$root"/.agentic/missions/MB-*.md; do
    id=$(basename "$brief" | cut -d- -f1-2)
    pack="$root/.agentic/continuity/$id.md"
    if [[ -f "$pack" ]] && grep -qE "^- In flight: .*not started" "$pack"; then
        skipped_briefs+=("$id")
        continue
    fi

    # `check:` lines that are a single `cargo test` command, optionally quoted.
    while IFS= read -r cmd; do
        parse "$cmd"
        ids+=("$id")
        cmds+=("$cmd")
        keys+=("$key")
        filters+=("$filter")
        refusals+=("$refusal")
    done < <(sed -nE 's/^[[:space:]]*check:[[:space:]]*["'\'']?(cargo test[^"'\''`;—]*[^"'\''`;— ])["'\'']?[[:space:]]*$/\1/p' "$brief")
done

# List each distinct command once. `listed[key]` holds its test names, one
# per line; a key missing from `listed` failed to list.
declare -A listed=()
declare -A listing_error=()
starts=0
for i in "${!keys[@]}"; do
    [[ -n "${refusals[i]}" ]] && continue
    k="${keys[i]}"
    [[ -n "${listed[$k]+set}" || -n "${listing_error[$k]+set}" ]] && continue
    if [[ -n "$listings" ]]; then
        file="$listings/${k// /_}.list"
        if [[ -f "$file" ]]; then
            out=$(cat "$file")
        else
            listing_error[$k]="no canned listing $file"
            continue
        fi
    else
        starts=$((starts + 1))
        err=$(mktemp)
        # shellcheck disable=SC2086 # word splitting is the point: it's a command line
        if ! out=$($k -- --list 2>"$err"); then
            listing_error[$k]="$(tail -n 3 "$err" | tr '\n' ' ')"
            rm -f "$err"
            continue
        fi
        rm -f "$err"
    fi
    listed[$k]=$(sed -n 's/: test$//p' <<<"$out")
done

for i in "${!cmds[@]}"; do
    checked=$((checked + 1))
    id="${ids[i]}"
    cmd="${cmds[i]}"
    k="${keys[i]}"
    if [[ -n "${refusals[i]}" ]]; then
        echo "FAIL $id: '$cmd' is not a check this script knows: ${refusals[i]}"
        failures=$((failures + 1))
        continue
    fi
    if [[ -z "${listed[$k]+set}" ]]; then
        echo "FAIL $id: '$cmd' could not be listed: ${listing_error[$k]}"
        failures=$((failures + 1))
        continue
    fi
    names="${listed[$k]}"
    if [[ -z "$names" ]]; then
        count=0
    else
        count=$(grep -cF -- "${filters[i]}" <<<"$names" || true)
    fi
    if [[ "$count" -eq 0 ]]; then
        echo "FAIL $id: '$cmd' selects no tests"
        failures=$((failures + 1))
    else
        echo "ok   $id: '$cmd' ($count)"
    fi
done

if [[ ${#skipped_briefs[@]} -gt 0 ]]; then
    echo "skipped (not started): ${skipped_briefs[*]}"
fi
echo "listed ${#listed[@]} commands, starting cargo $starts times" >&2
echo "checked $checked brief checks, $failures selecting no tests"
[[ "$failures" -eq 0 ]]
