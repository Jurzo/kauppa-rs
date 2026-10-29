#!/usr/bin/env python3
"""Validate the YAML front matter of every .agentic/ artifact against its schema.

Fails with a non-zero exit if any artifact is malformed or references a schema
that cannot be loaded. Skips TEMPLATE.md files.
"""

from __future__ import annotations

import json
import pathlib
import re
import sys

import yaml
from jsonschema import Draft202012Validator

ROOT = pathlib.Path(__file__).resolve().parents[2]
AGENTIC = ROOT / ".agentic"
SCHEMAS = AGENTIC / "schemas"

ARTIFACT_DIRS = {
    "missions": SCHEMAS / "mission-brief.schema.json",
    "consults": SCHEMAS / "consultation-request.schema.json",
    "resolutions": SCHEMAS / "resolution-record.schema.json",
}

FRONT_MATTER = re.compile(r"^```yaml\n(.*?)\n```", re.DOTALL | re.MULTILINE)


class _FrontMatterLoader(yaml.SafeLoader):
    """SafeLoader that keeps RFC3339 timestamps as strings.

    The schemas type fields like `decided_at` as `string`, but PyYAML's implicit
    resolver would parse an unquoted `2026-10-28T00:00:00Z` into a datetime and
    fail validation. Drop the timestamp resolver so such scalars load as str.
    """


_FrontMatterLoader.yaml_implicit_resolvers = {
    ch: [(tag, regexp) for tag, regexp in resolvers if tag != "tag:yaml.org,2002:timestamp"]
    for ch, resolvers in yaml.SafeLoader.yaml_implicit_resolvers.items()
}


def extract_front_matter(path: pathlib.Path) -> dict | None:
    text = path.read_text(encoding="utf-8")
    match = FRONT_MATTER.search(text)
    if not match:
        return None
    # The front-matter block wraps the mapping in `---` fences (see the
    # TEMPLATE files), so the captured text is a YAML *stream* whose trailing
    # `---` opens an empty second document. safe_load() rejects multi-document
    # streams; take the first non-empty document instead.
    docs = [doc for doc in yaml.load_all(match.group(1), Loader=_FrontMatterLoader) if doc is not None]
    return docs[0] if docs else None


def validate_dir(dir_name: str, schema_path: pathlib.Path) -> list[str]:
    errors: list[str] = []
    schema = json.loads(schema_path.read_text(encoding="utf-8"))
    validator = Draft202012Validator(schema)

    base = AGENTIC / dir_name
    if not base.exists():
        return errors

    for path in sorted(base.glob("*.md")):
        if path.name in {"TEMPLATE.md", "README.md"}:
            continue
        data = extract_front_matter(path)
        if data is None:
            errors.append(f"{path.relative_to(ROOT)}: no YAML front matter block found")
            continue
        for err in validator.iter_errors(data):
            location = "/".join(str(p) for p in err.absolute_path) or "<root>"
            errors.append(f"{path.relative_to(ROOT)}: {location}: {err.message}")
    return errors


def main() -> int:
    all_errors: list[str] = []
    for dir_name, schema_path in ARTIFACT_DIRS.items():
        all_errors.extend(validate_dir(dir_name, schema_path))

    if all_errors:
        print("Artifact validation failed:")
        for err in all_errors:
            print(f"  - {err}")
        return 1

    print("All .agentic/ artifacts validate.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
