#!/usr/bin/env python3

from __future__ import annotations

import json
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parent.parent
NIGHTLY_DIR = REPO_ROOT / "nightly"
OUTPUT_DIR = NIGHTLY_DIR / "output"
DATA_DIR = OUTPUT_DIR / "data"
REPORT_OUTPUT_DIR = DATA_DIR / "reports"
DATA_JSON_PATH = DATA_DIR / "data.json"
POACH_BIN = REPO_ROOT / "target" / "release" / "poach"


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit(f"Usage: {Path(sys.argv[0]).name} <benchmark-dir>")

    benchmark_dir = (REPO_ROOT / sys.argv[1]).resolve()

    benchmark_files = sorted(benchmark_dir.rglob("*.egg"))
    if not benchmark_files:
        raise SystemExit(
            f"No .egg benchmark files found under {benchmark_dir}."
        )

    command_results = run_benchmarks(benchmark_dir)
    data = aggregate_reports(benchmark_dir, command_results)

    DATA_DIR.mkdir(parents=True, exist_ok=True)
    DATA_JSON_PATH.write_text(json.dumps(data, indent=2), encoding="utf-8")
    print(f"Wrote {DATA_JSON_PATH}")

def run_command(command: list[str], *, cwd: Path) -> dict[str, Any]:
    started = time.perf_counter()
    try:
        completed = subprocess.run(command, check=True, cwd=cwd)
    except subprocess.CalledProcessError as err:
        time_seconds = time.perf_counter() - started
        print(
            f"Command failed after {time_seconds:.3f}s: {' '.join(command)}",
            file=sys.stderr,
        )
        raise err

    # TODO: collect stderr and stdout from completed

    return {
        "argv": command,
        "cwd": str(cwd),
        "returncode": completed.returncode,
        "time_seconds": time.perf_counter() - started,
    }


def run_benchmarks(benchmark_dir: Path) -> list[dict[str, Any]]:
    if REPORT_OUTPUT_DIR.exists():
        shutil.rmtree(REPORT_OUTPUT_DIR)
    REPORT_OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    # TODO: poach serve streaming is the default mode

    command = [
        str(POACH_BIN),         # poach binary
                                # fill in other args
        str(benchmark_dir),     # input files
    ]
    print("Running benchmarks:", " ".join(command))
    return [run_command(command, cwd=REPO_ROOT)]


def aggregate_reports(
    benchmark_dir: Path, command_results: list[dict[str, Any]]
) -> dict[str, Any]:
    report_files = sorted(REPORT_OUTPUT_DIR.rglob("*.report.json"))
    if not report_files:
        raise SystemExit(f"No report files were generated under {REPORT_OUTPUT_DIR}")

    return {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "suite": benchmark_dir.name,
        "benchmark_root": str(benchmark_dir.relative_to(REPO_ROOT)),
        "summary": {
            "commands": command_results,
            "total_time_seconds": sum(
                result["time_seconds"] for result in command_results
            ),
        },
    }


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as err:
        print(f"Command failed with exit code {err.returncode}: {err.cmd}", file=sys.stderr)
        raise
