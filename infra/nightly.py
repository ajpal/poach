import os
import subprocess
import shutil
from pathlib import Path
import transform
import glob

###############################################################################
# IMPORTANT:
# In order, for this script to run successfully, requires:
# 1. https://github.com/brendangregg/FlameGraph is located in the directory
# above this script.
# 2. Directories nightly/output, nightly/raw, and nightly/flamegraphs exist
# 3. Poach is built in release mode at target/release/poach
# 4. rustfilt is installed
###############################################################################

def run_cmd(cmd, msg = "", dry_run = False):
  prefix = "[DRY_RUN]" if dry_run else "[RUN]"
  cmd_str = " ".join(cmd)
  print(f"{prefix} {msg} {cmd_str}")
  if not dry_run:
    subprocess.run(cmd, check = True)

def run_poach(in_dir, out_dir, run_mode, extra_args = [], dry_run = False):
  prefix = "[DRY_RUN]" if dry_run else "[RUN]"
  cmd = [
    "cargo",
    "run",
    "--release",
    "--bin",
    "poach",
    "--",
    str(in_dir),
    str(out_dir),
    run_mode
  ] + extra_args

  print(f"{prefix} {' '.join(cmd)}")
  if not dry_run:
    subprocess.run(cmd, check = True)

def transform_and_cleanup(raw_dir, output_dir, benchmark_dir):
  if any(benchmark_dir.rglob("timeline.json")):
    transform.transform(benchmark_dir, output_dir, relative_to = raw_dir)
  shutil.rmtree(benchmark_dir, ignore_errors = True)
  for parent in benchmark_dir.parents:
    if parent == raw_dir:
      break
    try:
      parent.rmdir()
    except OSError:
      break

def benchmark_files(input_dir, recursive = False):
  pattern = "**/*.egg" if recursive else "*.egg"
  return sorted(input_dir.glob(pattern))

if __name__ == "__main__":
  print("Beginning poach nightly")

  # Suppress egglog warnings (only show errors)
  os.environ["RUST_LOG"] = "error"

  # determine location of this script
  script_dir = Path(__file__).resolve().parent

  # Absolute directory paths
  top_dir = script_dir.parent
  resource_dir = script_dir / "nightly-resources"
  nightly_dir = top_dir / "nightly"
  raw_dir = nightly_dir / "raw"
  output_data_dir = nightly_dir / "output" / "data"

  # Make sure we're in the right place
  os.chdir(top_dir)

  # Iterate through each benchmark suite:
  timeline_suites = ["easteregg", "herbie-hamming", "herbie-math-rewrite", "herbie-math-taylor"]
  for suite in timeline_suites:
    mode_dir = raw_dir / suite / "timeline"
    for benchmark in benchmark_files(resource_dir / "test-files" / suite):
      run_poach(benchmark, mode_dir, "timeline-only")
      transform_and_cleanup(raw_dir, output_data_dir, mode_dir / benchmark.stem)

  no_io_suites = ["easteregg", "herbie-hamming", "herbie-math-rewrite"] # herbie-math-taylor runs out of memory
  for suite in no_io_suites:
    mode_dir = raw_dir / suite / "no-io"
    for benchmark in benchmark_files(resource_dir / "test-files" / suite):
      run_poach(benchmark, mode_dir, "no-io")
      transform_and_cleanup(raw_dir, output_data_dir, mode_dir / benchmark.stem)

  # Run the egglog tests under each serialization experiemntal treatment:
  test_modes = [
    ("timeline", "timeline-only"),
    ("sequential", "sequential-round-trip"),
    ("old-serialize", "old-serialize"),
    ("no-io", "no-io"),
    ("extract", "extract"),
  ]
  for benchmark_name, run_mode in test_modes:
    mode_dir = raw_dir / "tests" / benchmark_name
    for benchmark in benchmark_files(top_dir / "tests", recursive = True):
      run_poach(benchmark, mode_dir, run_mode)
      transform_and_cleanup(raw_dir, output_data_dir, mode_dir / benchmark.stem)

  # Mined POACH Experiment
  # precompute
  serialize_dir = raw_dir / "easteregg" / "serialize"
  run_poach(resource_dir / "mega-easteregg.egg", serialize_dir, "serialize")
  mine_indiv_dir = raw_dir / "easteregg" / "mine-indiv"
  mine_mega_dir = raw_dir / "easteregg" / "mine-mega"
  for benchmark in benchmark_files(resource_dir / "test-files" / "easteregg"):
    run_poach(benchmark, serialize_dir, "serialize")
    run_poach(benchmark, mine_indiv_dir, "mine",
      ["--initial-egraph=" + str(serialize_dir)])
    transform_and_cleanup(raw_dir, output_data_dir, mine_indiv_dir / benchmark.stem)

    run_poach(benchmark, mine_mega_dir, "mine",
      ["--initial-egraph=" + str(serialize_dir / "mega-easteregg" / "serialize.json")])
    transform_and_cleanup(raw_dir, output_data_dir, mine_mega_dir / benchmark.stem)
    transform_and_cleanup(raw_dir, output_data_dir, serialize_dir / benchmark.stem)

  transform_and_cleanup(raw_dir, output_data_dir, serialize_dir / "mega-easteregg")

  if shutil.which("perf") is not None:
    # Generate flamegraphs
    for egg_file in glob.glob("tests/*.egg") + glob.glob("tests/web-demo/*.egg"):
      run_cmd([str(script_dir / "flamegraph.sh"), egg_file, str(nightly_dir / "output" / "flamegraphs")])
