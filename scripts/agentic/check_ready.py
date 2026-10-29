#!/usr/bin/env python3
"""Check that the PR body carries a complete MVP Merge-Readiness Pack.

Required for PRs labeled `agent-authored`. The workflow passes the PR body
in the PR_BODY env var and the base ref in PR_BASE.

What it enforces:

1. The PR body contains the top-level section headers from the template:
   `## Mission Brief`, `## Scope-to-proof map`, `## Verification bundle`,
   `## Engineering hygiene`, `## Rationale and trade-offs`,
   `## Risk and rollout plan`, `## Escalations`.
2. The PR body references a Mission Brief ID (`MB-NNNN`), and the referenced
   `.agentic/missions/<id>-*.md` file exists.
3. Every `acceptance[].id` from the brief appears in the body's scope-to-proof
   table.
"""

from __future__ import annotations

import os
import pathlib
import re
import sys

import yaml

ROOT = pathlib.Path(__file__).resolve().parents[2]
MISSIONS = ROOT / ".agentic" / "missions"

REQUIRED_HEADERS = [
    "## Mission Brief",
    "## Scope-to-proof map",
    "## Verification bundle",
    "## Engineering hygiene",
    "## Rationale and trade-offs",
    "## Risk and rollout plan",
    "## Escalations",
]

MB_RE = re.compile(r"\bMB-(\d{4,})\b")
FRONT_MATTER = re.compile(r"^```yaml\n(.*?)\n```", re.DOTALL | re.MULTILINE)


def fail(msg: str) -> None:
    print(f"check_ready: FAIL: {msg}")


def main() -> int:
    body = os.environ.get("PR_BODY") or ""
    if not body.strip():
        fail("PR body is empty — fill the Merge-Readiness Pack template.")
        return 1

    errors: list[str] = []

    for header in REQUIRED_HEADERS:
        if header not in body:
            errors.append(f"missing section: {header}")

    mb_match = MB_RE.search(body)
    if not mb_match:
        errors.append("PR body does not reference a Mission Brief (MB-NNNN).")
    else:
        mb_id = f"MB-{mb_match.group(1)}"
        candidates = list(MISSIONS.glob(f"{mb_id}-*.md"))
        if not candidates:
            errors.append(f"Mission Brief {mb_id} not found in .agentic/missions/.")
        else:
            brief_path = candidates[0]
            text = brief_path.read_text(encoding="utf-8")
            m = FRONT_MATTER.search(text)
            if not m:
                errors.append(f"{brief_path.name}: no YAML front matter.")
            else:
                # `---`-fenced front matter is a YAML stream; take the first
                # non-empty document (safe_load would reject the trailing `---`).
                docs = [d for d in yaml.safe_load_all(m.group(1)) if d is not None]
                data = docs[0] if docs else {}
                for prop in data.get("acceptance") or []:
                    pid = prop.get("id")
                    if pid and pid not in body:
                        errors.append(
                            f"scope-to-proof map missing acceptance id {pid} "
                            f"from {brief_path.name}"
                        )

    if errors:
        for e in errors:
            fail(e)
        return 1

    print("check_ready: OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
