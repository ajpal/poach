#!/usr/bin/env bash

# Generate a flamegraph for a single egg file
# Currently hard-coded to sequential run mode

set -euo pipefail

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


echo "Generating flamegraph for $EGG_FILE"

perf record -F 999 --call-graph dwarf -- ./target/release/poach "$EGG_FILE" nightly/raw baseline

perf script --demangle | rustfilt | ./FlameGraph/stackcollapse-perf.pl | ./FlameGraph/flamegraph.pl > "$out_svg"

echo "done"