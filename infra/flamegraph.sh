#!/usr/bin/env bash

# Generate a flamegraph for a single egg file
# Currently hard-coded to sequential run mode

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

cleanup() {
  echo "Cleaning up"
  rm -rf "$REPO_ROOT/FlameGraph"
  rm -rf "$REPO_ROOT/nightly/raw"
}
trap cleanup ERR INT TERM

if [[ $# -ne 2 ]]; then
  echo "Usage: $0 <file.egg> <out_dir>"
  exit 1
fi

EGG_FILE="$1"
OUT_DIR="$2"

mkdir -p "$OUT_DIR"

if [[ ! -f "$EGG_FILE" ]]; then
  echo "Error: file not found: $EGG_FILE"
  exit 1
fi

base=$(basename "$EGG_FILE" .egg)
out_svg="$OUT_DIR/$base.svg"
poach_bin="$REPO_ROOT/target/profiling/poach"


echo "Generating flamegraph for $EGG_FILE"

perf record -F 999 --call-graph fp -- "$poach_bin" "$EGG_FILE" nightly/raw timeline-only

perf script --demangle | rustfilt | "$REPO_ROOT/FlameGraph/stackcollapse-perf.pl" | "$REPO_ROOT/FlameGraph/flamegraph.pl" > "$out_svg"

echo "done"
