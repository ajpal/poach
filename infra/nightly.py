import os
import subprocess
import shutil
from pathlib import Path
import transform

###############################################################################
# IMPORTANT:
# In order, for this script to run successfully, requires:
# 1. https://github.com/brendangregg/FlameGraph is located in the directory
# above this script.
# 2. Directories nightly/output, nightly/raw, and nightly/flamegraphs exist
# 3. Poach is built in profiling mode at target/profiling/poach
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

  # Make sure we're in the right place
  os.chdir(top_dir)

  # # Iterate through each benchmark suite:
  # timeline_suites = ["easteregg", "herbie-hamming", "herbie-math-rewrite", "herbie-math-taylor"]
  # for suite in timeline_suites:
  #   run_poach(resource_dir / "test-files" / suite, nightly_dir / "raw" / suite / "timeline", "timeline-only")

  # no_io_suites = ["easteregg", "herbie-hamming", "herbie-math-rewrite"] # herbie-math-taylor runs out of memory
  # for suite in no_io_suites:
  #   run_poach(resource_dir / "test-files" / suite, nightly_dir / "raw" / suite / "no-io", "no-io")

  # # Run the egglog tests under each serialization experiemntal treatment:
  # run_poach(top_dir / "tests", nightly_dir / "raw" / "tests" / "timeline", "timeline-only")
  # run_poach(top_dir / "tests", nightly_dir / "raw" / "tests" / "sequential", "sequential-round-trip")
  # run_poach(top_dir / "tests", nightly_dir / "raw" / "tests" / "old-serialize", "old-serialize")
  # run_poach(top_dir / "tests", nightly_dir / "raw" / "tests" / "no-io", "no-io")
  # run_poach(top_dir / "tests", nightly_dir / "raw" / "tests" / "extract", "extract")

  # # Mined POACH Experiment
  # # precompute
  # run_poach(resource_dir / "mega-easteregg.egg", nightly_dir / "raw" / "easteregg" / "serialize", "serialize")
  # run_poach(resource_dir / "test-files" / "easteregg", nightly_dir / "raw" / "easteregg" / "serialize", "serialize")
  # # mined
  # run_poach(resource_dir / "test-files" / "easteregg", nightly_dir / "raw" / "easteregg" / "mine-indiv", "mine",
  #   ["--initial-egraph=" + str(nightly_dir / "raw" / "easteregg" / "serialize" )])
  # run_poach(resource_dir / "test-files" / "easteregg", nightly_dir / "raw" / "easteregg" / "mine-mega", "mine",
  #   ["--initial-egraph=" + str(nightly_dir / "raw" / "easteregg" / "serialize" / "mega-easteregg" / "serialize.json" )])

  # # Post-process timeline data
  # transform.transform((nightly_dir / "raw"), (nightly_dir / "output" / "data"))

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
      ]:
      run_cmd([
        str(script_dir / "flamegraph.sh"),
        str("infra/nightly-resources/test-files/" + egg_file),
        str(nightly_dir / "output" / "flamegraphs")])

    # Generate perf records
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
    perf_input_dir = Path("infra/nightly-resources/test-files")
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
