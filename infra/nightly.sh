#!/bin/bash
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

cleanup() {
  echo "Cleaning up"
  rm -rf "$REPO_ROOT/FlameGraph"
  rm -rf "$REPO_ROOT/nightly/raw"
}
trap cleanup EXIT

echo "Beginning POACH nightly script..."

###############################################################################
# This script generates the data for the nightly frontend 
# 
# Expected structure after running this script:
# nightly/output/data will contain two files
# 1. data.json is the big blob of data that the frontend uses to show graphs/tables
# 2. list.json is a text file containing a list of all benchmarks

# nightly/output/flamegraphs will contain an svg flamegraph for each benchmark

# nightly/output will contain several HTML and JS files for rendering the
# frontend correctly. These are copied over from infra/nightly-resources/web
# on each execution.

# Temporary files:
# nightly/raw is used for large intermediate files (mostly serialized egraphs). 
# It must be deleted so that these large files do not clutter the nightly machine.

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
mkdir -p nightly/raw

git clone https://github.com/brendangregg/FlameGraph.git

# Build profiling binary with frame pointers for stable flamegraph stacks
RUSTFLAGS="${RUSTFLAGS:-} -C force-frame-pointers=yes" cargo build --profile profiling --bin poach

# This script runs all of the benchmarks/experiments and generates flamegraphs
python3 infra/nightly.py

# Publish perf summary to nightly/output (without copying perf.data files).
if [ -f nightly/raw/perf/perf-summary.json ]; then
  mkdir -p nightly/output/perf
  cp nightly/raw/perf/perf-summary.json nightly/output/perf/perf-summary.json
else
  echo "WARNING: nightly/raw/perf/perf-summary.json was not found; skipping perf summary copy."
fi

# # Abort if nightly.py failed to produce data.json
# if [ ! -f nightly/output/data/data.json ]; then
#   echo "ERROR: nightly/output/data/data.json was not generated."
#   exit 1
# fi

ls nightly/output/flamegraphs > nightly/output/flamegraphs.txt

cp infra/nightly-resources/web/* nightly/output

# Uncomment for local development
# cd nightly/output && python3 -m http.server 8002
