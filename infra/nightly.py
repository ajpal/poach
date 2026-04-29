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

    benchmark_files = sorted((benchmark_dir / "train").rglob("*.egg"))
    if not benchmark_files:
        raise SystemExit(
            f"No .egg benchmark files found under {benchmark_dir}."
        )

    command_results = run_benchmarks(benchmark_dir)
    data = aggregate_reports(benchmark_dir, command_results)

    DATA_DIR.mkdir(parents=True, exist_ok=True)
    DATA_JSON_PATH.write_text(json.dumps(data, indent=2), encoding="utf-8")
    print(f"Wrote {DATA_JSON_PATH}")

def run_command(command: list[str], *, cwd: Path, report_path: Path) -> dict[str, Any]:
    started = time.perf_counter()
    try:
        completed = subprocess.run(
            command,
            check=True,
            cwd=cwd,
            capture_output=True,
            text=True,
        )
    except subprocess.CalledProcessError as err:
        time_seconds = time.perf_counter() - started
        print(
            f"Command failed after {time_seconds:.3f}s: {' '.join(command)}",
            file=sys.stderr,
        )
        raise err

    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(completed.stderr, encoding="utf-8")

    return {
        "argv": command,
        "cwd": str(cwd),
        "report_path": str(report_path),
        "returncode": completed.returncode,
        "time_seconds": time.perf_counter() - started,
    }


def run_benchmarks(benchmark_dir: Path) -> list[dict[str, Any]]:
    if REPORT_OUTPUT_DIR.exists():
        shutil.rmtree(REPORT_OUTPUT_DIR)
    REPORT_OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    command_results = []

    for benchmark in (benchmark_dir / "train").rglob("*.egg"):
        relative_benchmark = benchmark.relative_to(benchmark_dir / "train")
        bench_out_dir = REPORT_OUTPUT_DIR / relative_benchmark.with_suffix("")
        bench_out_dir.mkdir(parents=True, exist_ok=True)

        train_command = [
            str(POACH_BIN),
            "train",
            "--debug",
            str(benchmark),
            str(bench_out_dir / "model.fbs"),
        ]
        print("Running benchmark train:", relative_benchmark.as_posix())
        train_result = run_command(
            train_command,
            cwd=REPO_ROOT,
            report_path=(bench_out_dir / "train.report.json"),
        )
        command_results.append(
            {
                "benchmark": relative_benchmark.as_posix(),
                "phase": "train",
                "argv": train_result["argv"],
                "cwd": train_result["cwd"],
                "report_path": train_result["report_path"],
                "returncode": train_result["returncode"],
                "time_seconds": train_result["time_seconds"],
            }
        )

        serve_command = [
            str(POACH_BIN),
            "serve",
            "--debug",
            str(bench_out_dir / "model.fbs"),
            "single",
            str(benchmark_dir / "serve" / relative_benchmark),
        ]
        print("Running benchmark serve:", relative_benchmark.as_posix())
        serve_result = run_command(
            serve_command,
            cwd=REPO_ROOT,
            report_path=(bench_out_dir / "serve.report.json"),
        )
        command_results.append(
            {
                "benchmark": relative_benchmark.as_posix(),
                "phase": "serve",
                "argv": serve_result["argv"],
                "cwd": serve_result["cwd"],
                "report_path": serve_result["report_path"],
                "returncode": serve_result["returncode"],
                "time_seconds": serve_result["time_seconds"],
            }
        )

    return command_results


def aggregate_reports(
    benchmark_dir: Path, command_results: list[dict[str, Any]]
) -> dict[str, Any]:
    benchmarks = []
    try:
        benchmark_root = str(benchmark_dir.relative_to(REPO_ROOT))
    except ValueError:
        benchmark_root = str(benchmark_dir)

    for benchmark in (benchmark_dir / "train").rglob("*.egg"):
        relative_benchmark = benchmark.relative_to(benchmark_dir / "train")
        bench_out_dir = REPORT_OUTPUT_DIR / relative_benchmark.with_suffix("")
        train_report = json.loads((bench_out_dir / "train.report.json").read_text(encoding="utf-8"))
        serve_report = json.loads((bench_out_dir / "serve.report.json").read_text(encoding="utf-8"))
        benchmarks.append(
            {
                "name": relative_benchmark.stem,
                "path": relative_benchmark.as_posix(),
                "train_report": train_report,
                "serve_report": serve_report,
            }
        )

    return {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "suite": benchmark_dir.name,
        "benchmark_root": benchmark_root,
        "summary": {
            "benchmark_count": len(benchmarks),
            "commands": command_results,
            "total_time_seconds": sum(
                result["time_seconds"] for result in command_results
            ),
        },
        "benchmarks": benchmarks,
    }


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as err:
        print(f"Command failed with exit code {err.returncode}: {err.cmd}", file=sys.stderr)
        raise
