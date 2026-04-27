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
TMP_DIR = NIGHTLY_DIR / "tmp"
DATA_JSON_PATH = DATA_DIR / "data.json"
POACH_BIN = REPO_ROOT / "target" / "release" / "poach"


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit(f"Usage: {Path(sys.argv[0]).name} <benchmark-dir>")

    benchmark_dir = (REPO_ROOT / sys.argv[1]).resolve()
    benchmark_files = list((benchmark_dir / "train").rglob("*.egg"))
    if not benchmark_files:
        raise SystemExit(f"No .egg benchmark files found under {benchmark_dir / 'train'}.")

    command_results = run_benchmarks(benchmark_dir)
    data = aggregate_reports(benchmark_dir, command_results)

    DATA_DIR.mkdir(parents=True, exist_ok=True)
    DATA_JSON_PATH.write_text(json.dumps(data, indent=2), encoding="utf-8")
    print(f"Wrote {DATA_JSON_PATH}")


def run_command(
    command: list[str],
    *,
    cwd: Path,
    stdout: Any | None = None,
    stderr: Any | None = None,
) -> dict[str, Any]:
    started_at = datetime.now(timezone.utc)
    started = time.perf_counter()
    try:
        completed = subprocess.run(
            command,
            check=True,
            cwd=cwd,
            stdout=stdout,
            stderr=stderr,
        )
    except subprocess.CalledProcessError as err:
        time_seconds = time.perf_counter() - started
        print(
            f"Command failed after {time_seconds:.3f}s: {' '.join(command)}",
            file=sys.stderr,
        )
        raise err

    finished_at = datetime.now(timezone.utc)
    return {
        "argv": command,
        "cwd": str(cwd),
        "returncode": completed.returncode,
        "started_at": started_at.isoformat(),
        "finished_at": finished_at.isoformat(),
        "time_seconds": time.perf_counter() - started,
    }


def run_benchmarks(benchmark_dir: Path) -> list[dict[str, Any]]:
    train_dir = benchmark_dir / "train"
    serve_dir = benchmark_dir / "serve"

    if REPORT_OUTPUT_DIR.exists():
        shutil.rmtree(REPORT_OUTPUT_DIR)
    REPORT_OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    TMP_DIR.mkdir(parents=True, exist_ok=True)

    command_results = []
    for benchmark_file in train_dir.rglob("*.egg"):
        relative_path = benchmark_file.relative_to(train_dir)
        serve_file = serve_dir / relative_path
        model_path = TMP_DIR / relative_path.with_suffix(".model")
        train_report_path = REPORT_OUTPUT_DIR / "train" / relative_path.with_suffix(".report.json")
        serve_report_path = REPORT_OUTPUT_DIR / "serve" / relative_path.with_suffix(".report.json")

        model_path.parent.mkdir(parents=True, exist_ok=True)
        train_report_path.parent.mkdir(parents=True, exist_ok=True)
        serve_report_path.parent.mkdir(parents=True, exist_ok=True)

        train_command = [
            str(POACH_BIN),
            "train",
            "--debug",
            str(benchmark_file),
            str(model_path),
        ]
        print("Running benchmark train:", " ".join(train_command))
        with train_report_path.open("w", encoding="utf-8") as report_file:
            command_results.append(
                run_command(
                    train_command,
                    cwd=REPO_ROOT,
                    stdout=subprocess.DEVNULL,
                    stderr=report_file,
                )
            )

        serve_command = [
            str(POACH_BIN),
            "serve",
            "--debug",
            str(model_path),
            "single",
            str(serve_file),
        ]
        print("Running benchmark serve:", " ".join(serve_command))
        with serve_report_path.open("w", encoding="utf-8") as report_file:
            command_results.append(
                run_command(
                    serve_command,
                    cwd=REPO_ROOT,
                    stdout=subprocess.DEVNULL,
                    stderr=report_file,
                )
            )

    return command_results


def aggregate_reports(
    benchmark_dir: Path, command_results: list[dict[str, Any]]
) -> dict[str, Any]:
    train_report_dir = REPORT_OUTPUT_DIR / "train"
    serve_report_dir = REPORT_OUTPUT_DIR / "serve"
    train_report_files = list(train_report_dir.rglob("*.report.json"))
    if not train_report_files:
        raise SystemExit(f"No report files were generated under {REPORT_OUTPUT_DIR}")

    benchmark_command_times = {}
    for result in command_results:
        mode = result["argv"][1]
        benchmark_path = Path(result["argv"][-1])
        if mode == "train":
            relative_path = benchmark_path.relative_to(benchmark_dir / "train")
        else:
            relative_path = benchmark_path.relative_to(benchmark_dir / "serve")
        benchmark_entry = benchmark_command_times.setdefault(str(relative_path), {})
        benchmark_entry[mode] = result["time_seconds"]

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
        "benchmarks": [
            {
                "path": str(report_file.relative_to(train_report_dir)),
                "train_time_seconds": benchmark_command_times[
                    str(report_file.relative_to(train_report_dir))
                ]["train"],
                "serve_time_seconds": benchmark_command_times[
                    str(report_file.relative_to(train_report_dir))
                ]["serve"],
                "train": json.loads(report_file.read_text(encoding="utf-8")),
                "serve": json.loads(
                    (serve_report_dir / report_file.relative_to(train_report_dir)).read_text(
                        encoding="utf-8"
                    )
                ),
            }
            for report_file in train_report_files
        ],
    }


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as err:
        print(f"Command failed with exit code {err.returncode}: {err.cmd}", file=sys.stderr)
        raise
