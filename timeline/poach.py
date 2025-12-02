import subprocess
from pathlib import Path

def flamegraph_all(input_dir, output_dir, no_serialize):
  # List only files
  files = [f for f in input_dir.rglob('*.egg')]
  total = len(files)

  for idx, input_file in enumerate(files, start=1):
    # Create output directory for this file
    output_dir = output_dir / input_file.stem
    output_dir.mkdir(parents=True, exist_ok=True)

    # Output flamegraph path
    flamegraph_path = output_dir / "flamegraph.svg"

    # Run cargo flamegraph
    cmd = [
        "cargo", "flamegraph",
        "--output", str(flamegraph_path),
        "--bin", "poach",
        "--", str(input_file),
        str(output_dir)
    ]

    if no_serialize:
      cmd.append("--no-serialize")

    print(f"[{idx}/{total}]: {' '.join(cmd)}")
    subprocess.run(cmd, check=True)

if __name__ == "__main__":
  import argparse

  parser = argparse.ArgumentParser(description="run Poach and ")
  parser.add_argument("input_dir", type=Path, help="Directory containing input files")
  parser.add_argument("output_dir", type=Path, help="Base output directory for flamegraphs and timelines")
  parser.add_argument(
      "--no-serialize",
      action="store_true",
      help="Pass --no-serialize to the poach call"
  )


  args = parser.parse_args()
  flamegraph_all(args.input_dir, args.output_dir, args.no_serialize)