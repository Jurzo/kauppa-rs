#!/usr/bin/env bash
# scaffold-consult <slug>  →  .agentic/consults/CRP-NNNN-<slug>.md
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

id=$(next_id .agentic/consults)
dest=".agentic/consults/CRP-${id}-${slug}.md"
cp .agentic/consults/TEMPLATE.md "$dest"
sed -i "0,/^id: CRP-0001$/s//id: CRP-${id}/" "$dest" || true
echo "created $dest"
