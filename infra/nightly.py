import os
import subprocess
import shutil
from pathlib import Path
import transform

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

  rust_path = Path.home() / ".cargo" / "bin"
  os.environ["PATH"] = f"{rust_path}:{os.environ['PATH']}"
  run_cmd(["rustup", "update"])

  # determine location of this script
  script_dir = Path(__file__).resolve().parent

  # Absolute directory paths
  top_dir = script_dir.parent
  resource_dir = script_dir / "nightly-resources"
  nightly_dir = top_dir / "nightly"

  print(top_dir)
  print(resource_dir)
  print(nightly_dir)

  # Make sure we're in the right place
  os.chdir(top_dir)

  # Build poach
  run_cmd(["cargo", "build", "--release"])

  # Clean previous nightly run
  if nightly_dir.exists():
    shutil.rmtree(nightly_dir)

  # Prepare output directories
  (nightly_dir / "raw").mkdir(parents = True, exist_ok = False)
  (nightly_dir / "output").mkdir(parents = True, exist_ok = False)

  # Iterate through each benchmark suite:
  timeline_suites = ["easteregg", "herbie-hamming", "herbie-math-rewrite", "herbie-math-taylor"]
  for suite in timeline_suites:
    run_poach(resource_dir / "test-files" / suite, nightly_dir / "raw" / suite, "timeline-only")

  no_io_suites = ["easteregg", "herbie-hamming", "herbie-math-rewrite"] # herbie-math-taylor runs out of memory
  for suite in timeline_suites:
    run_poach(resource_dir / "test-files" / suite, nightly_dir / "raw" / suite, "no-io")

  # Run the egglog tests under each serialization experiemntal treatment:
  run_poach(top_dir / "tests", nightly_dir / "raw" / "tests", "timeline-only")
  run_poach(top_dir / "tests", nightly_dir / "raw" / "tests", "sequential-round-trip")
  run_poach(top_dir / "tests", nightly_dir / "raw" / "tests", "interleaved-round-trip")
  run_poach(top_dir / "tests", nightly_dir / "raw" / "tests", "idempotent-round-trip")
  run_poach(top_dir / "tests", nightly_dir / "raw" / "tests", "old-serialize")
  run_poach(top_dir / "tests", nightly_dir / "raw" / "tests", "no-io")

  # Post-process timeline data
  transform.transform((nightly_dir / "raw"), (nightly_dir / "output" / "data"))

  # Update HTML index page
  shutil.copytree(resource_dir / "web", nightly_dir / "output", dirs_exist_ok = True)