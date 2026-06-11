#!/usr/bin/env bash
# Asserts that the verify-only consumer's dependency graph contains no
# prover/host machinery. Must resolve with -p (resolver v2 per-package
# features); a --workspace build legitimately unifies rayon elsewhere.
set -euo pipefail
cd "$(dirname "$0")/.."

violations=$(cargo tree -p jolt-verify-smoke -e normal --prefix none --locked \
  | sort -u \
  | grep -E '^(jolt-core|tracer|jolt-sdk|jolt-inlines|rayon|sysinfo) ' || true)

if [ -n "$violations" ]; then
  echo "forbidden dependencies in jolt-verify-smoke:" >&2
  echo "$violations" >&2
  exit 1
fi
echo "ISOLATION-OK"
