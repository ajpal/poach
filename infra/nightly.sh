#!/bin/bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

echo "Beginning POACH combined nightly script..."

###############################################################################
# Drives the per-branch nightly scripts on each comparison branch, then
# merges their data.json outputs into a single unified data.json for the
# combined frontend. Add a branch name to BRANCHES to extend.
###############################################################################

export PATH=~/.cargo/bin:$PATH

BRANCHES=(
  ajpal-vanilla-nightly # change to PTP_VanillaEgglog once #51 merges
  ajpal-serialize-nightly # change to PTP_SerializeEgraph once #52 merges
)

# Ensure we start from a clean slate
rm -rf nightly
mkdir -p nightly/output/data nightly/tmp

# Top-level setup runs once. Per-branch nightly.sh scripts skip their own
# setup because we set POACH_NIGHTLY_COMBINED below.
rustup update
cargo install rustfilt

BENCHMARKS_DIR="$REPO_ROOT/nightly/tmp/poach-benchmarks"
git clone https://github.com/ajpal/poach-benchmarks.git "$BENCHMARKS_DIR"

WORKTREE_BASE="$REPO_ROOT/nightly/tmp/worktrees"
mkdir -p "$WORKTREE_BASE"

# Register worktree cleanup so it runs even if the script aborts partway
# Otherwise, subsequent nightly runs will fail if the worktree is already present.
cleanup_worktrees() {
  for branch in "${BRANCHES[@]}"; do
    git worktree remove --force "$WORKTREE_BASE/$branch" 2>/dev/null || true
  done
}
trap cleanup_worktrees EXIT

# Run nightly.sh for each branch
for branch in "${BRANCHES[@]}"; do
  echo ""
  echo "=== Running nightly for branch: $branch ==="
  worktree_dir="$WORKTREE_BASE/$branch"
  # --detach so multiple worktrees can sit on the same commit without
  # claiming the branch (the nightly runner already holds the branch lock
  # in a sibling directory).
  git worktree add --detach "$worktree_dir" "$branch"

  (
    cd "$worktree_dir"
    POACH_NIGHTLY_COMBINED=1 \
    POACH_BENCHMARKS_DIR="$BENCHMARKS_DIR" \
    bash infra/nightly.sh
  )

  branch_data_json="$worktree_dir/nightly/output/data/data.json"
  if [ ! -f "$branch_data_json" ]; then
    echo "ERROR: $branch did not produce $branch_data_json" >&2
    exit 1
  fi

  cp -R "$worktree_dir/nightly/output/data" "nightly/output/data/$branch"
done

# Merge per-branch data.json files into a single unified data.json.
python3 infra/merge.py "${BRANCHES[@]}"

if [ ! -f nightly/output/data/data.json ]; then
  echo "ERROR: nightly/output/data/data.json was not generated."
  exit 1
fi

cp infra/nightly-resources/web/* nightly/output

# Uncomment for local development
# cd nightly/output && python3 -m http.server 8002
