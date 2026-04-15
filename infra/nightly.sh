#!/bin/bash
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

cleanup() {
  echo "Cleaning up"
  rm -rf "$REPO_ROOT/nightly/tmp"
}
trap cleanup EXIT

echo "Beginning POACH nightly script..."

###############################################################################
# This script generates the data for the nightly frontend 
###############################################################################

export PATH=~/.cargo/bin:$PATH

rustup update

cargo install rustfilt

# Ensure we start from a clean slate
rm -rf nightly

# Set Up
mkdir -p nightly/output
mkdir -p nightly/tmp

# TODO: use real benchmarks
# git clone https://github.com/ajpal/poach-benchmarks.git

# Build in release mode before running nightly.py
cargo build --release

# This script runs all of the benchmarks/experiments
python3 infra/nightly.py

# Abort if nightly.py failed to produce data.json
if [ ! -f nightly/output/data/data.json ]; then
  echo "ERROR: nightly/output/data/data.json was not generated."
  exit 1
fi

cp infra/nightly-resources/web/* nightly/output

# Uncomment for local development
cd nightly/output && python3 -m http.server 8002
