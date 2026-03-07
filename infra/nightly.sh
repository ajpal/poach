#!/bin/bash
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

cleanup() {
  echo "Cleaning up"
  rm -rf "$REPO_ROOT/FlameGraph"
  rm -rf "$REPO_ROOT/nightly/tmp"
}
trap cleanup EXIT

echo "Beginning POACH nightly script..."

###############################################################################
# This script generates the data for the nightly frontend 
# 
# Expected structure after running this script:
# nightly/output/data will contain data.json, the big blob of data that the
# frontend uses to show graphs/tables.

# nightly/output/flamegraphs will contain an svg flamegraph for each benchmark

# nightly/output will contain several HTML and JS files for rendering the
# frontend correctly. These are copied over from infra/nightly-resources/web
# on each execution.

# Temporary files:
# nightly/tmp is used for intermediate files (mostly serialized egraphs).
# Benchmark outputs are deleted as each benchmark completes, and any leftovers
# are removed on exit so that these files do not clutter the nightly machine.

# FlameGraph/ is a repo for generating flamegraphs. It is cloned and removed
# on each execution of this script.
###############################################################################

export PATH=~/.cargo/bin:$PATH

rustup update

cargo install rustfilt

# Ensure we start from a clean slate
rm -rf FlameGraph
rm -rf nightly

# Set Up
mkdir -p nightly/output
mkdir -p nightly/output/flamegraphs
mkdir -p nightly/tmp

# Skip FlameGraphs for mining MVP
# git clone https://github.com/brendangregg/FlameGraph.git

# Build in release mode before running nightly.py
cargo build --release

# This script runs all of the benchmarks/experiments and generates flamegraphs
python3 infra/nightly.py

# Abort if nightly.py failed to produce data.json
if [ ! -f nightly/output/data/data.json ]; then
  echo "ERROR: nightly/output/data/data.json was not generated."
  exit 1
fi

ls nightly/output/flamegraphs > nightly/output/flamegraphs.txt

cp infra/nightly-resources/web/* nightly/output

# Uncomment for local development
# cd nightly/output && python3 -m http.server 8002
