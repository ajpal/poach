#!/bin/bash
# Nightly script for the egraph timeline

# python3 infra/nightly.py

echo "Beginning eggcc nightly script..."

export PATH=~/.cargo/bin:$PATH

rustup update

cargo install rustfilt

git clone https://github.com/brendangregg/FlameGraph.git

cargo build --release
perf record -F 999 --call-graph dwarf -- ./target/release/poach tests/repro-unsound.egg out sequential-round-trip ; perf script --demangle | rustfilt | ./FlameGraph/stackcollapse-perf.pl | ./FlameGraph/flamegraph.pl > flamegraph.svg

rm -rf nightly

mkdir -p nightly/output

cp infra/nightly-resources/web/* nightly/output

cp flamegraph.svg nightly/output

rm -rf FlameGraph