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
  poach_exe = top_dir / "target" / "release" / "poach"

  # Clean previous nightly run
  if nightly_dir.exists():
    shutil.rmtree(nightly_dir)

  # Prepare output directories
  (nightly_dir / "raw").mkdir(parents = True, exist_ok = False)
  (nightly_dir / "output").mkdir(parents = True, exist_ok = False)

  # Iterate through each benchmark suite:
  dirs = [d for d in (resource_dir / "test-files").iterdir() if d.is_dir()]
  for dir_path in dirs:
    dir_name = dir_path.name
    target_dir = nightly_dir / "raw" / dir_name
    target_dir.mkdir(parents = True, exist_ok = True)
    run_cmd([str(poach_exe), str(dir_path), str(target_dir)], dry_run = False)


  # Also run the egglog tests
  run_cmd([str(poach_exe), str( top_dir / "tests"), str(nightly_dir / "raw" / "tests")], dry_run = False)

  # Post-process timeline data
  transform.transform((nightly_dir / "raw"), (nightly_dir / "output" / "data"))

  # Update HTML index page
  shutil.copytree(resource_dir / "web", nightly_dir / "output", dirs_exist_ok = True)