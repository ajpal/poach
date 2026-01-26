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

def run_poach(in_dir, out_dir, run_mode, max_benchmarks = None):
  poach_exe = top_dir / "target" / "release" / "poach"
  run_cmd([str(poach_exe), str(in_dir), str(out_dir), run_mode,
    "" if max_benchmarks == None else "--max-benchmarks=" + str(max_benchmarks)])

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
  all_suites = ["easteregg", "herbie-hamming", "herbie-math-rewrite", "herbie-math-taylor"]
  for suite in all_suites:
    run_poach(resource_dir / "test-files" / suite, nightly_dir / "raw" / suite, "timeline-only")

    # Sample benchmarks in each suite for performance reasons
    num_samples = 10
    run_poach(resource_dir / "test-files" / suite, nightly_dir / "raw" / suite, "sequential-round-trip", num_samples)
    run_poach(resource_dir / "test-files" / suite, nightly_dir / "raw" / suite, "idempotent-round-trip", num_samples)
    run_poach(resource_dir / "test-files" / suite, nightly_dir / "raw" / suite, "no-io", num_samples)
    run_poach(resource_dir / "test-files" / suite, nightly_dir / "raw" / suite, "extract", num_samples)

  # Run the egglog tests under each serialization experiemntal treatment:
  run_poach(top_dir / "tests", nightly_dir / "raw" / "tests", "timeline-only")
  run_poach(top_dir / "tests", nightly_dir / "raw" / "tests", "sequential-round-trip")
  run_poach(top_dir / "tests", nightly_dir / "raw" / "tests", "interleaved-round-trip")
  run_poach(top_dir / "tests", nightly_dir / "raw" / "tests", "idempotent-round-trip")
  run_poach(top_dir / "tests", nightly_dir / "raw" / "tests", "old-serialize")
  run_poach(top_dir / "tests", nightly_dir / "raw" / "tests", "no-io")
  run_poach(top_dir / "tests", nightly_dir / "raw" / "tests", "extract")

  # Post-process timeline data
  transform.transform((nightly_dir / "raw"), (nightly_dir / "output" / "data"))

  # Generate flamegraphs
  for egg_file in glob.glob("tests/*.egg") + glob.glob("tests/web-demo/*.egg"):
    run_cmd([str(script_dir / "flamegraph.sh"), egg_file, str(nightly_dir / "output" / "flamegraphs")])
  if shutil.which("perf") is not None:
    # Generate flamegraphs
    for egg_file in glob.glob("tests/*.egg") + glob.glob("tests/web-demo/*.egg"):
      run_cmd([str(script_dir / "flamegraph.sh"), egg_file, str(nightly_dir / "output" / "flamegraphs")])
