#!/usr/bin/env bash
# check_workflows: CI runs on pinned inputs (MB-0045 P1, P2; RR-0032
# sub-answers 1 and 2).
#
#   P1  Every `uses:` in .github/workflows/ that isn't a local `./` path is
#       `owner/action@<40-hex commit SHA>  # v<version>`. Every workflow has
#       a top-level `permissions:`. No `write-all`, and no `write` permission
#       outside ALLOWED_WRITES below (adding an entry needs an RR; RR-0032
#       constraint 1).
#   P2  rust-toolchain.toml's channel is one exact version (N.N.N), it equals
#       the version of the Rust image in deploy/Dockerfile (its `FROM
#       rust:<version>…` line), and no workflow names another toolchain.
#
# The script works offline: it can't prove that a SHA is the version its
# comment claims. Dependabot keeps the two in step and a reviewer sees both
# in the diff.
#
# It first tests itself against fixtures in a temporary directory (a good
# tree must pass, each bad one must fail with the expected message), then
# checks this repository. Exit 1 if either part fails.
#
# Usage: scripts/agentic/check_workflows.sh   (PYTHON=... to pick the
# interpreter; it needs PyYAML)

set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$SCRIPT_DIR/../.." && pwd)
PYTHON="${PYTHON:-python3}"
export PYTHONDONTWRITEBYTECODE=1

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

