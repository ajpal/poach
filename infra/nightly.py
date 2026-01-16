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

def run_poach(in_dir, out_dir, run_mode):
  poach_exe = top_dir / "target" / "release" / "poach"
  run_cmd([str(poach_exe), str(in_dir), str(out_dir), run_mode])

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
    run_poach(resource_dir / "test-files" / suite, nightly_dir / "raw" / suite, "timeline-only")

  # no_io_suites = ["easteregg", "herbie-hamming", "herbie-math-rewrite"] # herbie-math-taylor runs out of memory
  # for suite in no_io_suites:
  #   run_poach(resource_dir / "test-files" / suite, nightly_dir / "raw" / suite, "no-io")

  # # Run the egglog tests under each serialization experiemntal treatment:
  # run_poach(top_dir / "tests", nightly_dir / "raw" / "tests", "timeline-only")
  # run_poach(top_dir / "tests", nightly_dir / "raw" / "tests", "sequential-round-trip")
  # run_poach(top_dir / "tests", nightly_dir / "raw" / "tests", "interleaved-round-trip")
  # run_poach(top_dir / "tests", nightly_dir / "raw" / "tests", "idempotent-round-trip")
  # run_poach(top_dir / "tests", nightly_dir / "raw" / "tests", "old-serialize")
  # run_poach(top_dir / "tests", nightly_dir / "raw" / "tests", "no-io")

  # Post-process timeline data
  transform.transform((nightly_dir / "raw"), (nightly_dir / "output" / "data"))

  # Run benchmark suites with Knuth extraction
  for suite in timeline_suites:
    poach_exe = top_dir / "target" / "release" / "poach"
    in_dir = resource_dir / "test-files" / suite
    out_dir = nightly_dir / "raw" / "kd" / suite
    run_cmd(["env", "EXTRACTION=KD", str(poach_exe), str(in_dir), str(out_dir), "timeline-only"])
  transform.transform((nightly_dir / "raw" / "kd"), (nightly_dir / "output" / "data" / "kd"))

  # Flamegraphs for Knuth extraction
  for egg_file in [f for f in glob.glob(f"infra/nightly-resources/test-files/{suite}") for suite in timeline_suites]:
    run_cmd(["env", "EXTRACTION=KD", str(script_dir / "flamegraph.sh"), egg_file, str(nightly_dir / "output" / "flamegraphs")])

  # Update HTML index page
  shutil.copytree(resource_dir / "web", nightly_dir / "output", dirs_exist_ok = True)