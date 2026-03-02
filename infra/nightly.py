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
# 2. Directories nightly/output, nightly/tmp, and nightly/flamegraphs exist
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

def add_benchmark_data(aggregator, benchmark_dir, benchmark_key):
  timeline_file = benchmark_dir / "timeline.json"
  if timeline_file.exists():
    aggregator.add_file(timeline_file, benchmark_key)

def remove_timeline_file(benchmark_dir):
  timeline_file = benchmark_dir / "timeline.json"
  if timeline_file.exists():
    timeline_file.unlink()

def cleanup_benchmark_dir(tmp_dir, benchmark_dir):
  shutil.rmtree(benchmark_dir, ignore_errors = True)
  for parent in benchmark_dir.parents:
    if parent == tmp_dir:
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
  tmp_dir = nightly_dir / "tmp"
  output_data_dir = nightly_dir / "output" / "data"
  aggregator = transform.TimelineAggregator(output_data_dir)

  # Make sure we're in the right place
  os.chdir(top_dir)

  # Iterate through each benchmark suite:
  timeline_suites = ["easteregg", "herbie-hamming", "herbie-math-rewrite", "herbie-math-taylor"]
  for suite in timeline_suites:
    for benchmark in benchmark_files(resource_dir / "test-files" / suite):
      benchmark_dir = tmp_dir / benchmark.stem
      run_poach(benchmark, tmp_dir, "timeline-only")
      add_benchmark_data(aggregator, benchmark_dir, f"{suite}/timeline/{benchmark.stem}/timeline.json")
      cleanup_benchmark_dir(tmp_dir, benchmark_dir)

  no_io_suites = ["easteregg", "herbie-hamming", "herbie-math-rewrite"] # herbie-math-taylor runs out of memory
  for suite in no_io_suites:
    for benchmark in benchmark_files(resource_dir / "test-files" / suite):
      benchmark_dir = tmp_dir / benchmark.stem
      run_poach(benchmark, tmp_dir, "no-io")
      add_benchmark_data(aggregator, benchmark_dir, f"{suite}/no-io/{benchmark.stem}/timeline.json")
      cleanup_benchmark_dir(tmp_dir, benchmark_dir)

  # Run the egglog tests under each serialization experiemntal treatment:
  test_modes = [
    ("timeline", "timeline-only"),
    ("sequential", "sequential-round-trip"),
    ("old-serialize", "old-serialize"),
    ("no-io", "no-io"),
    ("extract", "extract"),
  ]
  for benchmark_name, run_mode in test_modes:
    for benchmark in benchmark_files(top_dir / "tests", recursive = True):
      benchmark_dir = tmp_dir / benchmark.stem
      run_poach(benchmark, tmp_dir, run_mode)
      add_benchmark_data(aggregator, benchmark_dir, f"tests/{benchmark_name}/{benchmark.stem}/timeline.json")
      cleanup_benchmark_dir(tmp_dir, benchmark_dir)

  # Mined POACH Experiment
  # precompute
  mega_dir = tmp_dir / "mega-easteregg"
  run_poach(resource_dir / "mega-easteregg.egg", tmp_dir, "serialize")
  add_benchmark_data(aggregator, mega_dir, "easteregg/serialize/mega-easteregg/timeline.json")
  remove_timeline_file(mega_dir)
  for benchmark in benchmark_files(resource_dir / "test-files" / "easteregg"):
    benchmark_dir = tmp_dir / benchmark.stem
    run_poach(benchmark, tmp_dir, "serialize")
    add_benchmark_data(aggregator, benchmark_dir, f"easteregg/serialize/{benchmark.stem}/timeline.json")
    remove_timeline_file(benchmark_dir)

    run_poach(benchmark, tmp_dir, "mine",
      ["--initial-egraph=" + str(tmp_dir)])
    add_benchmark_data(aggregator, benchmark_dir, f"easteregg/mine-indiv/{benchmark.stem}/timeline.json")
    remove_timeline_file(benchmark_dir)

    run_poach(benchmark, tmp_dir, "mine",
      ["--initial-egraph=" + str(mega_dir / "serialize.json")])
    add_benchmark_data(aggregator, benchmark_dir, f"easteregg/mine-mega/{benchmark.stem}/timeline.json")
    cleanup_benchmark_dir(tmp_dir, benchmark_dir)

  cleanup_benchmark_dir(tmp_dir, mega_dir)
  aggregator.save()

  if shutil.which("perf") is not None:
    # Generate flamegraphs
    for egg_file in glob.glob("tests/*.egg") + glob.glob("tests/web-demo/*.egg"):
      run_cmd([str(script_dir / "flamegraph.sh"), egg_file, str(nightly_dir / "output" / "flamegraphs")])
