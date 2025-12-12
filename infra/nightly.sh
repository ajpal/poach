#!/bin/bash
# Nightly script for the egraph timeline

# python3 infra/nightly.py

echo "Beginning eggcc nightly script..."

export PATH=~/.cargo/bin:$PATH

rustup update

cargo install rustfilt

rm -rf FlameGraph
git clone https://github.com/brendangregg/FlameGraph.git

cargo build --release

rm -rf nightly
mkdir -p nightly/output/flamegraphs

touch nightly/output/flamegraphs/files.txt
for egg_file in tests/*/*.egg; do
  # If no files match, the pattern expands to itself
  [[ -f "$egg_file" ]] || continue

  echo "Processing $egg_file"
  echo "$egg_file" >> nightly/output/flamegraphs/files.txt
  ./infra/flamegraph.sh "$egg_file" nightly/output/flamegraphs
done

cp infra/nightly-resources/web/* nightly/output

rm -rf FlameGraph