# check_tree <root>: prints one line per problem; exit 1 if there is any.
check_tree() {
    "$PYTHON" - "$1" <<'PY'
import re
import sys
from pathlib import Path

import yaml

# The only `write` permissions a workflow may hold: (file, job, scope).
# A new entry needs a Resolution Record (RR-0032 constraint 1).
ALLOWED_WRITES = {
    ("deploy.yml", "image", "packages"),
}

root = Path(sys.argv[1])
problems = []


def problem(where, text):
    problems.append(f"{where}: {text}")


# --- the Rust version -------------------------------------------------------
EXACT = re.compile(r"\d+\.\d+\.\d+")
channel = None
toolchain_file = root / "rust-toolchain.toml"
if not toolchain_file.is_file():
    problem("rust-toolchain.toml", "missing")
else:
    in_table = False
    for line in toolchain_file.read_text(encoding="utf-8").splitlines():
        line = line.split("#", 1)[0].strip()
        if line.startswith("["):
            in_table = line == "[toolchain]"
        m = re.fullmatch(r"channel\s*=\s*([\"'])(.*)\1", line)
        if in_table and m:
            channel = m.group(2)
    if channel is None:
        problem("rust-toolchain.toml", "no `channel` under [toolchain]")
    elif not EXACT.fullmatch(channel):
        problem(
            "rust-toolchain.toml",
            f'channel "{channel}" is not an exact version (N.N.N)',
        )

# The Rust image: a literal `FROM rust:<version>…` line. An image held in an
# `ARG` default doesn't count: Dependabot reads only FROM lines (RR-0032
# sub-answer 3).
RUST_IMAGE = re.compile(
    r"^\s*FROM\s+(?:--platform=\S+\s+)?"
    r"(?:docker\.io/)?(?:library/)?rust:(?P<tag>[\w.-]+)",
    re.IGNORECASE,
)
dockerfile = root / "deploy" / "Dockerfile"
if not dockerfile.is_file():
    problem("deploy/Dockerfile", "missing")
else:
    tags = []
    for line in dockerfile.read_text(encoding="utf-8").splitlines():
        m = RUST_IMAGE.match(line)
        if m:
            tags.append(m.group("tag"))
    versions = set()
    for tag in tags:
        m = re.match(r"(\d+\.\d+\.\d+)(?:-|$)", tag)
        if m:
            versions.add(m.group(1))
        else:
            problem(
                "deploy/Dockerfile",
                f"the Rust image tag `{tag}` doesn't start with an exact version (N.N.N)",
            )
    if not tags:
        problem("deploy/Dockerfile", "no Rust image found (`FROM rust:<version>…`)")
    elif len(versions) > 1:
        problem("deploy/Dockerfile", f"more than one Rust image version: {sorted(versions)}")
    elif versions and channel is not None and EXACT.fullmatch(channel) and versions != {channel}:
        problem(
            "rust-toolchain.toml",
            f'channel "{channel}" differs from the Rust image in deploy/Dockerfile '
            f"({versions.pop()})",
        )

# --- the workflows ----------------------------------------------------------
USES_LINE = re.compile(r"^\s*(?:-\s+)?uses:\s*(?P<value>\S+)(?:\s+#\s*(?P<comment>.*))?\s*$")
PINNED = re.compile(r"[\w.-]+/[\w./-]+@[0-9a-f]{40}")
VERSION_COMMENT = re.compile(r"v\d+(\.\d+)*\b")

# A toolchain name: a channel, optionally dated, or a version.
NAME = r"(?:stable|beta|nightly)(?:-\d{4}-\d{2}-\d{2})?|\d+\.\d+(?:\.\d+)?"
TOOLCHAIN_NAMED = [
    # `toolchain: <name>` (an action input), `RUSTUP_TOOLCHAIN: <name>`.
    re.compile(rf"\b(?:toolchain|rust-version|RUSTUP_TOOLCHAIN)\s*[:=]\s*[\"']?(?P<name>{NAME})\b"),
    # `cargo +<name>`, `rustc +<name>`, …
    re.compile(rf"(?:^|\s)\+(?P<name>{NAME})\b"),
    # `rustup toolchain install <name>`, `rustup default <name>`, …
    re.compile(
        r"\brustup\s+(?:toolchain\s+(?:install|add|update)|install|update|default|"
        r"override\s+set|run)\b[^;&|\n]*?"
        rf"(?<![\w.+-])(?P<name>{NAME})(?![\w.])"
    ),
    # `dtolnay/rust-toolchain@<name>`: the ref is the toolchain.
    re.compile(rf"rust-toolchain@(?P<name>{NAME})\b"),
]


def count_uses(node):
    if isinstance(node, dict):
        return sum(
            (1 if key == "uses" and isinstance(value, str) else count_uses(value))
            for key, value in node.items()
        )
    if isinstance(node, list):
        return sum(count_uses(item) for item in node)
    return 0


def check_permissions(name, where, job, perms):
    if perms is None:
        return
    if isinstance(perms, str):
        if perms != "read-all":
            problem(where, f"`permissions: {perms}` (only `read-all` or a mapping is allowed)")
        return
    if not isinstance(perms, dict):
        problem(where, "`permissions:` is neither a mapping nor `read-all`")
        return
    for scope, level in perms.items():
        if level in ("read", "none"):
            continue
        if level == "write" and (name, job, scope) in ALLOWED_WRITES:
            continue
        problem(where, f"`{scope}: {level}` is not on the allowlist of write permissions")


workflow_dir = root / ".github" / "workflows"
workflows = sorted(
    p for p in workflow_dir.glob("*") if p.suffix in (".yml", ".yaml")
) if workflow_dir.is_dir() else []
if not workflows:
    problem(".github/workflows", "no workflow files found")

for path in workflows:
    name = path.name
    text = path.read_text(encoding="utf-8")
    try:
        doc = yaml.safe_load(text)
    except yaml.YAMLError as exc:
        problem(name, f"not valid YAML: {exc}")
        continue
    if not isinstance(doc, dict):
        problem(name, "not a YAML mapping")
        continue

    # P1: pins. Line by line, because the version lives in a comment; the
    # YAML count makes sure no `uses` is written in a way the lines miss.
    seen = 0
    for number, line in enumerate(text.splitlines(), 1):
        if line.lstrip().startswith("#"):
            continue
        m = USES_LINE.match(line)
        if not m:
            continue
        seen += 1
        value = m.group("value").strip("\"'")
        comment = (m.group("comment") or "").strip()
        where = f"{name}:{number}"
        if value.startswith("./"):
            continue
        if not PINNED.fullmatch(value):
            problem(where, f"`uses: {value}` is not pinned to a 40-hex commit SHA")
        elif not VERSION_COMMENT.match(comment):
            problem(where, f"`uses: {value}` has no `# v<version>` comment")
    if seen != count_uses(doc):
        problem(
            name,
            f"{count_uses(doc)} `uses` keys in the YAML but {seen} `uses:` lines read; "
            "write each as `uses: owner/action@<sha>  # v<version>` on one line",
        )

    # P1: permissions.
    if "permissions" not in doc or doc["permissions"] is None:
        problem(name, "no top-level `permissions:`")
    else:
        check_permissions(name, name, None, doc["permissions"])
    jobs = doc.get("jobs") if isinstance(doc.get("jobs"), dict) else {}
    for job, body in jobs.items():
        if isinstance(body, dict) and "permissions" in body:
            if body["permissions"] is None:
                problem(f"{name} job `{job}`", "empty `permissions:`")
            check_permissions(name, f"{name} job `{job}`", job, body["permissions"])
    if re.search(r"\bwrite-all\b", "\n".join(
        line.split("#", 1)[0] for line in text.splitlines()
    )) and not any("write-all" in p for p in problems if p.startswith(name)):
        problem(name, "`write-all` appears in the file")

    # P2: no other toolchain than the file's.
    for number, line in enumerate(text.splitlines(), 1):
        code = line.split(" #", 1)[0]
        if code.lstrip().startswith("#"):
            continue
        for pattern in TOOLCHAIN_NAMED:
            for m in pattern.finditer(code):
                if m.group("name") != channel:
                    problem(
                        f"{name}:{number}",
                        f"names the toolchain `{m.group('name')}`; "
                        f"rust-toolchain.toml says `{channel}`",
                    )

for line in problems:
    print(line)
sys.exit(1 if problems else 0)
PY
}

