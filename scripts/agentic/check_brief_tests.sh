#!/usr/bin/env bash
# check_brief_tests: every `cargo test` check in a Mission Brief selects at
# least one test (MB-0019 P4, RR-0009 constraint 2).
#
# Brief checks name exact test paths, and `cargo test <filter>` matches by
# substring, so a moved or renamed test leaves its check selecting nothing,
# silently. This runs each check with `-- --list` and fails on any that lists
# no tests. Briefs whose Continuity Pack says "In flight: … not started" are
# skipped: their tests don't exist yet.

set -euo pipefail

cd "$(dirname "$0")/../.."

failures=0
checked=0
skipped_briefs=()

for brief in .agentic/missions/MB-*.md; do
    id=$(basename "$brief" | cut -d- -f1-2)
    pack=".agentic/continuity/$id.md"
    if [[ -f "$pack" ]] && grep -qE "^- In flight: .*not started" "$pack"; then
        skipped_briefs+=("$id")
        continue
    fi

    # `check:` lines that are a single `cargo test` command, optionally quoted.
    while IFS= read -r cmd; do
        checked=$((checked + 1))
        # shellcheck disable=SC2086 # word splitting is the point: it's a command line
        count=$($cmd -- --list 2>/dev/null | grep -c ': test$' || true)
        if [[ "$count" -eq 0 ]]; then
            echo "FAIL $id: '$cmd' selects no tests"
            failures=$((failures + 1))
        else
            echo "ok   $id: '$cmd' ($count)"
        fi
    done < <(sed -nE 's/^[[:space:]]*check:[[:space:]]*["'\'']?(cargo test[^"'\''`;—]*[^"'\''`;— ])["'\'']?[[:space:]]*$/\1/p' "$brief")
done

if [[ ${#skipped_briefs[@]} -gt 0 ]]; then
    echo "skipped (not started): ${skipped_briefs[*]}"
fi
echo "checked $checked brief checks, $failures selecting no tests"
[[ "$failures" -eq 0 ]]
