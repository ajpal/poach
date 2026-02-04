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

  # determine location of this script
  script_dir = Path(__file__).resolve().parent

  # Absolute directory paths
  top_dir = script_dir.parent
  resource_dir = script_dir / "nightly-resources"
  nightly_dir = top_dir / "nightly"

  # Make sure we're in the right place
  os.chdir(top_dir)

  # Iterate through each benchmark suite:
  timeline_suites = ["easteregg", "herbie-hamming", "herbie-math-rewrite", "herbie-math-taylor"]
  for suite in timeline_suites:
    run_poach(resource_dir / "test-files" / suite, nightly_dir / "raw" / suite / "timeline", "timeline-only")

  no_io_suites = ["easteregg", "herbie-hamming", "herbie-math-rewrite"] # herbie-math-taylor runs out of memory
  for suite in no_io_suites:
    run_poach(resource_dir / "test-files" / suite, nightly_dir / "raw" / suite, "no-io")

  # Run the egglog tests under each serialization experiemntal treatment:
  run_poach(top_dir / "tests", nightly_dir / "raw" / "tests" / "timeline", "timeline-only")
  run_poach(top_dir / "tests", nightly_dir / "raw" / "tests" / "sequential", "sequential-round-trip")
  run_poach(top_dir / "tests", nightly_dir / "raw" / "tests" / "old-serialize", "old-serialize")
  run_poach(top_dir / "tests", nightly_dir / "raw" / "tests" / "no-io", "no-io")
  run_poach(top_dir / "tests", nightly_dir / "raw" / "tests" / "extract", "extract")

  # Mined POACH Experiment
  # precompute
  run_poach(resource_dir / "mega-easteregg.egg", nightly_dir / "raw" / "easteregg" / "serialize", "serialize")
  run_poach(resource_dir / "test-files" / "easteregg", nightly_dir / "raw" / "easteregg" / "serialize", "serialize")
  # mined
  run_poach(resource_dir / "test-files" / "easteregg", nightly_dir / "raw" / "easteregg" / "mine-indiv", "mine",
    ["--initial-egraph=" + str(nightly_dir / "raw" / "easteregg" / "serialize" )])
  run_poach(resource_dir / "test-files" / "easteregg", nightly_dir / "raw" / "easteregg" / "mine-mega", "mine",
    ["--initial-egraph=" + str(nightly_dir / "raw" / "easteregg" / "serialize" / "mega-easteregg" / "serialize.json" ), "--allow-let"])

  # Post-process timeline data
  transform.transform((nightly_dir / "raw"), (nightly_dir / "output" / "data"))

  if shutil.which("perf") is not None:
    # Generate flamegraphs
    for egg_file in glob.glob("tests/*.egg") + glob.glob("tests/web-demo/*.egg"):
      run_cmd([str(script_dir / "flamegraph.sh"), egg_file, str(nightly_dir / "output" / "flamegraphs")])