failures=0
pass() { echo "ok   $1"; }
fail() { echo "FAIL $1"; failures=$((failures + 1)); }

# --- Self-tests ---------------------------------------------------------------
SHA=0123456789abcdef0123456789abcdef01234567

# good_tree <dir>: a minimal tree that satisfies every rule.
good_tree() {
    mkdir -p "$1/.github/workflows" "$1/deploy"
    cat >"$1/rust-toolchain.toml" <<'EOF'
[toolchain]
channel = "1.99.1"
components = ["rustfmt", "clippy"]
EOF
    cat >"$1/deploy/Dockerfile" <<EOF
FROM rust:1.99.1-slim-trixie@sha256:$SHA$SHA AS builder
FROM debian:trixie-slim
EOF
    cat >"$1/.github/workflows/ci.yml" <<EOF
name: ci
on:
  pull_request:
permissions:
  contents: read
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@$SHA  # v4.4.0
      - name: Rust from rust-toolchain.toml
        run: rustup toolchain install
      - name: a local action
        uses: ./.github/actions/local
      - run: cargo test
EOF
    cat >"$1/.github/workflows/deploy.yml" <<EOF
name: deploy
on:
  push:
permissions:
  contents: read
jobs:
  image:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      packages: write
    steps:
      - uses: actions/checkout@$SHA # v4
  other:
    runs-on: ubuntu-latest
    steps:
      - run: "true"
EOF
}

# expect <pass|fail> <label> <dir> [text the output must contain]
expect() {
    local out rc=0
    out=$(check_tree "$3" 2>&1) || rc=$?
    [[ -z "$out" ]] || printf '%s\n' "$out" | sed 's/^/    /'
    if [[ $1 == pass && $rc -eq 0 ]] ||
        [[ $1 == fail && $rc -eq 1 && "$out" == *"${4:-}"* ]]; then
        pass "self-test $2"
    else
        fail "self-test $2 (exit $rc)"
    fi
}

# bad <label> <expected text> <sed expression> <file under the tree>
n=0
bad() {
    n=$((n + 1))
    local dir="$TMP/bad$n"
    good_tree "$dir"
    sed -i "$3" "$dir/$4"
    if cmp -s "$dir/$4" "$TMP/good/$4"; then
        fail "self-test $1: the fixture edit changed nothing"
        return
    fi
    expect fail "$1" "$dir" "$2"
}

good_tree "$TMP/good"
expect pass "the good tree passes" "$TMP/good"

CI=.github/workflows/ci.yml
DEPLOY=.github/workflows/deploy.yml

# P1: pins.
bad "P1 an action pinned by tag" "ci.yml:10: \`uses: actions/checkout@v4\` is not pinned" \
    "s|actions/checkout@$SHA  # v4.4.0|actions/checkout@v4|" $CI
bad "P1 an action pinned by branch" "is not pinned" \
    "s|actions/checkout@$SHA  # v4.4.0|dtolnay/rust-toolchain@master|" $CI
bad "P1 a short SHA" "is not pinned" \
    "s|actions/checkout@$SHA  # v4.4.0|actions/checkout@0123456  # v4.4.0|" $CI
bad "P1 a SHA without a version comment" "has no \`# v<version>\` comment" \
    "s|actions/checkout@$SHA  # v4.4.0|actions/checkout@$SHA|" $CI
bad "P1 a SHA whose comment is not a version" "has no \`# v<version>\` comment" \
    "s|# v4.4.0|# pinned|" $CI
bad "P1 a container action by tag" "is not pinned" \
    "s|actions/checkout@$SHA  # v4.4.0|docker://alpine:3.24|" $CI
bad "P1 a \`uses\` the line scan can't read" "\`uses\` keys in the YAML" \
    "s|      - uses: actions/checkout@$SHA  # v4.4.0|      - {uses: actions/checkout@v4}|" $CI

# P1: permissions.
bad "P1 a workflow without \`permissions:\`" "ci.yml: no top-level \`permissions:\`" \
    '/^permissions:$/,/^  contents: read$/d' $CI
bad "P1 an unlisted write permission (top level)" \
    "ci.yml: \`contents: write\` is not on the allowlist" \
    's|^  contents: read$|  contents: write|' $CI
