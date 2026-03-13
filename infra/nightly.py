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

def add_benchmark_data(aggregator, timeline_file, benchmark_key):
  if timeline_file.exists():
    aggregator.add_file(timeline_file, benchmark_key)

def remove_file(path):
  if path.exists():
    path.unlink()

def cleanup_benchmark_files(*paths):
  for path in paths:
    remove_file(path)

def benchmark_files(input_dir, recursive = False):
  pattern = "**/*.egg" if recursive else "*.egg"
  return sorted(input_dir.glob(pattern))

def run_timeline_experiments(resource_dir, tmp_dir, aggregator):
  timeline_suites = ["easteregg", "herbie-hamming", "herbie-math-rewrite", "herbie-math-taylor"]
  for suite in timeline_suites:
    for benchmark in benchmark_files(resource_dir / "test-files" / suite):
      timeline_file = tmp_dir / f"{benchmark.stem}-timeline.json"
      run_poach(benchmark, tmp_dir, "timeline-only")
      add_benchmark_data(aggregator, timeline_file, f"{suite}/timeline/{benchmark.stem}/timeline.json")
      cleanup_benchmark_files(timeline_file, tmp_dir / "summary.json")

def run_no_io_experiments(resource_dir, tmp_dir, aggregator):
  no_io_suites = ["easteregg", "herbie-hamming", "herbie-math-rewrite"] # herbie-math-taylor runs out of memory
  for suite in no_io_suites:
    for benchmark in benchmark_files(resource_dir / "test-files" / suite):
      timeline_file = tmp_dir / f"{benchmark.stem}-timeline.json"
      run_poach(benchmark, tmp_dir, "no-io")
      add_benchmark_data(aggregator, timeline_file, f"{suite}/no-io/{benchmark.stem}/timeline.json")
      cleanup_benchmark_files(timeline_file, tmp_dir / "summary.json")

def run_test_experiments(top_dir, tmp_dir, aggregator):
  test_modes = [
    ("timeline", "timeline-only"),
    ("sequential", "sequential-round-trip"),
    ("old-serialize", "old-serialize"),
    ("no-io", "no-io"),
    ("extract", "extract"),
  ]
  for benchmark_name, run_mode in test_modes:
    for benchmark in benchmark_files(top_dir / "tests", recursive = True):
      timeline_file = tmp_dir / f"{benchmark.stem}-timeline.json"
      run_poach(benchmark, tmp_dir, run_mode)
      add_benchmark_data(aggregator, timeline_file, f"tests/{benchmark_name}/{benchmark.stem}/timeline.json")
      extra_files = {
        "sequential-round-trip": [tmp_dir / f"{benchmark.stem}-serialize1.json"],
        "old-serialize": [
          tmp_dir / f"{benchmark.stem}-serialize-poach.json",
          tmp_dir / f"{benchmark.stem}-serialize-old.json",
        ],
      }.get(run_mode, [])
      cleanup_benchmark_files(timeline_file, tmp_dir / "summary.json", *extra_files)

