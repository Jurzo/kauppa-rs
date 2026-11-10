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
    # Matches routes/auth.rs and routes/auth/ (RR-0012).
    ("crates/kauppa-web/src/routes/auth", "security:auth"),
    ("crates/kauppa-web/src/routes/admin/", "security:admin"),
    ("crates/kauppa-web/src/admin/", "security:admin"),
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


# A shallow clone is deepened this many commits at a time, at most this many
# times, until the merge base is reachable.
DEEPEN_STEP = 50
DEEPEN_ROUNDS = 20


def _git(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(["git", *args], check=False, capture_output=True, text=True)


def _is_shallow() -> bool:
    return _git("rev-parse", "--is-shallow-repository").stdout.strip() == "true"


def _merge_base(base: str) -> str | None:
    result = _git("merge-base", f"origin/{base}", "HEAD")
    return result.stdout.strip() if result.returncode == 0 else None


def changed_files(base: str) -> list[str]:
    # Never fetch with --depth: in a full clone it truncates history and makes
    # the repository shallow (MB-0033). A full clone just fetches the base; a
    # shallow one is deepened (--deepen extends the existing boundaries) until
    # the merge base is reachable. The explicit refspec creates origin/<base>
    # even in a single-branch clone.
    refspec = f"+refs/heads/{base}:refs/remotes/origin/{base}"
    merge_base = None
    if _is_shallow():
        for _ in range(DEEPEN_ROUNDS):
            fetch = _git(
                "fetch", "--no-tags", f"--deepen={DEEPEN_STEP}", "origin", refspec
            )
            merge_base = _merge_base(base)
            if merge_base or fetch.returncode != 0 or not _is_shallow():
                break
    else:
        _git("fetch", "--no-tags", "origin", refspec)
        merge_base = _merge_base(base)

    # Fallback: if git merge-base fails, diff against HEAD~1.
    if merge_base is None:
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
        # The front-matter block wraps the mapping in `---` fences, so it is a
        # YAML stream (trailing `---` opens an empty second doc); safe_load
        # rejects that. Take the first non-empty document.
        docs = [d for d in yaml.safe_load_all(m.group(1)) if d is not None]
        data = docs[0] if docs else {}
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