bad "P1 an unlisted write permission (allowlisted job, other scope)" \
    "deploy.yml job \`image\`: \`contents: write\` is not on the allowlist" \
    's|^      contents: read$|      contents: write|' $DEPLOY
bad "P1 the allowlisted scope on another job" \
    "deploy.yml job \`other\`: \`packages: write\` is not on the allowlist" \
    's|^  other:$|  other:\n    permissions:\n      packages: write|' $DEPLOY
bad "P1 the allowlisted scope at the top level" \
    "deploy.yml: \`packages: write\` is not on the allowlist" \
    's|^  contents: read$|  contents: read\n  packages: write|' $DEPLOY
bad "P1 \`write-all\` at the top level" "ci.yml: \`permissions: write-all\`" \
    '/^  contents: read$/d; s|^permissions:$|permissions: write-all|' $CI
bad "P1 \`write-all\` on a job" "deploy.yml job \`image\`: \`permissions: write-all\`" \
    '/^      contents: read$/d; /^      packages: write$/d; s|^    permissions:$|    permissions: write-all|' $DEPLOY

# P2: one Rust version.
bad "P2 the toolchain differs from the Rust image" \
    'channel "1.99.0" differs from the Rust image in deploy/Dockerfile (1.99.1)' \
    's|channel = "1.99.1"|channel = "1.99.0"|' rust-toolchain.toml
bad "P2 the Rust image differs from the toolchain" \
    'channel "1.99.1" differs from the Rust image in deploy/Dockerfile (1.100.0)' \
    's|rust:1.99.1-slim|rust:1.100.0-slim|' deploy/Dockerfile
bad "P2 the channel is \`stable\`" 'channel "stable" is not an exact version (N.N.N)' \
    's|channel = "1.99.1"|channel = "stable"|' rust-toolchain.toml
bad "P2 the channel is N.N" 'channel "1.99" is not an exact version (N.N.N)' \
    's|channel = "1.99.1"|channel = "1.99"|' rust-toolchain.toml
bad "P2 no channel" "no \`channel\` under [toolchain]" \
    '/^channel/d' rust-toolchain.toml
bad "P2 the Rust image has no exact version" "doesn't start with an exact version" \
    's|rust:1.99.1-slim-trixie|rust:slim-trixie|' deploy/Dockerfile
bad "P2 no Rust image in the Dockerfile" "no Rust image found" \
    '/^FROM rust:/d' deploy/Dockerfile
bad "P2 a workflow installs another toolchain (rustup)" \
    "ci.yml:12: names the toolchain \`stable\`" \
    's|rustup toolchain install$|rustup toolchain install stable|' $CI
bad "P2 a workflow installs another version (rustup, with flags)" \
    "names the toolchain \`1.98.0\`" \
    's|rustup toolchain install$|rustup toolchain install --profile minimal 1.98.0|' $CI
bad "P2 a workflow names another toolchain (action input)" \
    "names the toolchain \`nightly\`" \
    's|        run: rustup toolchain install$|        with:\n          toolchain: nightly|' $CI
bad "P2 a workflow names another toolchain (cargo +name)" \
    "names the toolchain \`nightly-2026-11-01\`" \
    's|run: cargo test|run: cargo +nightly-2026-11-01 test|' $CI
bad "P2 a workflow names another toolchain (RUSTUP_TOOLCHAIN)" \
    "names the toolchain \`beta\`" \
    's|^permissions:$|env:\n  RUSTUP_TOOLCHAIN: beta\npermissions:|' $CI

bad "P2 the Rust image is held in an ARG, where the update bot can't read it" \
    "no Rust image found" \
    's|^FROM rust:\(.*\) AS builder$|ARG RUST_IMAGE=rust:\1\nFROM ${RUST_IMAGE} AS builder|' deploy/Dockerfile

# A workflow may name the file's own version.
good_tree "$TMP/same"
sed -i 's|rustup toolchain install$|rustup toolchain install 1.99.1|' "$TMP/same/$CI"
expect pass "P2 naming the pinned version is allowed" "$TMP/same"

# --- This repository ---------------------------------------------------------
echo "--- $REPO"
out="" rc=0
out=$(check_tree "$REPO" 2>&1) || rc=$?
[[ -z "$out" ]] || printf '%s\n' "$out" | sed 's/^/    /'
if [[ $rc -eq 0 ]]; then
    pass "P1 P2 this repository: actions pinned by SHA, permissions declared, one Rust version"
else
    fail "P1 P2 this repository ($(printf '%s\n' "$out" | wc -l) problem(s))"
fi

if [[ $failures -gt 0 ]]; then
    echo "check_workflows: $failures failure(s)"
    exit 1
fi
echo "check_workflows: all checks passed"
