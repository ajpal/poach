#!/usr/bin/env bash

# Record perf data for a single egg file.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

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
out_perf="$OUT_DIR/$base.perf.data"
poach_bin="$REPO_ROOT/target/profiling/poach"

echo "Recording perf data for $EGG_FILE"

perf record -o "$out_perf" -F 9999 --call-graph fp -- "$poach_bin" "$EGG_FILE" nightly/raw/perf timeline-only

echo "Wrote $out_perf"
