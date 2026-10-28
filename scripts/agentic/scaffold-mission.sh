#!/usr/bin/env bash
# scaffold-mission <slug>  →  creates .agentic/missions/MB-NNNN-<slug>.md
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
cp .agentic/missions/TEMPLATE.md "$dest"
# Update the id in the YAML block (first match).
sed -i "0,/^id: MB-0001$/s//id: MB-${id}/" "$dest" || true
echo "created $dest"
