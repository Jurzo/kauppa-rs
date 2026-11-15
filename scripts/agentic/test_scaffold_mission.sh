#!/usr/bin/env bash
# test_scaffold_mission: scaffold-mission.sh creates a Mission Brief and its
# Continuity Pack with matching ids and owner, the pack's state is "not
# started" (the line check_brief_tests.sh skips on), the artifacts still
# validate, and a second run never overwrites an existing pack (MB-0050 P6,
# RR-0042).
#
# Runs against a copy of `.agentic/` and `scripts/agentic/` in a temporary
# directory; the working copy is never touched.
#
# Usage: scripts/agentic/test_scaffold_mission.sh   (PYTHON=... to pick the
# interpreter; validate_artifacts.py needs PyYAML and jsonschema)

set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$SCRIPT_DIR/../.." && pwd)
PYTHON="${PYTHON:-python3}"
export PYTHONDONTWRITEBYTECODE=1

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

mkdir -p "$TMP/scripts"
cp -R "$REPO/.agentic" "$TMP/.agentic"
cp -R "$REPO/scripts/agentic" "$TMP/scripts/agentic"

failures=0
fail() { echo "FAIL $*"; failures=$((failures + 1)); }
ok() { echo "ok   $*"; }

# The id the script should pick: one past the highest existing brief.
max=$(find "$TMP/.agentic/missions" -name 'MB-[0-9]*.md' -printf '%f\n' \
    | sed -E 's/^MB-0*([0-9]+).*/\1/' | sort -n | tail -1)
id=$(printf 'MB-%04d' $((max + 1)))
brief="$TMP/.agentic/missions/$id-scaffold-test.md"
pack="$TMP/.agentic/continuity/$id.md"

"$TMP/scripts/agentic/scaffold-mission.sh" scaffold-test >/dev/null

[[ -f "$brief" ]] && ok "brief created ($id)" || fail "no brief at $brief"
[[ -f "$pack" ]] && ok "pack created ($id)" || fail "no pack at $pack"

if [[ -f "$brief" && -f "$pack" ]]; then
    grep -qx "id: $id" "$brief" && ok "brief id is $id" || fail "brief id is not $id"
    grep -qx "mission: $id" "$pack" && ok "pack mission is $id" || fail "pack mission is not $id"
    head -1 "$pack" | grep -qF "Continuity Pack — $id" \
        && ok "pack title names $id" || fail "pack title doesn't name $id"

    brief_owner=$(sed -nE '0,/^owner:/s/^owner:[[:space:]]*(.*)$/\1/p' "$brief")
    pack_owner=$(sed -nE 's/^owner:[[:space:]]*(.*)$/\1/p' "$pack")
    [[ -n "$brief_owner" && "$brief_owner" == "$pack_owner" ]] \
        && ok "pack owner matches the brief ($pack_owner)" \
        || fail "pack owner '$pack_owner' != brief owner '$brief_owner'"

    grep -qE '^updated_at: [0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$' "$pack" \
        && ok "updated_at is an RFC 3339 UTC time" || fail "updated_at is not set"

    # The same pattern check_brief_tests.sh skips on.
    grep -qE "^- In flight: .*not started" "$pack" \
        && ok "state is \"not started\"" || fail "no \"In flight: … not started\" line"

    grep -qF "Copy to \`<MB-id>.md\`" "$pack" \
        && fail "the template's instruction paragraph was kept" \
        || ok "template instructions dropped"

    for heading in "## Current state snapshot" "## Open questions and next steps" \
        "## Dead ends and rejected approaches" "## Decisions and constraints" \
        "## Evidence pointers" "## Handoff metadata"; do
        grep -qxF "$heading" "$pack" || fail "pack lacks '$heading'"
    done
fi

if "$PYTHON" "$TMP/scripts/agentic/validate_artifacts.py" >/dev/null; then
    ok "validate_artifacts.py passes with the new brief"
else
    fail "validate_artifacts.py fails with the new brief"
fi

# A pack that already exists for the next id is never overwritten, and no
# brief is left behind.
next=$(printf 'MB-%04d' $((max + 2)))
echo "sentinel" > "$TMP/.agentic/continuity/$next.md"
if "$TMP/scripts/agentic/scaffold-mission.sh" second-test >/dev/null 2>&1; then
    fail "second run succeeded over an existing pack"
else
    ok "second run refused an existing pack"
fi
[[ "$(cat "$TMP/.agentic/continuity/$next.md")" == "sentinel" ]] \
    && ok "existing pack unchanged" || fail "existing pack was overwritten"
compgen -G "$TMP/.agentic/missions/$next-*.md" >/dev/null \
    && fail "a brief was left behind by the refused run" \
    || ok "no brief left behind"

echo "$failures failures"
[[ "$failures" -eq 0 ]]
