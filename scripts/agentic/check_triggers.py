#!/usr/bin/env python3
"""Fail CI if a PR touches an escalation-trigger path without a linked, approved RR.

Trigger paths and their routing are defined in `.agentic/runbooks/feature.md`.
Keep this script in sync when you add or remove paths there.

The workflow provides PR_BASE (base ref) and PR_BODY (full PR description).
"""

from __future__ import annotations

import os
import pathlib
import re
import subprocess
import sys

import yaml

ROOT = pathlib.Path(__file__).resolve().parents[2]
RRS = ROOT / ".agentic" / "resolutions"
FRONT_MATTER = re.compile(r"^```yaml\n(.*?)\n```", re.DOTALL | re.MULTILINE)

TRIGGERS = [
    # (path prefix, label)
    ("migrations/", "schema"),
    ("crates/kauppa-core/src/password.rs", "security:password"),
    ("crates/kauppa-web/src/routes/auth.rs", "security:auth"),
    ("crates/kauppa-web/src/main.rs", "security:session-middleware"),
    ("Cargo.toml", "platform:deps"),
    ("rust-toolchain.toml", "platform:toolchain"),
    ("rustfmt.toml", "platform:lint-config"),
    ("deny.toml", "platform:lint-config"),
    (".agentic/mentorship/", "governance"),
    (".agentic/runbooks/", "governance"),
    (".agentic/resolutions/", "governance"),
]

RR_RE = re.compile(r"\bRR-(\d{4,})\b")


def changed_files(base: str) -> list[str]:
    # Fallback: if git merge-base fails, diff against HEAD~1.
    try:
        subprocess.run(
            ["git", "fetch", "--no-tags", "--depth=1", "origin", base],
            check=False,
            capture_output=True,
        )
        merge_base = subprocess.check_output(
            ["git", "merge-base", f"origin/{base}", "HEAD"], text=True
        ).strip()
    except subprocess.CalledProcessError:
        merge_base = "HEAD~1"

    out = subprocess.check_output(
        ["git", "diff", "--name-only", f"{merge_base}...HEAD"], text=True
    )
    return [line.strip() for line in out.splitlines() if line.strip()]


def crate_cargo_touched(files: list[str]) -> bool:
    return any(f.startswith("crates/") and f.endswith("Cargo.toml") for f in files)


def approved_rrs() -> set[str]:
    found: set[str] = set()
    if not RRS.exists():
        return found
    for path in RRS.glob("*.md"):
        if path.name in {"TEMPLATE.md", "README.md"}:
            continue
        text = path.read_text(encoding="utf-8")
        m = FRONT_MATTER.search(text)
        if not m:
            continue
        data = yaml.safe_load(m.group(1)) or {}
        if data.get("status") == "approved":
            found.add(data.get("id", ""))
    return found


def main() -> int:
    base = os.environ.get("PR_BASE") or "main"
    body = os.environ.get("PR_BODY") or ""

    files = changed_files(base)
    if not files:
        print("check_triggers: no changed files detected; skipping.")
        return 0

    tripped: list[str] = []
    for prefix, label in TRIGGERS:
        if any(f == prefix or f.startswith(prefix) for f in files):
            tripped.append(label)
    if crate_cargo_touched(files):
        tripped.append("platform:deps(workspace)")

    if not tripped:
        print("check_triggers: no escalation paths touched; OK.")
        return 0

    referenced = {f"RR-{m}" for m in RR_RE.findall(body)}
    approved = approved_rrs()
    satisfied = referenced & approved

    if satisfied:
        print(f"check_triggers: triggers {tripped} satisfied by {sorted(satisfied)}")
        return 0

    print("check_triggers: FAIL")
    print(f"  triggers fired: {tripped}")
    print(f"  RRs referenced in PR body: {sorted(referenced) or 'none'}")
    print(f"  approved RRs on disk: {sorted(approved) or 'none'}")
    print("  Open a Consultation Request Pack and get a Resolution Record approved,")
    print("  then reference its RR-ID in the PR body.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