def run_mined_experiments(resource_dir, tmp_dir, aggregator):
  mega_serialize_file = tmp_dir / "mega-easteregg-serialize.json"
  mega_timeline_file = tmp_dir / "mega-easteregg-timeline.json"
  run_poach(resource_dir / "mega-easteregg.egg", tmp_dir, "serialize")
  add_benchmark_data(aggregator, mega_timeline_file, "easteregg/serialize/mega-easteregg/timeline.json")
  cleanup_benchmark_files(mega_timeline_file, tmp_dir / "summary.json")
  for benchmark in benchmark_files(resource_dir / "test-files" / "easteregg"):
    timeline_file = tmp_dir / f"{benchmark.stem}-timeline.json"
    serialize_file = tmp_dir / f"{benchmark.stem}-serialize.json"
    run_poach(benchmark, tmp_dir, "serialize")
    add_benchmark_data(aggregator, timeline_file, f"easteregg/serialize/{benchmark.stem}/timeline.json")
    cleanup_benchmark_files(timeline_file, tmp_dir / "summary.json")

    run_poach(benchmark, tmp_dir, "mine",
      ["--initial-egraph=" + str(tmp_dir)])
    add_benchmark_data(aggregator, timeline_file, f"easteregg/mine-indiv/{benchmark.stem}/timeline.json")
    cleanup_benchmark_files(timeline_file, serialize_file, tmp_dir / "summary.json")

    run_poach(benchmark, tmp_dir, "mine",
      ["--initial-egraph=" + str(mega_serialize_file)])
    add_benchmark_data(aggregator, timeline_file, f"easteregg/mine-mega/{benchmark.stem}/timeline.json")
    cleanup_benchmark_files(timeline_file, tmp_dir / "summary.json")

  cleanup_benchmark_files(mega_serialize_file, tmp_dir / "summary.json")

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

  # ##############################################################################
  # #                          run experiments                                   #
  # ##############################################################################

  # # Run the benchmarks and record timeline-only data.
  # run_timeline_experiments(resource_dir, tmp_dir, aggregator)

  # # Re-run the benchmarks with JSON round-tripping kept entirely in memory.
  # run_no_io_experiments(resource_dir, tmp_dir, aggregator)

  # # Run the egglog tests under each serialization experiment mode.
  # run_test_experiments(top_dir, tmp_dir, aggregator)

  # # Run the mined-egraph experiment using both per-benchmark and mega-egraph seeds.
  # run_mined_experiments(resource_dir, tmp_dir, aggregator)

  # ##############################################################################

  # aggregator.save()

  if shutil.which("perf") is not None:
    # Generate flamegraphs
    for egg_file in [
      # high extract
      "easteregg/Zen_News__layer_0.egg",
      "herbie-hamming/rewrite73.egg",
      "herbie-hamming/rewrite102.egg",
      "herbie-hamming/rewrite103.egg",
      "herbie-hamming/taylor17.egg",
      "herbie-math-rewrite/rewrite60.egg",
      "herbie-math-rewrite/rewrite116.egg",
      "herbie-math-taylor/taylor40.egg",
      # low extract
      "herbie-hamming/rewrite59.egg",
      "herbie-hamming/rewrite67.egg",
      "herbie-hamming/rewrite53.egg",
      "herbie-hamming/taylor85.egg",
      "herbie-math-rewrite/rewrite180.egg",
      "herbie-math-rewrite/rewrite172.egg",
      "herbie-math-rewrite/rewrite122.egg",
      "herbie-math-taylor/taylor22.egg"
      # timeout
      "pathological/herbie_arrays_timeout.egg"
      ]:
      run_cmd([
        str(script_dir / "flamegraph.sh"),
        str("infra/nightly-resources/test-files/" + egg_file),
        str(nightly_dir / "output" / "flamegraphs")])

    # Generate perf records and aggregate perf summaries.
    perf_targets = {
      "root_symbol": "run_extract_command",
      "callee_symbols": [
        "extract_variants_with_sort",
        "compute_costs_from_rootsorts",
        "for_each",
        "scan_bounded",
        "reconstruct_termdag_node_helper",
        "compute_cost_hyperedge"
      ]
    }
    perf_out_dir = nightly_dir / "raw" / "perf"
    perf_input_dir = resource_dir / "test-files"
    for egg_file in perf_input_dir.rglob("*.egg"):
      relative_parent = egg_file.relative_to(perf_input_dir).parent
      out_dir = perf_out_dir / relative_parent
      run_cmd([str(script_dir / "perf.sh"), str(egg_file), str(out_dir)])

    perf_summary_cmd = [
      "cargo",
      "run",
      "--release",
      "--bin",
      "perf_analyze",
      "--",
      str(perf_out_dir),
      "--out",
      str(perf_out_dir / "perf-summary.json"),
      "--root-symbol",
      perf_targets["root_symbol"]
    ]
    for callee_symbol in perf_targets["callee_symbols"]:
      perf_summary_cmd.extend(["--callee-symbol", callee_symbol])

    run_cmd(perf_summary_cmd)
