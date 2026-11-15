#!/usr/bin/env bash
# scaffold-mission <slug>  →  creates .agentic/missions/MB-NNNN-<slug>.md and
# its Continuity Pack .agentic/continuity/MB-NNNN.md, state "not started"
# (AGENTS.md → Continuity Packs; MB-0050 P6, RR-0042).
set -euo pipefail

cd "$(dirname "$0")/../.."

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <slug>" >&2
  exit 2
fi
slug="$1"

next_id() {
  local max=0
  shopt -s nullglob
  for f in "$1"/*.md; do
    name="$(basename "$f")"
    [[ "$name" == "TEMPLATE.md" || "$name" == "README.md" ]] && continue
    if [[ "$name" =~ ^[A-Z]+-([0-9]{4,}) ]]; then
      n=$((10#${BASH_REMATCH[1]}))
      (( n > max )) && max=$n
    fi
  done
  printf '%04d' $((max + 1))
}

id=$(next_id .agentic/missions)
dest=".agentic/missions/MB-${id}-${slug}.md"
pack=".agentic/continuity/MB-${id}.md"

# Refuse before writing anything, so a failed run leaves no half-made mission.
for f in "$dest" "$pack"; do
  if [[ -e "$f" ]]; then
    echo "refusing to overwrite $f" >&2
    exit 1
  fi
done

cp .agentic/missions/TEMPLATE.md "$dest"
# Update the id in the YAML block (first match).
sed -i "0,/^id: MB-0001$/s//id: MB-${id}/" "$dest"
echo "created $dest"

# The pack takes the brief's owner, so the two never disagree.
owner=$(sed -nE '0,/^owner:/s/^owner:[[:space:]]*(.*)$/\1/p' "$dest")
now=$(date -u +%Y-%m-%dT%H:%M:%SZ)
today=${now%%T*}

# Copy the template without its instruction paragraph (from the line after
# the title up to the YAML block), then fill in the front matter and state.
awk -v id="MB-${id}" -v owner="$owner" -v now="$now" -v today="$today" '
  NR == 1 { sub(/<MB-id>/, id); print; print ""; skip = 1; next }
  skip && /^```yaml$/ { skip = 0 }
  skip { next }
  /^mission: /    { print "mission: " id; next }
  /^updated_at: / { print "updated_at: " now; next }
  /^owner: /      { print "owner: " owner; next }
  /^- Done: /     { print "- Done: Brief drafted (" today ")."; next }
  /^- In flight: / {
    print "- In flight: implementation not started; the brief awaits the owner'"'"'s acceptance."
    next
  }
  { print }
' .agentic/continuity/TEMPLATE.md > "$pack"
echo "created $pack"
