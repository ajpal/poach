#!/bin/bash

echo "Beginning POACH nightly script..."

export PATH=~/.cargo/bin:$PATH

rustup update

cargo install rustfilt

rm -rf FlameGraph
rm -rf nightly

mkdir -p nightly/output
mkdir -p nightly/raw

git clone https://github.com/brendangregg/FlameGraph.git

cargo build --release

python3 infra/nightly.py

ls nightly/output/flamegraphs > nightly/output/flamegraphs.txt

cp infra/nightly-resources/web/* nightly/output

rm -rf FlameGraph
rm -rf nightly/raw