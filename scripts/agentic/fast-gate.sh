#!/usr/bin/env bash
# fast-gate: the inner-loop check an agent runs before claiming S3 complete.
#
# Mirrors the first three gates of the feature runbook. CI runs more
# (cargo-deny, artifact validators) — this is the fast path.

set -euo pipefail

cd "$(dirname "$0")/../.."

echo "::group::cargo fmt"
cargo fmt --all -- --check
echo "::endgroup::"

echo "::group::cargo clippy"
cargo clippy --workspace --all-targets --all-features -- -D warnings
echo "::endgroup::"

echo "::group::cargo test"
cargo test --workspace --all-targets
echo "::endgroup::"

# kauppa-core alone: a feature that only arrives through another crate's
# dependency tree fails here (MB-0009, RR-0005).
echo "::group::cargo test -p kauppa-core"
cargo test -p kauppa-core --all-targets
echo "::endgroup::"

echo "fast-gate: OK"
